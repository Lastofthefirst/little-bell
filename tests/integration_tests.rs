use axum::http::StatusCode;
use axum_test::TestServer;
use serde_json::{json, Value};
use std::sync::Arc;
use little_bell::{create_app, database::Database, Config};

async fn create_test_app() -> TestServer {
    // Use in-memory SQLite database for testing
    let db = Database::new(":memory:").await.expect("Failed to create test database");
    let config = Config {
        port: 3000,
        database_url: "sqlite::memory:".to_string(),
        base_url: "http://localhost:3000".to_string(),
    };
    
    let app = create_app(Arc::new(db), config).await;
    TestServer::new(app).expect("Failed to create test server")
}

#[tokio::test]
async fn test_health_check() {
    let server = create_test_app().await;
    
    let response = server.get("/health").await;
    response.assert_status_ok();
    
    let json: Value = response.json();
    assert_eq!(json["status"], "healthy");
    assert_eq!(json["service"], "little-bell");
}

#[tokio::test]
async fn test_create_email() {
    let server = create_test_app().await;
    
    let payload = json!({
        "subject": "Test Email",
        "recipient": "test@example.com"
    });
    
    let response = server.post("/test_tenant/emails")
        .json(&payload)
        .await;
    
    response.assert_status(StatusCode::CREATED);
    
    let json: Value = response.json();
    assert!(json["email_id"].is_number());
    assert!(json["tracking_pixel_url"].as_str().unwrap().contains("/test_tenant/pixel/"));
}

#[tokio::test]
async fn test_pixel_tracking() {
    let server = create_test_app().await;
    
    // First create an email
    let payload = json!({
        "subject": "Test Email",
        "recipient": "test@example.com"
    });
    
    let email_response = server.post("/test_tenant/emails")
        .json(&payload)
        .await;
    
    let email_json: Value = email_response.json();
    let email_id = email_json["email_id"].as_i64().unwrap();
    
    // Now track the pixel
    let pixel_response = server.get(&format!("/test_tenant/pixel/{}.gif", email_id)).await;
    pixel_response.assert_status_ok();
    
    // Check content type
    assert_eq!(pixel_response.headers()["content-type"], "image/gif");
}

#[tokio::test]
async fn test_click_tracking() {
    let server = create_test_app().await;
    
    // First create an email
    let payload = json!({
        "subject": "Test Email",
        "recipient": "test@example.com"
    });
    
    let email_response = server.post("/test_tenant/emails")
        .json(&payload)
        .await;
    
    let email_json: Value = email_response.json();
    let email_id = email_json["email_id"].as_i64().unwrap();
    
    // Now track a click
    let target_url = "https://example.com";
    let click_response = server.get(&format!("/test_tenant/click/{}?url={}", email_id, urlencoding::encode(target_url))).await;
    
    // Should redirect
    assert_eq!(click_response.status_code(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(click_response.headers()["location"], target_url);
}

#[tokio::test]
async fn test_get_click_url() {
    let server = create_test_app().await;
    
    // First create an email
    let payload = json!({
        "subject": "Test Email",
        "recipient": "test@example.com"
    });
    
    let email_response = server.post("/test_tenant/emails")
        .json(&payload)
        .await;
    
    let email_json: Value = email_response.json();
    let email_id = email_json["email_id"].as_i64().unwrap();
    
    // Get click URL
    let target_url = "https://example.com";
    let response = server.get(&format!("/test_tenant/click-url/{}?url={}", email_id, urlencoding::encode(target_url))).await;
    
    response.assert_status_ok();
    
    let json: Value = response.json();
    assert!(json["click_url"].as_str().unwrap().contains(&format!("/test_tenant/click/{}", email_id)));
    assert_eq!(json["original_url"], target_url);
}

#[tokio::test]
async fn test_dashboard() {
    let server = create_test_app().await;
    
    let response = server.get("/test_tenant/dashboard").await;
    response.assert_status_ok();
    
    // Should return HTML
    assert!(response.headers()["content-type"].to_str().unwrap().contains("text/html"));
}

#[tokio::test]
async fn test_email_not_found() {
    let server = create_test_app().await;
    
    // Try to track a non-existent email
    let response = server.get("/test_tenant/pixel/999.gif").await;
    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_invalid_email_id() {
    let server = create_test_app().await;
    
    // Try to track with invalid email ID
    let response = server.get("/test_tenant/pixel/invalid.gif").await;
    response.assert_status(StatusCode::BAD_REQUEST);
}