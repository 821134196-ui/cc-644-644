use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
    #[error("{message}")]
    BadRequest { message: String },
    #[error("{message}")]
    NotFound { message: String },
    /// 409: 库存不足 / 母盒不可售 / 打印失败已回滚 等业务冲突
    #[error("{message}")]
    Conflict { message: String, code: String },
}

impl AppError {
    pub fn conflict(code: &str, message: impl Into<String>) -> Self {
        AppError::Conflict {
            message: message.into(),
            code: code.to_string(),
        }
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        AppError::BadRequest {
            message: msg.into(),
        }
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        AppError::NotFound {
            message: msg.into(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            AppError::Db(_) | AppError::Other(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
            AppError::BadRequest { .. } => (StatusCode::BAD_REQUEST, "bad_request"),
            AppError::NotFound { .. } => (StatusCode::NOT_FOUND, "not_found"),
            AppError::Conflict { code, .. } => (StatusCode::CONFLICT, code.as_str()),
        };
        let message = self.to_string();
        tracing::warn!(%code, %message, "request failed");
        (status, Json(json!({ "error": message, "code": code }))).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
