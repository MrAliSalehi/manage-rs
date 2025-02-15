use machine_info::{SystemInfo, SystemStatus};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
pub struct ServerMetric {
    pub system_info: SystemInfo,
    pub system_status: Option<SystemStatus>,
}

#[derive(Serialize,Deserialize)]
pub struct Config {
    pub api_host:String,
    pub auth_token:String,
}
