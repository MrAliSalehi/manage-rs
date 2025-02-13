use crate::libs::shared_state::SharedState;
use crate::middlewares::auth_mw::require_authentication;
use axum::middleware::from_fn_with_state;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

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
