use crate::models::server::ServerSecret;
use serde::Deserialize;

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct AddOrUpdateServerRequest {
    pub name: String,
    pub ip: String,
    pub port: Option<usize>,
    pub user: String,
    pub secret: ServerSecret,
}
