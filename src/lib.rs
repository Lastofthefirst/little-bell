use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tower_http::{compression::CompressionLayer, trace::TraceLayer, cors::CorsLayer};
use tracing::{info, warn};

pub mod database;
pub mod error;

use database::{Database, EventStats};
use error::{AppError, AppResult};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_database_url")]
    pub database_url: String,
    #[serde(default = "default_base_url")]
    pub base_url: String,
}

fn default_port() -> u16 {
    3000
}

fn default_database_url() -> String {
    "sqlite:data/tracking.db".to_string()
}

fn default_base_url() -> String {
    "http://localhost:3000".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            port: 3000,
            database_url: "sqlite:data/tracking.db".to_string(),
            base_url: "http://localhost:3000".to_string(),
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, envy::Error> {
        envy::from_env()
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub config: Config,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    tenant_id: String,
    stats: EventStats,
    base_url: String,
}

#[derive(Deserialize)]
pub(crate) struct ClickQuery {
    url: String,
}

#[derive(Deserialize, Serialize)]
pub struct CreateEmailRequest {
    pub subject: Option<String>,
    pub recipient: Option<String>,
}

#[derive(Serialize)]
pub struct CreateEmailResponse {
    pub email_id: i64,
    pub tracking_pixel_url: String,
}

pub async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "little-bell",
        "version": "0.1.0",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

pub async fn metrics(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    // Basic system metrics
    let uptime = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Get database file size if possible
    let db_path = state.config.database_url.strip_prefix("sqlite:").unwrap_or(&state.config.database_url);
    let db_size = std::fs::metadata(db_path)
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(Json(serde_json::json!({
        "service": "little-bell",
        "version": "0.1.0",
        "uptime_seconds": uptime,
        "database": {
            "path": db_path,
            "size_bytes": db_size
        },
        "memory_usage": {
            "rss_bytes": get_memory_usage()
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

// Simple memory usage getter (Unix only)
#[cfg(unix)]
fn get_memory_usage() -> u64 {
    use std::fs;
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                if let Some(kb_str) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = kb_str.parse::<u64>() {
                        return kb * 1024; // Convert KB to bytes
                    }
                }
            }
        }
    }
    0
}

#[cfg(not(unix))]
fn get_memory_usage() -> u64 {
    0 // Not implemented for non-Unix systems
}

pub async fn track_open(
    Path((tenant_id, email_id_str)): Path<(String, String)>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> AppResult<impl IntoResponse> {
    // Extract email ID from the path (remove .gif extension)
    let email_id_str = email_id_str.strip_suffix(".gif").unwrap_or(&email_id_str);
    let email_id = email_id_str.parse::<i64>()
        .map_err(|_| AppError::InvalidEmailId(email_id_str.to_string()))?;

    // Extract user agent and IP address
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    
    let ip_address = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());

    // Verify email exists and belongs to tenant
    let email = state.db.get_email(email_id, &tenant_id).await?;
    
    match email {
        Some(_) => {
            // Log the open event
            state.db.log_event(
                email_id,
                "open",
                user_agent.as_deref(),
                ip_address.as_deref(),
            ).await?;

            info!(
                tenant_id = %tenant_id,
                email_id = %email_id,
                ip_address = ?ip_address,
                "Email opened"
            );

            // Return 1x1 transparent GIF
            let gif_bytes = include_bytes!("pixel.gif");
            Ok(Response::builder()
                .header("Content-Type", "image/gif")
                .header("Cache-Control", "no-store, no-cache, must-revalidate")
                .header("Pragma", "no-cache")
                .header("Expires", "0")
                .body(axum::body::Body::from(&gif_bytes[..]))
                .unwrap())
        }
        None => {
            warn!(
                tenant_id = %tenant_id,
                email_id = %email_id,
                "Email not found for open tracking"
            );
            Err(AppError::EmailNotFound)
        }
    }
}

pub async fn track_click(
    Path((tenant_id, email_id)): Path<(String, i64)>,
    Query(params): Query<ClickQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> AppResult<impl IntoResponse> {
    // Extract user agent and IP address
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    
    let ip_address = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());

    // Verify email exists and belongs to tenant
    let email = state.db.get_email(email_id, &tenant_id).await?;
    
    match email {
        Some(_) => {
            // Log the click event
            state.db.log_event(
                email_id,
                "click",
                user_agent.as_deref(),
                ip_address.as_deref(),
            ).await?;

            info!(
                tenant_id = %tenant_id,
                email_id = %email_id,
                url = %params.url,
                ip_address = ?ip_address,
                "Email link clicked"
            );

            // Redirect to the original URL
            Ok(Redirect::temporary(&params.url))
        }
        None => {
            warn!(
                tenant_id = %tenant_id,
                email_id = %email_id,
                "Email not found for click tracking"
            );
            Err(AppError::EmailNotFound)
        }
    }
}

pub async fn show_dashboard(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<impl IntoResponse> {
    // Ensure tenant exists (create if not)
    state.db.create_tenant(&tenant_id, &tenant_id).await?;

    // Get statistics for the tenant
    let stats = state.db.get_tenant_stats(&tenant_id).await?;
    
    let template = DashboardTemplate {
        tenant_id,
        stats,
        base_url: state.config.base_url.clone(),
    };
    
    let html = template.render()?;
    Ok(Html(html))
}

pub async fn create_email(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<CreateEmailRequest>,
) -> AppResult<impl IntoResponse> {
    // Ensure tenant exists (create if not)
    state.db.create_tenant(&tenant_id, &tenant_id).await?;

    // Create email record
    let email_id = state.db.create_email(
        &tenant_id,
        payload.subject.as_deref(),
        payload.recipient.as_deref(),
    ).await?;
    
    let tracking_pixel_url = format!(
        "{}/{}/pixel/{}.gif",
        state.config.base_url, tenant_id, email_id
    );
    
    let response = CreateEmailResponse {
        email_id,
        tracking_pixel_url,
    };
    
    info!(
        tenant_id = %tenant_id,
        email_id = %email_id,
        subject = ?payload.subject,
        recipient = ?payload.recipient,
        "Email record created"
    );
    
    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn get_click_url(
    Path((tenant_id, email_id)): Path<(String, i64)>,
    Query(mut params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> AppResult<impl IntoResponse> {
    let target_url = params.remove("url")
        .ok_or_else(|| AppError::InvalidUrl("Missing 'url' parameter".to_string()))?;

    // Verify email exists and belongs to tenant
    let email = state.db.get_email(email_id, &tenant_id).await?;
    
    match email {
        Some(_) => {
            let click_url = format!(
                "{}/{}/click/{}?url={}",
                state.config.base_url,
                tenant_id,
                email_id,
                urlencoding::encode(&target_url)
            );
            
            Ok(Json(serde_json::json!({
                "click_url": click_url,
                "original_url": target_url
            })))
        }
        None => Err(AppError::EmailNotFound)
    }
}

pub async fn create_app(db: Arc<Database>, config: Config) -> Router {
    let state = AppState { db, config };

    Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(metrics))
        .route("/:tenant_id/pixel/:email_id", get(track_open))
        .route("/:tenant_id/click/:email_id", get(track_click))
        .route("/:tenant_id/dashboard", get(show_dashboard))
        .route("/:tenant_id/emails", post(create_email))
        .route("/:tenant_id/click-url/:email_id", get(get_click_url))
        .layer(CorsLayer::permissive()) // Allow CORS for dashboard access
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .with_state(state)
}