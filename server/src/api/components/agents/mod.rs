use crate::libs::api_response::ApiResponse;
use crate::libs::shared_state::SharedState;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{debug_handler, Router};

pub fn routes(state: SharedState) -> Router {
    Router::new()
        .route("/run/{server_id}", get(run_agent))
        .with_state(state.clone())
}

#[debug_handler]
async fn run_agent(
    State(state): State<SharedState>,
    Path(server_id): Path<String>,
) -> eyre::Result<ApiResponse, ApiResponse> {
    let server = state
        .db_driver
        .get_server_by_id(server_id)
        .map_err(|e|ApiResponse::internal(&e.to_string()))?
        .ok_or(ApiResponse::bad_request("server not found"))?;
    
    //todo check if agent already exist and/or you wanna "force" update it
    
    state.agent_service.upload_agent(server).await
        .map_err(|e|ApiResponse::internal(&e.to_string()))?;
    
    
    
    Ok(ApiResponse::bad_request(""))
}
