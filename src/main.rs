use little_bell::{create_app, database::Database, Config, error::AppError};
use std::sync::Arc;
use tracing::{info, error};
use tracing_subscriber::EnvFilter;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("little_bell=info".parse().unwrap()))
        .init();

    info!("Starting Little Bell Email Tracking Server");

    // Load configuration from environment
    let config = match envy::from_env::<Config>() {
        Ok(config) => {
            info!("Configuration loaded successfully");
            config
        },
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            info!("Using default configuration");
            Config::default()
        }
    };

    // Ensure data directory exists
    let db_path = config.database_url.strip_prefix("sqlite:").unwrap_or(&config.database_url);
    if let Some(parent) = std::path::Path::new(db_path).parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            error!("Failed to create data directory: {}", e);
            return Err(AppError::Io(e));
        }
    }

    // Initialize database
    let db = match Database::new(db_path).await {
        Ok(db) => {
            info!("Database initialized successfully: {}", db_path);
            Arc::new(db)
        },
        Err(e) => {
            error!("Failed to initialize database: {}", e);
            return Err(AppError::Database(e));
        }
    };

    // Create the application
    let app = create_app(db, config.clone()).await;

    // Start the server
    let bind_addr = format!("0.0.0.0:{}", config.port);
    info!("Server configuration:");
    info!("  Bind address: {}", bind_addr);
    info!("  Base URL: {}", config.base_url);
    info!("  Database: {}", config.database_url);

    let listener = match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(listener) => {
            info!("Server listening on {}", bind_addr);
            listener
        },
        Err(e) => {
            error!("Failed to bind to address {}: {}", bind_addr, e);
            return Err(AppError::Io(e));
        }
    };

    info!("Little Bell Email Tracking Server started successfully");
    
    // Setup graceful shutdown
    let server = axum::serve(listener, app);
    
    // Create shutdown signal
    let shutdown_signal = async {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                info!("Received Ctrl+C, shutting down gracefully...");
            },
            _ = terminate => {
                info!("Received SIGTERM, shutting down gracefully...");
            },
        }
    };

    // Run server with graceful shutdown
    if let Err(e) = server.with_graceful_shutdown(shutdown_signal).await {
        error!("Server error: {}", e);
        return Err(AppError::Io(e));
    }

    info!("Server shut down gracefully");
    Ok(())
}