//! HTTP client for external fraud check service.
//!
//! This module demonstrates calling ANY external REST/gRPC service from an aggregate.
//! The fraud service is just one example - aggregates may call:
//! - Pricing services (get current prices, apply dynamic pricing)
//! - Tax services (calculate taxes by jurisdiction)
//! - Address validation services
//! - Payment gateways (pre-authorization, card verification)
//! - Customer services (loyalty status, preferences)
//! - Analytics/ML services (recommendations, predictions)
//! - Inventory services (real-time availability)
//! - Notification services (send confirmations)

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Result of a fraud check from the external service.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FraudCheckResult {
    Approved,
    ReviewRequired,
    Declined,
}

impl fmt::Display for FraudCheckResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FraudCheckResult::Approved => write!(f, "approved"),
            FraudCheckResult::ReviewRequired => write!(f, "review_required"),
            FraudCheckResult::Declined => write!(f, "declined"),
        }
    }
}

impl Default for FraudCheckResult {
    fn default() -> Self {
        FraudCheckResult::Approved
    }
}

/// Error from fraud service calls.
#[derive(Debug, thiserror::Error)]
pub enum FraudError {
    #[error("network error: {0}")]
    NetworkError(String),
    #[error("service error: {0}")]
    ServiceError(String),
    #[error("parse error: {0}")]
    ParseError(String),
}

/// Request sent to the fraud check service.
#[derive(Serialize)]
struct FraudCheckRequest {
    customer_id: String,
    amount_cents: i32,
    payment_method: String,
}

/// Response from the fraud check service.
#[derive(Deserialize)]
struct FraudCheckResponse {
    result: FraudCheckResult,
}

/// HTTP client for the fraud check service.
///
/// This is an example of calling an external REST service from an aggregate.
/// The pattern can be applied to any external service: pricing, tax, address
/// validation, payment gateways, analytics, etc.
pub struct FraudServiceClient {
    client: Client,
    base_url: String,
}

impl FraudServiceClient {
    /// Create a new fraud service client.
    ///
    /// # Arguments
    /// * `base_url` - Base URL of the fraud service (e.g., "http://fraud-service:8080")
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
        }
    }

    /// Check a transaction for fraud.
    ///
    /// # Arguments
    /// * `customer_id` - Customer identifier
    /// * `amount_cents` - Transaction amount in cents
    /// * `payment_method` - Payment method (e.g., "card", "cash")
    ///
    /// # Returns
    /// The fraud check result: Approved, ReviewRequired, or Declined
    pub async fn check(
        &self,
        customer_id: &str,
        amount_cents: i32,
        payment_method: &str,
    ) -> Result<FraudCheckResult, FraudError> {
        let request = FraudCheckRequest {
            customer_id: customer_id.to_string(),
            amount_cents,
            payment_method: payment_method.to_string(),
        };

        let response = self
            .client
            .post(format!("{}/check", self.base_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| FraudError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(FraudError::ServiceError(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let body: FraudCheckResponse = response
            .json()
            .await
            .map_err(|e| FraudError::ParseError(e.to_string()))?;

        Ok(body.result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fraud_check_result_display() {
        assert_eq!(FraudCheckResult::Approved.to_string(), "approved");
        assert_eq!(
            FraudCheckResult::ReviewRequired.to_string(),
            "review_required"
        );
        assert_eq!(FraudCheckResult::Declined.to_string(), "declined");
    }
}
