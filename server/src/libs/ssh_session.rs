use std::borrow::Cow;
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use crate::models::server::{Server, ServerSecret};
use crate::prelude::Res;
use eyre::eyre;
use russh::keys::*;
use russh::*;
use russh_sftp::client::SftpSession;
use tokio::io::AsyncWriteExt;

pub async fn connect(server: &Server) -> eyre::Result<SshSession> {
    let ServerSecret::Pwd(pwd) = &server.secret else {
        return Err(eyre!("ssh keys are not supported yet!"));
    };

    SshSession::connect(&server.user, pwd, &server.ip, server.port).await
}

pub struct SshSession {
    session: client::Handle<SshClient>,
}

struct SshClient;

impl client::Handler for SshClient {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}
impl SshSession {
    pub async fn connect(
        user: &String,
        pwd: &String,
        host: &str,
        port: usize,
    ) -> eyre::Result<Self> {
        let addrs = SocketAddr::from((IpAddr::V4(Ipv4Addr::from_str(host)?), port as u16));
        log::info!("connecting to {addrs} : {user} + {pwd}");
        let config = client::Config {
            inactivity_timeout: Some(Duration::from_secs(5)),
            preferred: Preferred {
                kex: Cow::Owned(vec![
                    russh::kex::CURVE25519_PRE_RFC_8731,
                    russh::kex::EXTENSION_SUPPORT_AS_CLIENT,
                ]),
                ..Default::default()
            },
            ..<_>::default()
        };

        let config = Arc::new(config);
        let sh = SshClient {};

        let mut session = client::connect(config, addrs, sh).await?;

        let auth_res = session.authenticate_password(user, pwd).await?;

        if !auth_res.success() {
            return Err(eyre!("Authentication (with publickey) failed"));
        }

        Ok(Self { session })
    }

    pub async fn call_capture_output(&mut self, command: &str) -> eyre::Result<String> {
        let output_str;
        {
            let output = Arc::new(std::sync::RwLock::new(vec![]));
            let output_cl = output.clone();
            self.call(command, move |data| {
                let output_cl = output_cl.clone();
                async move {
                    output_cl.clone().write().unwrap().extend(data.iter());
                    Ok(())
                }
            })
            .await?;

            let b = output.read().unwrap();
            output_str = String::from_utf8_lossy(b.deref()).to_string();
        }
        Ok(output_str)
    }

    pub async fn call_with_stdout(&mut self, command: &str) -> Res {
        self.call(command, move |data| async move {
            let mut stdout = tokio::io::stdout();
            stdout.write_all(&data).await?;
            stdout.flush().await?;
            Ok(())
        })
        .await?;
        Ok(())
    }

    pub async fn call<F: Future<Output = Res>>(
        &mut self,
        command: &str,
        on_data_cb: impl Fn(CryptoVec) -> F,
    ) -> eyre::Result<u32> {
        let mut channel = self.session.channel_open_session().await?;
        channel.exec(true, command).await?;

        let mut code = None;

        loop {
            let Some(msg) = channel.wait().await else {
                break;
            };
            match msg {
                ChannelMsg::Data { data } => {
                    on_data_cb(data).await?;
                }
                ChannelMsg::ExitStatus { exit_status } => {
                    code = Some(exit_status);
                }
                _ => {}
            }
        }
        Ok(code.expect("program did not exit cleanly"))
    }

    pub async fn get_sftp(&self) -> eyre::Result<SftpSession> {
        let channel = self.session.channel_open_session().await?;
        channel.request_subsystem(true, "sftp").await?;
        Ok(SftpSession::new(channel.into_stream()).await?)
    }

    pub async fn close(&mut self) -> Res {
        self.session
            .disconnect(Disconnect::ByApplication, "", "English")
            .await?;
        Ok(())
    }
}
