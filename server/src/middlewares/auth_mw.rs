use crate::libs::api_response::ApiResponse;
use crate::libs::shared_state::SharedState;
use axum::extract::{Request, State};
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;

pub async fn require_authentication(
    State(state): State<SharedState>,
    req: Request,
    next: Next,
) -> eyre::Result<Response, ApiResponse> {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok().map(|f| f.replace("Bearer ", "")))
        .ok_or_else(|| ApiResponse::unauthorized("authorization header is required"))?;

    if !token.eq(&state.app_config.pwd) {
        return Err(ApiResponse::unauthorized("invalid auth key!"));
    }

    Ok(next.run(req).await)
}
