use crate::libs;
use crate::libs::app_config::AppConfigRef;
use crate::libs::ssh_session;
use crate::models::server::Server;
use crate::prelude::{Res, DATA_DIR_PATH};
use eyre::eyre;
use itertools::Itertools;
use rust_embed::Embed;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio::time::Instant;

/// default path of the agent on the sub server
const SS_AGENT_PATH: &str = "/usr/bin/managers_agent";
const AGENT_UNIT_NAME: &str = "managers_agent";
#[derive(Embed)]
#[folder = "../agent/"]
#[include= "src/*"]
#[include= "Cargo.toml"]
#[exclude= "target/*"]
struct AgentAsset;

#[derive(Embed)]
#[folder = "../agent-shared/"]
#[include= "src/*"]
#[include= "Cargo.toml"]
#[exclude= "target/*"]
struct AgentSharedAsset;

#[derive(Clone)]
pub struct AgentService {
    inner: Arc<Mutex<AgentServiceInner>>,
    app_config: AppConfigRef,
}
#[derive(Default)]
struct AgentServiceInner {
    last_agent_src_hash: String,
    agent_bin_path: String,
    last_shared_lib_src_hash: String
}

impl AgentService {
    pub async fn new(app_config: AppConfigRef) -> Self {
        let slf = Self {
            app_config,
            inner: Arc::new(Mutex::new(AgentServiceInner::default())),
        };
        log::info!("initializing agents");
        slf.build_agent().await.unwrap();
        slf
    }

    /// upload the binary of the agent to the specified server.
    ///
    /// the agent bin will be placed in `/usr/bin/managers_agent` in the sub-server
    ///
    /// a directory will be created in `/usr/share/managers_agent/` to write the `agent.lock` inside it
    ///
    /// the lock will be later used to compare src hashes and avoid rebuilding the same agent
    pub async fn upload_agent(&self, server: &Server) -> Res {
        let l = self.inner.lock().await;
        let agent_path = l.agent_bin_path.clone();
        let last_src_hash = l.last_agent_src_hash.clone();
        drop(l);
        if !tokio::fs::try_exists(&agent_path).await? {
            return Err(eyre!("agent did not build yet!"));
        }
        let agent_binary = tokio::fs::read(agent_path).await?;

        let mut ssh = ssh_session::connect(server).await?;

        log::info!("agent path: {SS_AGENT_PATH}");

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

        if sftp.try_exists(SS_AGENT_PATH).await? {
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
        let mut file = sftp.create(SS_AGENT_PATH).await?;
        file.write_all(&agent_binary).await?;
        file.flush().await?;
        file.shutdown().await?;

        ssh.call_capture_output(&format!("chmod +x {SS_AGENT_PATH}"))
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

    pub async fn init_agent(&self, server: &Server) -> Res {
        let mut ssh = ssh_session::connect(server).await?;

        let token = libs::create_jwt_token(&self.app_config.pwd, &server.id);
        let ip = public_ip::addr()
            .await
            .ok_or(eyre!("failed to find the public ip"))?;
        //use 3939 by default
        ssh.call_with_stdout(&format!("dash -c '{SS_AGENT_PATH} init {token} {ip}:3939'"))
            .await?;

        ssh.call_with_stdout(&format!("systemctl restart {AGENT_UNIT_NAME}"))
            .await?;

        ssh.close().await?;
        Ok(())
    }

    pub async fn build_agent(&self) -> Res {
        self.install_toolchain().await?;

        self.load_agent_lock().await?;

        let (cached, src_path) = self.write_agent_src().await?;
        let agent_cached = self.write_shared_lib_src().await?;
        if cached && agent_cached {
            let output_path = DATA_DIR_PATH.join("agent_output");
            let agent_path = self.sync_agent_bin_path(&output_path).await?;
            log::info!("skipping agent compilation -> {agent_path:?}");
            return Ok(());
        }
        let src_path = PathBuf::from(src_path).join("Cargo.toml");
        let src_path = src_path.to_str().unwrap().to_string();

        self.compile_agent(&src_path).await?;

        Ok(())
    }

    async fn compile_agent(&self, src_path: &str) -> Res {
        log::info!("compiling the agent...");
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
            "-q",
            "--release",
            "--manifest-path",
            src_path,
            "--target-dir",
            output_path_str,
            "--target",
            "x86_64-unknown-linux-musl",
        ]);

        let result = result.output()?;

        if result.status.success() {
            let agent_path = self.sync_agent_bin_path(&output_path).await?;
            log::info!(
                "agent compiled in {} seconds -> {agent_path:?}",
                instant.elapsed().as_secs()
            );

            let l = self.inner.lock().await;
            let hash = l.last_agent_src_hash.clone();
            drop(l);
            tokio::fs::write(output_path.join("agent.lock"), hash.as_bytes()).await?;
        } else {
            log::error!("failed to compile agent ({}):\n{:?}", result.status.to_string(),String::from_utf8(result.stderr));
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

    //todo write a generic impl for writing the source and remove these two functions
    async fn write_agent_src(&self) -> eyre::Result<(bool, String)> {
        let agent_src_output = DATA_DIR_PATH.join("agent_src");

        let agent_path_str = agent_src_output.to_str().unwrap().to_string();
        log::info!("building agent: {agent_path_str}");
        tokio::fs::create_dir_all(&agent_src_output).await?;

        let files = AgentAsset::iter().collect_vec();
        let hash = Self::calc_hash::<AgentAsset>(&files);
        let mut l = self.inner.lock().await;
        let last_src_hash = l.last_agent_src_hash.clone();

        if last_src_hash.eq(&hash) {
            log::info!("using cached src: {last_src_hash}");
            return Ok((true, agent_path_str));
        }

        l.last_agent_src_hash = hash;
        drop(l);

        for file in files {
            let file = file.to_string();
            log::trace!("writing src: {file}");

            let content = AgentAsset::get(&file).unwrap();

            //create parent if it doesnt exist
            let full_path = agent_src_output.join(&file);

            let parent = full_path.parent().unwrap();
            if !tokio::fs::try_exists(&parent).await? {
                tokio::fs::create_dir_all(&parent).await?;
            }

            log::info!("writing source: {full_path:?}");
            tokio::fs::write(&full_path, content.data).await?;
        }

        Ok((false, agent_path_str))
    }    
    
    async fn write_shared_lib_src(&self) -> eyre::Result<bool> {
        let shared_lib_src_output = DATA_DIR_PATH.join("agent-shared");

        let shared_lib_path_str = shared_lib_src_output.to_str().unwrap().to_string();
        log::info!("building shared lib: {shared_lib_path_str}");
        tokio::fs::create_dir_all(&shared_lib_src_output).await?;

        let files = AgentSharedAsset::iter().collect_vec();
        let hash = Self::calc_hash::<AgentSharedAsset>(&files);
        let mut l = self.inner.lock().await;
        let last_src_hash = l.last_shared_lib_src_hash.clone();

        if last_src_hash.eq(&hash) {
            log::info!("using cached src for shared lib: {last_src_hash}");
            return Ok(true);
        }

        l.last_shared_lib_src_hash = hash;
        drop(l);

        for file in files {
            let file = file.to_string();
            log::trace!("writing shared lib: {file}");

            let content = AgentSharedAsset::get(&file).unwrap();

            //create parent if it doesnt exist
            let full_path = shared_lib_src_output.join(&file);

            let parent = full_path.parent().unwrap();
            if !tokio::fs::try_exists(&parent).await? {
                tokio::fs::create_dir_all(&parent).await?;
            }

            log::info!("writing source: {full_path:?}");
            tokio::fs::write(&full_path, content.data).await?;
        }

        Ok(false)
    }

    async fn sync_agent_bin_path(&self, output_path: &Path) -> eyre::Result<PathBuf> {
        let agent_path = output_path
            .join("x86_64-unknown-linux-musl")
            .join("release")
            .join("agent");
        if !agent_path.exists() {
            return Err(eyre!("could not find the agent: {agent_path:?}"));
        }
        let mut l = self.inner.lock().await;
        l.agent_bin_path = agent_path.to_str().unwrap().to_owned();
        drop(l);
        Ok(agent_path)
    }

    async fn load_agent_lock(&self) -> Res {
        let agent_lock_path = DATA_DIR_PATH.join("agent_output").join("agent.lock");
        if tokio::fs::try_exists(&agent_lock_path).await? {
            let hash = tokio::fs::read_to_string(&agent_lock_path).await?;
            let mut l = self.inner.lock().await;
            l.last_agent_src_hash = hash;
            drop(l);
            log::info!("restored the last agent build");
        }
        Ok(())
    }

    fn calc_hash<T:Embed>(files: &[Cow<str>]) -> String {
        let mut joined_hash = Vec::with_capacity(files.len() * 32);
        
        for file in files {
            let content = T::get(file.as_ref()).unwrap();
            let hash = content.metadata.sha256_hash();
            for byte in hash {
                joined_hash.push(byte);
            }
        }

        hex::encode(joined_hash)
    }
}
