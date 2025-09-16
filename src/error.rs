use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    
    #[error("Configuration error: {0}")]
    Config(#[from] envy::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Invalid email ID: {0}")]
    InvalidEmailId(String),
    
    #[error("Email not found for tenant")]
    EmailNotFound,
    
    #[error("Tenant not found")]
    TenantNotFound,
    
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    
    #[error("Template rendering error: {0}")]
    Template(#[from] askama::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Database(_) => {
                tracing::error!("Database error: {}", self);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
            AppError::Config(_) => {
                tracing::error!("Configuration error: {}", self);
                (StatusCode::INTERNAL_SERVER_ERROR, "Configuration error")
            }
            AppError::Io(_) => {
                tracing::error!("IO error: {}", self);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
            AppError::InvalidEmailId(_) => {
                tracing::warn!("Invalid email ID: {}", self);
                (StatusCode::BAD_REQUEST, "Invalid email ID")
            }
            AppError::EmailNotFound => {
                tracing::warn!("Email not found");
                (StatusCode::NOT_FOUND, "Email not found")
            }
            AppError::TenantNotFound => {
                tracing::warn!("Tenant not found");
                (StatusCode::NOT_FOUND, "Tenant not found")
            }
            AppError::InvalidUrl(_) => {
                tracing::warn!("Invalid URL: {}", self);
                (StatusCode::BAD_REQUEST, "Invalid URL")
            }
            AppError::Template(_) => {
                tracing::error!("Template rendering error: {}", self);
                (StatusCode::INTERNAL_SERVER_ERROR, "Template error")
            }
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;