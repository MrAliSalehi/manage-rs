use crate::libs::shared_state::SharedState;
use crate::prelude::Res;
use axum::http::StatusCode;
use axum::routing::get;
use axum_helmet::{Helmet, HelmetLayer};
use std::time::Duration;
use tokio::net::TcpListener;
use tower::{BoxError, ServiceBuilder};
use tower_http::cors::{Any, CorsLayer};

pub mod components;
const T_OUT: Duration = Duration::from_secs(10);
pub async fn run(state:SharedState) -> Res {
    
    let helmet = build_helmet();

    let app = axum::Router::new()
        .route("/", get(components::root))
        .merge(components::routes(state.clone()))
        .layer(
            ServiceBuilder::new() //executes from top to bottom
                .layer(axum::error_handling::HandleErrorLayer::new(unhandled_err))
                .layer(tower_http::catch_panic::CatchPanicLayer::new())
                .layer(tower_http::timeout::TimeoutLayer::new(T_OUT))
                .layer(HelmetLayer::new(helmet))
                .layer(cors())
                .layer(tower::buffer::BufferLayer::new(2048)),
        );

    log::info!("api is running on {}", state.app_config.port);
    let socket = TcpListener::bind(format!("0.0.0.0:{}", state.app_config.port)).await?;
    axum::serve(socket, app).await?;
    Ok(())
}

fn build_helmet() -> Helmet {
    Helmet::new()
        .add(axum_helmet::XContentTypeOptions::nosniff())
        .add(axum_helmet::ReferrerPolicy::StrictOriginWhenCrossOrigin)
        .add(axum_helmet::XFrameOptions::SameOrigin) // deprecated
        .add(axum_helmet::XDownloadOptions::NoOpen)
        .add(axum_helmet::CrossOriginEmbedderPolicy::RequireCorp)
        .add(
            axum_helmet::ContentSecurityPolicy::new()
                //disable iframe or cross-content
                .frame_src(vec!["'none'"])
                .frame_ancestors(vec!["'none'"])
                .default_src(vec!["'self'"]),
        )
}

async fn unhandled_err(e: BoxError) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Unhandled error: {e}"),
    )
}

#[inline]
fn cors() -> CorsLayer {
    CorsLayer::new()
        .allow_headers(Any)
        .allow_methods(Any)
        .allow_origin(Any)
        .allow_private_network(true)
}
