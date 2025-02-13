use crate::api::components::servers::models::AddOrUpdateServerRequest;
use crate::libs::api_response::ApiResponse;
use crate::libs::shared_state::SharedState;
use crate::models::server::Server;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use std::net::Ipv4Addr;
use std::str::FromStr;

pub mod models;

pub fn routes(state: SharedState) -> Router {
    Router::new()
        .route("/", get(get_servers).post(add_server))
        .route(
            "/{id}",
            get(get_by_id).put(update_server).delete(delete_server),
        )
        .with_state(state.clone())
}
async fn get_by_id(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResponse {
    state
        .db_driver
        .get_server_by_id(id)
        .map(|a| {
            a.map(|s| ApiResponse::ok("", Some(json!(s))))
                .unwrap_or(ApiResponse::bad_request("server not found"))
        })
        .map_err(|e| ApiResponse::internal(&e.to_string()))
        .into()
}

async fn delete_server(State(state): State<SharedState>, Path(id): Path<String>) -> ApiResponse {
    state
        .db_driver
        .delete_server(id)
        .map(|_| ApiResponse::ok("", None))
        .map_err(|e| ApiResponse::internal(&e.to_string()))
        .into()
}

async fn update_server(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<AddOrUpdateServerRequest>,
) -> ApiResponse {
    state
        .db_driver
        .update_server(Server::from_req(id, req))
        .map(|r| ApiResponse::ok("", Some(json!({"old":r}))))
        .map_err(|e| ApiResponse::internal(&e.to_string()))
        .into()
}

async fn get_servers(State(state): State<SharedState>) -> ApiResponse {
    state
        .db_driver
        .all_servers()
        .map(|f| ApiResponse::ok("", Some(json!(f))))
        .map_err(|e| ApiResponse::bad_request(e.to_string()))
        .into()
}

async fn add_server(
    State(state): State<SharedState>,
    Json(req): Json<AddOrUpdateServerRequest>,
) -> eyre::Result<ApiResponse, ApiResponse> {
    let ip = Ipv4Addr::from_str(&req.ip).map_err(|_| ApiResponse::bad_request("invalid ip!"))?;

    let valid = !ip.is_loopback() && !ip.is_unspecified();
    if !valid {
        return Err(ApiResponse::bad_request("ip address is not reachable!"));
    }

    let port = if let Some(p) = req.port {
        if p == 0 {
            20
        } else {
            p
        }
    } else {
        20
    };

    let id = cuid2::create_id();
    state
        .db_driver
        .add_server(Server {
            port,
            user:req.user,
            name: req.name.trim().to_owned(),
            ip: ip.to_string(),
            id: id.clone(),
            secret: req.secret,
        })
        .map(|_| ApiResponse::ok("", Some(json!({"id":id}))))
        .map_err(|e| ApiResponse::bad_request(e.to_string()))
}
