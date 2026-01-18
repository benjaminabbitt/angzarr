//! Mock business logic client for testing.

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::{BusinessError, BusinessLogicClient, Result};
use crate::proto::{business_response, BusinessResponse, ContextualCommand, EventBook, EventPage};

/// Mock business logic client for testing.
pub struct MockBusinessLogic {
    domains: Vec<String>,
    fail_on_handle: RwLock<bool>,
    reject_command: RwLock<bool>,
    return_snapshot: RwLock<bool>,
}

impl MockBusinessLogic {
    pub fn new(domains: Vec<String>) -> Self {
        Self {
            domains,
            fail_on_handle: RwLock::new(false),
            reject_command: RwLock::new(false),
            return_snapshot: RwLock::new(false),
        }
    }

    pub async fn set_fail_on_handle(&self, fail: bool) {
        *self.fail_on_handle.write().await = fail;
    }

    pub async fn set_reject_command(&self, reject: bool) {
        *self.reject_command.write().await = reject;
    }

    pub async fn set_return_snapshot(&self, return_snapshot: bool) {
        *self.return_snapshot.write().await = return_snapshot;
    }
}

#[async_trait]
impl BusinessLogicClient for MockBusinessLogic {
    async fn handle(&self, domain: &str, cmd: ContextualCommand) -> Result<BusinessResponse> {
        if *self.fail_on_handle.read().await {
            return Err(BusinessError::Connection {
                domain: domain.to_string(),
                message: "Mock connection failure".to_string(),
            });
        }

        if *self.reject_command.read().await {
            return Err(BusinessError::Rejected("Mock rejection".to_string()));
        }

        if !self.has_domain(domain) {
            return Err(BusinessError::DomainNotFound(domain.to_string()));
        }

        // Generate a simple event from the command
        let cover = cmd.command.and_then(|c| c.cover);
        let prior_seq = cmd
            .events
            .as_ref()
            .map(|e| e.pages.len() as u32)
            .unwrap_or(0);

        // Optionally include snapshot state (framework computes sequence)
        let snapshot_state = if *self.return_snapshot.read().await {
            Some(prost_types::Any {
                type_url: "test.MockState".to_string(),
                value: vec![1, 2, 3],
            })
        } else {
            None
        };

        Ok(BusinessResponse {
            result: Some(business_response::Result::Events(EventBook {
                cover,
                pages: vec![EventPage {
                    sequence: Some(crate::proto::event_page::Sequence::Num(prior_seq)),
                    event: Some(prost_types::Any {
                        type_url: "test.MockEvent".to_string(),
                        value: vec![],
                    }),
                    created_at: None,
                }],
                snapshot: None, // Framework-populated on load, not set by business logic
                correlation_id: String::new(),
                snapshot_state,
            })),
        })
    }

    fn has_domain(&self, domain: &str) -> bool {
        self.domains.contains(&domain.to_string())
    }

    fn domains(&self) -> Vec<String> {
        self.domains.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_business_logic_handle() {
        let logic = MockBusinessLogic::new(vec!["orders".to_string()]);

        let cmd = ContextualCommand {
            events: None,
            command: None,
        };

        let result = logic.handle("orders", cmd).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_business_logic_domain_not_found() {
        let logic = MockBusinessLogic::new(vec!["orders".to_string()]);

        let cmd = ContextualCommand {
            events: None,
            command: None,
        };

        let result = logic.handle("unknown", cmd).await;
        assert!(matches!(result, Err(BusinessError::DomainNotFound(_))));
    }

    #[tokio::test]
    async fn test_mock_business_logic_fail_on_handle() {
        let logic = MockBusinessLogic::new(vec!["orders".to_string()]);
        logic.set_fail_on_handle(true).await;

        let cmd = ContextualCommand {
            events: None,
            command: None,
        };

        let result = logic.handle("orders", cmd).await;
        assert!(matches!(result, Err(BusinessError::Connection { .. })));
    }

    #[tokio::test]
    async fn test_mock_business_logic_reject_command() {
        let logic = MockBusinessLogic::new(vec!["orders".to_string()]);
        logic.set_reject_command(true).await;

        let cmd = ContextualCommand {
            events: None,
            command: None,
        };

        let result = logic.handle("orders", cmd).await;
        assert!(matches!(result, Err(BusinessError::Rejected(_))));
    }
}
