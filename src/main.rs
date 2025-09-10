use envy;
use little_bell::{create_app, database::Database, Config};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Load configuration from environment
    let config = match envy::from_env::<Config>() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            Config::default()
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
    let db = match Database::new(db_path).await {
        Ok(db) => Arc::new(db),
        Err(e) => {
            eprintln!("Failed to initialize database: {}", e);
            std::process::exit(1);
        }
    };

    // Create the application
    let app = create_app(db, config.clone()).await;

    // Start the server
    let bind_addr = format!("0.0.0.0:{}", config.port);
    println!("Starting Little Bell Email Tracking Server on {}", bind_addr);
    println!("Base URL: {}", config.base_url);
    println!("Database: {}", config.database_url);

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