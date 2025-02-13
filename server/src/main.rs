use crate::libs::app_config::{AppConfig, AppConfigRef};
use clap::Parser;
use prelude::Res;

mod api;
mod libs;
mod middlewares;
mod models;
mod prelude;

#[tokio::main]
async fn main() -> Res {
    dotenv::dotenv().ok();
    prelude::init_logger().await?;
    
    let mut config = AppConfig::parse();
    config.default().await?;
    let config = AppConfigRef::from(config);

    api::run(config).await?;

    Ok(())
}
