use crate::libs::ssh_session;
use crate::models::server::Server;
use crate::prelude::{Res, DATA_DIR_PATH};
use eyre::eyre;
use itertools::Itertools;
use rust_embed::Embed;
use std::borrow::Cow;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio::time::Instant;

#[derive(Embed)]
#[folder = "../agent/"]
#[include = "*.toml"]
#[include = "*.rs"]
struct AgentAsset;

#[derive(Clone)]
pub struct AgentService {
    inner: Arc<Mutex<AgentServiceInner>>,
}
struct AgentServiceInner {
    last_src_hash: String,
    agent_bin_path: String,
}

impl AgentService {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(AgentServiceInner {
                last_src_hash: Default::default(),
                agent_bin_path: Default::default(),
            })),
        }
    }

    pub async fn upload_agent(&self, server: Server) -> Res {
        let l = self.inner.lock().await;
        let agent_path = l.agent_bin_path.clone();
        let last_src_hash = l.last_src_hash.clone();
        drop(l);
        if !tokio::fs::try_exists(&agent_path).await? {
            return Err(eyre!("agent did not build yet!"));
        }
        let agent_binary = tokio::fs::read(agent_path).await?;

        let mut ssh = ssh_session::connect(&server).await?;

        //default path of the agent on the sub server
        let ss_agent_path = "/usr/bin/managers_agent".to_owned();

        log::info!("agent path: {ss_agent_path}");

        let sftp = ssh.get_sftp().await?;

        // create managers dir
        let managers_agent_dir = PathBuf::from_str("/usr/share/managers_agent")?;
        let managers_agent_dir_str = managers_agent_dir.to_str().unwrap().to_owned();

        if !sftp.try_exists(&managers_agent_dir_str).await? {
            sftp.create_dir(&managers_agent_dir_str).await?;
        }
        //agent lock file
        let agent_lock_file = managers_agent_dir
            .join("agent.lock")
            .to_str()
            .unwrap()
            .to_owned();

        if sftp.try_exists(&ss_agent_path).await? {
            log::info!("an old version of the agent already exists, checking for hashes");
            //check lock file, if the src is not changed, do not replace the agent
            let mut file = sftp.open(&agent_lock_file).await?;
            let mut hash = String::default();
            file.read_to_string(&mut hash).await?;
            if hash.eq(&last_src_hash) {
                log::error!("agent already exists on machine {}", server.ip);
                return Ok(());
            }
        }

        log::info!("sending agent to {}", server.ip);
        let mut file = sftp.create(&ss_agent_path).await?;
        file.write_all(&agent_binary).await?;
        file.flush().await?;
        file.shutdown().await?;

        ssh.call_capture_output(&format!("chmod +x {ss_agent_path}"))
            .await?;

        let mut file = sftp.create(agent_lock_file).await?;

        file.write_all(last_src_hash.as_bytes()).await?;
        file.flush().await?;
        file.shutdown().await?;

        sftp.close().await?;
        ssh.close().await?;
        log::info!("agent sent");
        Ok(())
    }

    pub async fn build_agent(&self) -> Res {
        self.install_toolchain().await?;

        self.load_agent_lock().await?;

        let src_path = self.write_agent_src().await?;
        let src_path = PathBuf::from(src_path).join("Cargo.toml");
        let src_path = src_path.to_str().unwrap().to_string();

        self.compile_agent(&src_path).await?;

        Ok(())
    }

    async fn compile_agent(&self, src_path: &str) -> Res {
        let output_path = DATA_DIR_PATH.join("agent_output");
        let output_path_str = output_path.to_str().unwrap();

        let instant = Instant::now();
        let cargo = dirs::home_dir()
            .unwrap()
            .join(".cargo")
            .join("bin")
            .join("cargo");

        let mut command = std::process::Command::new(cargo.to_str().unwrap());
        let result = command.args([
            "build",
            "--release",
            "--manifest-path",
            src_path,
            "--target-dir",
            output_path_str,
            "--target",
            "x86_64-unknown-linux-musl",
        ]);

        let status = result.status()?;

        if status.success() {
            let agent_path = output_path
                .join("x86_64-unknown-linux-musl")
                .join("release")
                .join("agent");
            if !agent_path.exists() {
                log::error!("could not find the agent: {agent_path:?}");
                return Ok(());
            }
            let mut l = self.inner.lock().await;
            l.agent_bin_path = agent_path.to_str().unwrap().to_owned();
            drop(l);
            log::info!(
                "agent compiled in {} seconds -> {agent_path:?}",
                instant.elapsed().as_secs()
            );

            let l = self.inner.lock().await;
            let hash = l.last_src_hash.clone();
            drop(l);
            tokio::fs::write(output_path.join("agent.lock"), hash.as_bytes()).await?;
        } else {
            log::error!("failed to compile agent: {}", status.to_string());
        }
        Ok(())
    }

    pub async fn install_toolchain(&self) -> Res {
        log::info!("checking for toolchain");
        let need_installation;
        let mut rustup_exists = false;
        //if the rustup is installed it will be here
        let installed_rustup_path = dirs::home_dir()
            .unwrap()
            .join(".cargo")
            .join("bin")
            .join("rustup");

        match std::process::Command::new(&installed_rustup_path)
            .arg("show")
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    rustup_exists = true;
                    let output = String::from_utf8(output.stdout)?;
                    need_installation = !output.contains("x86_64-unknown-linux-musl");
                } else {
                    need_installation = true;
                }
            }
            Err(_) => {
                rustup_exists = false;
                need_installation = true;
            }
        };

        if !need_installation {
            log::info!("toolchain already installed");
            return Ok(());
        }

        if !rustup_exists {
            let rustup_path = DATA_DIR_PATH.join("rustup").join("rustup.sh");

            let parent = rustup_path.parent().unwrap();
            if !tokio::fs::try_exists(&parent).await? {
                tokio::fs::create_dir_all(&parent).await?;
            }

            log::info!("downloading rustup -> {rustup_path:?}");

            let status = std::process::Command::new("curl")
                .args([
                    "--proto",
                    "=https",
                    "--tlsv1.2",
                    "-sSf",
                    "https://sh.rustup.rs",
                    "-o",
                    rustup_path.to_str().unwrap(),
                ])
                .status()?;

            if !status.success() {
                log::error!("failed to install rustup: {}", status.to_string());
                return Ok(());
            }
            log::info!("installing rustup");

            std::process::Command::new("chmod")
                .args(["+x", rustup_path.to_str().unwrap()])
                .status()?;

            //-y --default-host x86_64-unknown-linux-musl --no-modify-path --no-update-default-toolchain
            let status = std::process::Command::new(rustup_path.to_str().unwrap())
                .args(["-y", "--no-modify-path"])
                .status()?;

            if !status.success() {
                log::info!(
                    "failed to install rustc toolchain (x86_64-unknown-linux-musl): {}",
                    status.to_string()
                );
                return Ok(());
            }
        }

        log::info!("adding target for x86_64-unknown-linux-musl");
        //rustup default stable
        std::process::Command::new(installed_rustup_path.to_str().unwrap())
            .args(["target", "add", "x86_64-unknown-linux-musl"])
            .status()?;

        Ok(())
    }

    async fn write_agent_src(&self) -> eyre::Result<String> {
        let agent_src_output = DATA_DIR_PATH.join("agent_src");

        let agent_path_str = agent_src_output.to_str().unwrap().to_string();
        log::info!("building agent: {agent_path_str}");
        tokio::fs::create_dir_all(&agent_src_output).await?;

        let files = AgentAsset::iter().collect_vec();
        let hash = Self::calc_hash(&files);
        let mut l = self.inner.lock().await;
        let last_src_hash = l.last_src_hash.clone();

        if last_src_hash.eq(&hash) {
            log::info!("using cached src: {last_src_hash}");
            return Ok(agent_path_str);
        }

        l.last_src_hash = hash;
        drop(l);

        for file in files {
            let file = file.to_string();
            log::trace!("writing src: {file}");

            let content = AgentAsset::get(&file).unwrap();

            //create parent if it doesnt exist
            let full_path = agent_src_output.join(&file);

            let parent = full_path.parent().unwrap();
            log::info!("parent: {parent:?}");
            if !tokio::fs::try_exists(&parent).await? {
                tokio::fs::create_dir_all(&parent).await?;
            }

            log::info!("final_path: {full_path:?}");
            tokio::fs::write(&full_path, content.data).await?;
        }

        Ok(agent_path_str)
    }

    async fn load_agent_lock(&self) -> Res {
        let agent_lock_path = DATA_DIR_PATH.join("agent_output").join("agent.lock");
        if tokio::fs::try_exists(&agent_lock_path).await? {
            let hash = tokio::fs::read_to_string(&agent_lock_path).await?;
            let mut l = self.inner.lock().await;
            l.last_src_hash = hash;
            drop(l);
            log::info!("restored the last agent build");
        }
        Ok(())
    }
    fn calc_hash(files: &[Cow<str>]) -> String {
        let mut joined_hash = Vec::with_capacity(files.len() * 32);

        for file in files {
            let content = AgentAsset::get(file.as_ref()).unwrap();
            let hash = content.metadata.sha256_hash();
            for byte in hash {
                joined_hash.push(byte);
            }
        }

        hex::encode(joined_hash)
    }
}
