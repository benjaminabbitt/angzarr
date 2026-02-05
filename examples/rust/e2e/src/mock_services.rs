//! In-process mock servers for external service integration tests.
//!
//! These mock servers run within the test process on random available ports,
//! allowing acceptance tests to verify external service integration without
//! requiring actual external services or K8s deployments.
//!
//! This pattern demonstrates how to test aggregate handlers that call
//! external services (fraud check, pricing, tax, address validation, etc.).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

/// Request format for fraud check API.
#[derive(Debug, Deserialize)]
pub struct FraudCheckRequest {
    pub customer_id: String,
    #[allow(dead_code)]
    pub amount_cents: i32,
    #[allow(dead_code)]
    pub payment_method: String,
}

/// Response format for fraud check API.
#[derive(Debug, Serialize, Deserialize)]
pub struct FraudCheckResponse {
    pub result: String,
}

/// In-process mock fraud check service for testing.
///
/// Runs an axum HTTP server on a random port. Configure responses
/// per customer_id. Customers not configured return "approved" by default.
pub struct MockFraudServer {
    /// Map of customer_id -> result
    responses: Arc<RwLock<HashMap<String, String>>>,
    /// Server handle (kept alive to prevent shutdown)
    _handle: JoinHandle<()>,
    /// Server address
    addr: SocketAddr,
}

impl MockFraudServer {
    /// Create and start a new mock fraud server on a random available port.
    ///
    /// Default response is "approved" for any customer_id not explicitly configured.
    pub async fn start() -> Self {
        Self::start_with_responses(HashMap::new()).await
    }

    /// Create and start with pre-configured responses.
    ///
    /// # Arguments
    /// * `responses` - Map of customer_id -> result ("approved", "declined", "review_required")
    pub async fn start_with_responses(responses: HashMap<String, String>) -> Self {
        let responses = Arc::new(RwLock::new(responses));
        let state = responses.clone();

        // Bind to port 0 to get a random available port
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind mock fraud server");
        let addr = listener.local_addr().expect("Failed to get local address");

        let app = Router::new()
            .route("/check", post(handle_check))
            .route("/health", axum::routing::get(handle_health))
            .with_state(state);

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("Mock fraud server failed");
        });

        Self {
            responses,
            _handle: handle,
            addr,
        }
    }

    /// Get the base URL for this server (e.g., "http://127.0.0.1:12345")
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Configure a response for a specific customer_id.
    ///
    /// # Arguments
    /// * `customer_id` - The customer ID to configure
    /// * `result` - The result to return: "approved", "declined", or "review_required"
    pub async fn set_response(&self, customer_id: &str, result: &str) {
        self.responses
            .write()
            .await
            .insert(customer_id.to_string(), result.to_string());
    }

    /// Configure multiple responses at once.
    pub async fn set_responses(&self, responses: Vec<(&str, &str)>) {
        let mut map = self.responses.write().await;
        for (customer_id, result) in responses {
            map.insert(customer_id.to_string(), result.to_string());
        }
    }
}

/// Handle fraud check requests.
async fn handle_check(
    State(responses): State<Arc<RwLock<HashMap<String, String>>>>,
    Json(req): Json<FraudCheckRequest>,
) -> (StatusCode, Json<FraudCheckResponse>) {
    let responses = responses.read().await;
    let result = responses
        .get(&req.customer_id)
        .cloned()
        .unwrap_or_else(|| "approved".to_string());

    (StatusCode::OK, Json(FraudCheckResponse { result }))
}

/// Health check endpoint.
async fn handle_health() -> StatusCode {
    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_fraud_server_default_approved() {
        let server = MockFraudServer::start().await;
        let client = reqwest::Client::new();

        let response = client
            .post(format!("{}/check", server.url()))
            .json(&serde_json::json!({
                "customer_id": "CUST-ANY",
                "amount_cents": 1000,
                "payment_method": "card"
            }))
            .send()
            .await
            .expect("Request failed");

        let body: FraudCheckResponse = response.json().await.expect("Failed to parse");
        assert_eq!(body.result, "approved");
    }

    #[tokio::test]
    async fn test_mock_fraud_server_configured_response() {
        let mut responses = HashMap::new();
        responses.insert("CUST-BAD".to_string(), "declined".to_string());
        let server = MockFraudServer::start_with_responses(responses).await;
        let client = reqwest::Client::new();

        let response = client
            .post(format!("{}/check", server.url()))
            .json(&serde_json::json!({
                "customer_id": "CUST-BAD",
                "amount_cents": 100000,
                "payment_method": "card"
            }))
            .send()
            .await
            .expect("Request failed");

        let body: FraudCheckResponse = response.json().await.expect("Failed to parse");
        assert_eq!(body.result, "declined");
    }

    #[tokio::test]
    async fn test_mock_fraud_server_dynamic_config() {
        let server = MockFraudServer::start().await;

        // Initially approved (default)
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/check", server.url()))
            .json(&serde_json::json!({
                "customer_id": "CUST-DYNAMIC",
                "amount_cents": 500,
                "payment_method": "cash"
            }))
            .send()
            .await
            .expect("Request failed");

        let body: FraudCheckResponse = response.json().await.expect("Failed to parse");
        assert_eq!(body.result, "approved");

        // Configure to decline
        server.set_response("CUST-DYNAMIC", "declined").await;

        let response = client
            .post(format!("{}/check", server.url()))
            .json(&serde_json::json!({
                "customer_id": "CUST-DYNAMIC",
                "amount_cents": 500,
                "payment_method": "cash"
            }))
            .send()
            .await
            .expect("Request failed");

        let body: FraudCheckResponse = response.json().await.expect("Failed to parse");
        assert_eq!(body.result, "declined");
    }
}
