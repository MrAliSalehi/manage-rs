use machine_info::{SystemInfo, SystemStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum Signal {
    Init,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientMessage {
    pub token: Option<String>,
    pub message: ClientMessageDetail,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessageDetail {
    Ping,
    UpdateMetric { metric: AddServerMetric },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    Ping,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AddServerMetric {
    pub system_info: SystemInfo,
    pub system_status: Option<SystemStatus>,
}
