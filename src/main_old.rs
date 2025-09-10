use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use envy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;

mod database;
use database::{Database, EventStats};

#[derive(Debug, Deserialize, Clone)]
struct Config {
    #[serde(default = "default_port")]
    port: u16,
    #[serde(default = "default_database_url")]
    database_url: String,
    #[serde(default = "default_base_url")]
    base_url: String,
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

#[derive(Clone)]
struct AppState {
    db: Arc<Database>,
    config: Config,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    tenant_id: String,
    stats: EventStats,
    base_url: String,
}

#[derive(Deserialize)]
struct ClickQuery {
    url: String,
}

#[derive(Deserialize, Serialize)]
struct CreateEmailRequest {
    subject: Option<String>,
    recipient: Option<String>,
}

#[derive(Serialize)]
struct CreateEmailResponse {
    email_id: i64,
    tracking_pixel_url: String,
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "little-bell",
        "version": "0.1.0"
    }))
}

async fn track_open(
    Path((tenant_id, email_id_str)): Path<(String, String)>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Extract email ID from the path (remove .gif extension)
    let email_id_str = email_id_str.strip_suffix(".gif").unwrap_or(&email_id_str);
    let email_id = match email_id_str.parse::<i64>() {
        Ok(id) => id,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

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
    match state.db.get_email(email_id, &tenant_id).await {
        Ok(Some(_)) => {
            // Log the open event
            if let Err(e) = state.db.log_event(
                email_id,
                "open",
                user_agent.as_deref(),
                ip_address.as_deref(),
            ).await {
                eprintln!("Failed to log open event: {}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }

            // Return 1x1 transparent GIF
            let gif_bytes = include_bytes!("pixel.gif");
            Response::builder()
                .header("Content-Type", "image/gif")
                .header("Cache-Control", "no-store, no-cache, must-revalidate")
                .header("Pragma", "no-cache")
                .header("Expires", "0")
                .body(axum::body::Body::from(&gif_bytes[..]))
                .unwrap()
                .into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            eprintln!("Database error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn track_click(
    Path((tenant_id, email_id)): Path<(String, i64)>,
    Query(params): Query<ClickQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
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
    match state.db.get_email(email_id, &tenant_id).await {
        Ok(Some(_)) => {
            // Log the click event
            if let Err(e) = state.db.log_event(
                email_id,
                "click",
                user_agent.as_deref(),
                ip_address.as_deref(),
            ).await {
                eprintln!("Failed to log click event: {}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }

            // Redirect to the original URL
            Redirect::temporary(&params.url).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            eprintln!("Database error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn show_dashboard(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Ensure tenant exists (create if not)
    if let Err(e) = state.db.create_tenant(&tenant_id, &tenant_id).await {
        eprintln!("Failed to create/ensure tenant: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // Get statistics for the tenant
    match state.db.get_tenant_stats(&tenant_id).await {
        Ok(stats) => {
            let template = DashboardTemplate {
                tenant_id,
                stats,
                base_url: state.config.base_url.clone(),
            };
            match template.render() {
                Ok(html) => Html(html).into_response(),
                Err(e) => {
                    eprintln!("Template render error: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        Err(e) => {
            eprintln!("Database error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn create_email(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<CreateEmailRequest>,
) -> impl IntoResponse {
    // Ensure tenant exists (create if not)
    if let Err(e) = state.db.create_tenant(&tenant_id, &tenant_id).await {
        eprintln!("Failed to create/ensure tenant: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // Create email record
    match state.db.create_email(
        &tenant_id,
        payload.subject.as_deref(),
        payload.recipient.as_deref(),
    ).await {
        Ok(email_id) => {
            let tracking_pixel_url = format!(
                "{}/{}/pixel/{}.gif",
                state.config.base_url, tenant_id, email_id
            );
            
            let response = CreateEmailResponse {
                email_id,
                tracking_pixel_url,
            };
            
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => {
            eprintln!("Failed to create email: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn get_click_url(
    Path((tenant_id, email_id)): Path<(String, i64)>,
    Query(mut params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let target_url = match params.remove("url") {
        Some(url) => url,
        None => return (StatusCode::BAD_REQUEST, "Missing 'url' parameter").into_response(),
    };

    // Verify email exists and belongs to tenant
    match state.db.get_email(email_id, &tenant_id).await {
        Ok(Some(_)) => {
            let click_url = format!(
                "{}/{}/click/{}?url={}",
                state.config.base_url,
                tenant_id,
                email_id,
                urlencoding::encode(&target_url)
            );
            Json(serde_json::json!({
                "click_url": click_url,
                "original_url": target_url
            })).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            eprintln!("Database error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[tokio::main]
async fn main() {
    // Load configuration from environment
    let config = match envy::from_env::<Config>() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            Config {
                port: default_port(),
                database_url: default_database_url(),
                base_url: default_base_url(),
            }
        }
    };

    // Ensure data directory exists
    let db_path = config.database_url.strip_prefix("sqlite:").unwrap_or(&config.database_url);
    if let Some(parent) = std::path::Path::new(db_path).parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("Failed to create data directory: {}", e);
            std::process::exit(1);
        }
    }

    // Initialize database
    let db_path = config.database_url.strip_prefix("sqlite:").unwrap_or(&config.database_url);
    let db = match Database::new(db_path).await {
        Ok(db) => Arc::new(db),
        Err(e) => {
            eprintln!("Failed to initialize database: {}", e);
            std::process::exit(1);
        }
    };

    let state = AppState { db, config };

    // Build the application router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/:tenant_id/pixel/:email_id", get(track_open))
        .route("/:tenant_id/click/:email_id", get(track_click))
        .route("/:tenant_id/dashboard", get(show_dashboard))
        .route("/:tenant_id/emails", post(create_email))
        .route("/:tenant_id/click-url/:email_id", get(get_click_url))
        .layer(CompressionLayer::new())
        .with_state(state.clone());

    // Start the server
    let bind_addr = format!("0.0.0.0:{}", state.config.port);
    println!("Starting Little Bell Email Tracking Server on {}", bind_addr);
    println!("Base URL: {}", state.config.base_url);
    println!("Database: {}", state.config.database_url);

    let listener = match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("Failed to bind to address {}: {}", bind_addr, e);
            std::process::exit(1);
        }
    };

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}
