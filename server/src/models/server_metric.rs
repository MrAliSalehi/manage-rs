use crate::libs::rmp_serializer::RmpSerde;
use chrono::NaiveDateTime;
use machine_info::{SystemInfo, SystemStatus};
use native_db::ToKey;
use native_model::{native_model, Model};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[native_model(id = 2, version = 1, with = RmpSerde)]
#[native_db::native_db]
pub struct ServerMetric {
    #[primary_key]
    pub server_id: String,
    pub time: NaiveDateTime,
    pub system_info: SystemInfo,
    pub system_status: Option<SystemStatus>,
}
