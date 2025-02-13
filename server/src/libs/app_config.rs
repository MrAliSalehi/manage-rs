use crate::libs::agent_service::AgentService;
use crate::prelude::{Res, DATA_DIR_PATH};
use clap::ArgAction;
use clap::Parser;
use eyre::eyre;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, LazyLock};

static CONFIG_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| dirs::config_dir().unwrap().join("managers_server.json"));
const DB_NAME: &str = "native.db";

#[derive(Parser, Debug, Serialize, Deserialize, Default, Clone)]
#[command(version, about, long_about = None)]
#[serde(default)]
pub struct AppConfig {
    #[arg(
        short,
        long,
        default_value = "",
        help = "password for authenticating in the api, if not specified a random string will be generated"
    )]
    pub pwd: String,

    #[arg(long, default_value_t = 8080, help = "specify the api port")]
    pub port: usize,

    #[arg(short,long, action=ArgAction::SetTrue,help="initialize the api and sync the new configurations")]
    #[serde(skip)]
    pub init: bool,

    #[arg(short,long,default_value_t=default_db_path(), help="storage path for the api")]
    pub db_path: String,
}

#[derive(Clone)]
pub struct AppConfigRef {
    inner: Arc<AppConfig>,
}

impl AppConfig {
    pub async fn default(&mut self) -> Res {
        if self.init {
            self.init_config().await?;
            return Ok(());
        }
        if !tokio::fs::try_exists(CONFIG_PATH.deref()).await? {
            log::error!("the server is not initialized, see --help for more info.");
            return Ok(());
        }
        if !self.pwd.is_empty() {
            log::info!(
                "the --pwd flag was ignored, to change the password use --init flag with the new password"
            );
        }
        log::info!("loading {}", CONFIG_PATH.deref().to_str().unwrap());
        let str = tokio::fs::read_to_string(CONFIG_PATH.deref()).await?;
        *self = serde_json::from_str(&str)?;
        log::info!("password: {}", self.pwd);
        Ok(())
    }
    
    pub async fn check_agents(&self, agent_service: &AgentService) -> Res {
        log::info!("initializing agents");
        agent_service.build_agent().await
    }
    
    async fn init_config(&mut self) -> Res {
        log::info!("initializing the server");
        if self.pwd.is_empty() {
            self.pwd = cuid2::CuidConstructor::default().with_length(8).create_id();
            log::info!(
                "default password:{}. you can change this via CLI options, see --help for more info.",
                self.pwd
            );
        }
        if !tokio::fs::try_exists(&self.db_path).await? {
            let base = PathBuf::from_str(&self.db_path)?;
            let dir = base.parent().ok_or(eyre!(""))?;
            tokio::fs::create_dir_all(dir).await?;
        }
        let slf = serde_json::to_vec(&self)?;
        tokio::fs::write(CONFIG_PATH.deref(), &slf).await?;
        Ok(())
    }
}
fn default_db_path() -> String {
    DATA_DIR_PATH.join(DB_NAME).to_str().unwrap().to_owned()
}
impl From<AppConfig> for AppConfigRef {
    fn from(value: AppConfig) -> Self {
        AppConfigRef {
            inner: Arc::new(value),
        }
    }
}

impl std::ops::Deref for AppConfigRef {
    type Target = AppConfig;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
