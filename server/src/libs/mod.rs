use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};

pub mod agent_service;
pub mod api_response;
pub mod app_config;
pub mod db_driver;
pub mod rmp_serializer;
pub mod shared_state;
pub mod ssh_session;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TokenClaims {
    pub sub: String,
    pub iat: usize,
    pub exp: usize,
}

pub(crate) fn create_jwt_token(secret: &str, sub: &String) -> String {
    let now = chrono::Utc::now();
    let claims = TokenClaims {
        sub: sub.to_owned(),
        iat: now.timestamp() as usize,
        exp: 0,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(&secret.as_bytes()),
    )
    .unwrap_or_default()
}
