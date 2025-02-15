use crate::libs::agent_service::AgentService;
use crate::libs::app_config::{AppConfig, AppConfigRef};
use crate::libs::shared_state::SharedState;
use clap::Parser;
use prelude::Res;

mod api;
mod libs;
mod middlewares;
mod models;
mod prelude;
mod sub_server_io;

#[tokio::main]
async fn main() -> Res {
    dotenv::dotenv().ok();
    prelude::init_logger().await?;

    let mut config = AppConfig::parse();
    config.default().await?;
    
    let config = AppConfigRef::from(config);
    
    let agent_service = AgentService::new(config.clone()).await;
    
    let state = SharedState::new(config.clone(), agent_service).await;
    
    sub_server_io::run(state.clone())?;
    api::run(state.clone()).await?;

    Ok(())
}
