
use axum::http::StatusCode;
use axum::Json;
use axum::response::{IntoResponse, Response};
use serde_json::{json, Value};

pub(crate) struct ApiResponse {
    pub message: String,
    pub data: Option<Value>,
    pub status: StatusCode,
}


impl ApiResponse {
    pub(crate) fn ok<M: Into<String>>(message: M, data: Option<Value>) -> Self {
        Self {
            message: message.into(),
            data,
            status: StatusCode::OK,
        }
    }

    pub fn bad_request<M: Into<String>>(message: M) -> Self {
        Self {
            data: None,
            message: message.into(),
            status: StatusCode::BAD_REQUEST,
        }
    }

    pub(crate) fn internal(msg: &str) -> Self {
        Self {
            message: msg.into(),
            data: None,
            status: StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn unauthorized(message: &str) -> Self {
        Self {
            data: None,
            message: message.into(),
            status: StatusCode::UNAUTHORIZED,
        }
    }
    #[allow(unused)]
    pub fn conflict(message: &str) -> Self {
        Self {
            data: None,
            message: message.into(),
            status: StatusCode::CONFLICT,
        }
    }
}

impl IntoResponse for ApiResponse {
    fn into_response(self) -> Response {
        (self.status, Json(json!({
            "message":self.message,
            "data":self.data,
        }))).into_response()
    }
}

impl From<eyre::Result<ApiResponse,ApiResponse>> for ApiResponse {
    fn from(value: eyre::Result<ApiResponse, ApiResponse>) -> Self {
        value.unwrap_or_else(|f| f)
    }
}