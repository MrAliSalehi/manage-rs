use crate::api::components::servers::models::AddOrUpdateServerRequest;

use crate::libs::rmp_serializer::RmpSerde;
use native_db::ToKey;
use native_model::{native_model, Model};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default)]
#[native_model(id = 1, version = 1,with = RmpSerde)]
#[native_db::native_db]
pub struct Server {
    #[primary_key]
    pub id: String,
    pub name: String,
    #[secondary_key(unique)]
    pub ip: String,
    /// default 20
    pub port: usize,
    pub user: String,
    pub secret: ServerSecret,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", content = "value")]
#[native_model(id = 1, version = 1,with = RmpSerde)]
pub enum ServerSecret {
    Pwd(String),
    SshKey(String),
}

impl Server {
    pub fn from_req(id: String, value: AddOrUpdateServerRequest) -> Self {
        Self {
            id,
            user: value.user,
            name: value.name,
            ip: value.ip,
            port: value.port.unwrap_or(20),
            secret: value.secret,
        }
    }
}

impl Default for ServerSecret {
    fn default() -> Self {
        Self::Pwd(Default::default())
    }
}
