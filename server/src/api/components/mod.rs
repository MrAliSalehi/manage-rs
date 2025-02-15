use std::ops::Deref;
use std::sync::LazyLock;
use axum::extract::{Request, State};
use axum::http::header;
use crate::libs::shared_state::SharedState;
use crate::middlewares::auth_mw::require_authentication;
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use jsonwebtoken::{decode, DecodingKey, Validation};
use crate::libs::api_response::ApiResponse;
use crate::libs::TokenClaims;

pub mod agents;
pub mod servers;

pub fn routes(state: SharedState) -> Router {
    Router::new()
        .merge(authorized_routes(state.clone()))
        .route("/ping", get(root))
}

fn authorized_routes(state: SharedState) -> Router {
    Router::new()
        .nest("/servers", servers::routes(state.clone()))
        .nest("/agents", agents::routes(state.clone()))
        .layer(from_fn_with_state(state.clone(), require_authentication))
}

pub(crate) async fn root() -> impl IntoResponse {
    "UP".into_response()
}


/*pub async fn authorize_servers(State(state): State<SharedState>, mut req: Request, next: Next) -> eyre::Result<Response, ApiResponse> {
    let token = req.headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok().map(|f| f.replace("Bearer ", "")))
        .ok_or_else(|| ApiResponse::unauthorized("authorization header is required"))?;


    let claims = decode::<TokenClaims>(&token, &DecodingKey::from_secret(&state.app_config.pwd.as_bytes()), V.deref())
        .map_err(|_| ApiResponse::unauthorized("invalid token"))?;

    req.extensions_mut().insert(claims.claims);
    Ok(next.run(req).await)
}*/