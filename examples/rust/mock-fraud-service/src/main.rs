//! Mock Fraud Service for testing external service integration.
//!
//! This service demonstrates deploying a mock external service for testing.
//! It can be deployed as:
//! - Standalone K8s Deployment (shared by all test pods)
//! - In-process server for unit/acceptance tests
//!
//! Configuration via environment variables:
//! - `PORT`: HTTP port (default: 8080)
//! - `FRAUD_RESPONSES`: Comma-separated customer_id=result pairs
//!   Example: "CUST-FRAUD=declined,CUST-REVIEW=review_required"
//!
//! This pattern applies to ANY external service mock (pricing, tax, address, etc.)

use std::collections::HashMap;
use std::sync::Arc;

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

/// Request from the fraud check client.
#[derive(Debug, Deserialize)]
struct FraudCheckRequest {
    customer_id: String,
    #[allow(dead_code)]
    amount_cents: i32,
    #[allow(dead_code)]
    payment_method: String,
}

/// Response to the fraud check client.
#[derive(Debug, Serialize, Deserialize)]
struct FraudCheckResponse {
    result: String,
}

/// Application state holding the configured responses.
struct AppState {
    /// customer_id -> result mapping
    responses: HashMap<String, String>,
}

impl AppState {
    /// Create from environment variable FRAUD_RESPONSES.
    ///
    /// Format: "customer_id1=result1,customer_id2=result2"
    /// Example: "CUST-FRAUD=declined,CUST-REVIEW=review_required"
    fn from_env() -> Self {
        let mut responses = HashMap::new();

        if let Ok(mappings) = std::env::var("FRAUD_RESPONSES") {
            for pair in mappings.split(',') {
                let parts: Vec<&str> = pair.split('=').collect();
                if parts.len() == 2 {
                    responses.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
                    info!(
                        customer_id = %parts[0].trim(),
                        result = %parts[1].trim(),
                        "configured fraud response"
                    );
                }
            }
        }

        Self { responses }
    }
}

/// Handle fraud check requests.
///
/// Returns the configured result for the customer_id, or "approved" by default.
async fn check_fraud(
    State(state): State<Arc<RwLock<AppState>>>,
    Json(req): Json<FraudCheckRequest>,
) -> (StatusCode, Json<FraudCheckResponse>) {
    let state = state.read().await;

    let result = state
        .responses
        .get(&req.customer_id)
        .cloned()
        .unwrap_or_else(|| "approved".to_string());

    info!(
        customer_id = %req.customer_id,
        result = %result,
        "fraud check processed"
    );

    (StatusCode::OK, Json(FraudCheckResponse { result }))
}

/// Health check endpoint.
async fn health() -> StatusCode {
    StatusCode::OK
}

#[tokio::main]
async fn main() {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting subscriber failed");

    // Load configuration
    let state = Arc::new(RwLock::new(AppState::from_env()));

    // Build router
    let app = Router::new()
        .route("/check", post(check_fraud))
        .route("/health", axum::routing::get(health))
        .with_state(state);

    // Start server
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);

    info!(addr = %addr, "starting mock fraud service");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn make_state(responses: Vec<(&str, &str)>) -> Arc<RwLock<AppState>> {
        let mut map = HashMap::new();
        for (k, v) in responses {
            map.insert(k.to_string(), v.to_string());
        }
        Arc::new(RwLock::new(AppState { responses: map }))
    }

    #[tokio::test]
    async fn test_check_fraud_returns_configured_result() {
        let state = make_state(vec![("CUST-BAD", "declined")]);
        let app = Router::new()
            .route("/check", post(check_fraud))
            .with_state(state);

        let req = Request::builder()
            .method("POST")
            .uri("/check")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"customer_id":"CUST-BAD","amount_cents":1000,"payment_method":"card"}"#,
            ))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 1000)
            .await
            .unwrap();
        let resp: FraudCheckResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp.result, "declined");
    }

    #[tokio::test]
    async fn test_check_fraud_returns_approved_by_default() {
        let state = make_state(vec![]);
        let app = Router::new()
            .route("/check", post(check_fraud))
            .with_state(state);

        let req = Request::builder()
            .method("POST")
            .uri("/check")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"customer_id":"CUST-OK","amount_cents":500,"payment_method":"cash"}"#,
            ))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 1000)
            .await
            .unwrap();
        let resp: FraudCheckResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp.result, "approved");
    }
}
