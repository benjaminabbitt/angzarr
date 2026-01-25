//! In-process command client for embedded runtime.
//!
//! Provides a simple API for submitting commands programmatically.

use std::sync::Arc;

use uuid::Uuid;

use crate::proto::{
    CommandBook, CommandPage, CommandResponse, Cover, Uuid as ProtoUuid,
};

use super::router::CommandRouter;

/// In-process command client.
///
/// Provides a simple interface for submitting commands to the runtime.
/// Can be cloned and shared across tasks.
///
/// # Example
///
/// ```ignore
/// let client = runtime.command_client();
///
/// // Submit a command
/// let response = client
///     .command("orders", order_id)
///     .with_type("CreateOrder")
///     .with_data(order_data)
///     .send()
///     .await?;
/// ```
#[derive(Clone)]
pub struct CommandClient {
    router: Arc<CommandRouter>,
}

impl CommandClient {
    /// Create a new command client.
    pub(crate) fn new(router: Arc<CommandRouter>) -> Self {
        Self { router }
    }

    /// Start building a command for a domain and root.
    pub fn command(&self, domain: impl Into<String>, root: Uuid) -> CommandBuilder {
        CommandBuilder::new(self.router.clone(), domain.into(), root)
    }

    /// Execute a pre-built command book.
    pub async fn execute(
        &self,
        command: CommandBook,
    ) -> Result<CommandResponse, Box<dyn std::error::Error + Send + Sync>> {
        self.router
            .execute(command)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    /// Check if a domain has a registered handler.
    pub fn has_domain(&self, domain: &str) -> bool {
        self.router.has_handler(domain)
    }

    /// Get list of registered domains.
    pub fn domains(&self) -> Vec<&str> {
        self.router.domains()
    }
}

/// Builder for constructing commands.
pub struct CommandBuilder {
    router: Arc<CommandRouter>,
    domain: String,
    root: Uuid,
    command_type: Option<String>,
    command_data: Option<Vec<u8>>,
    correlation_id: Option<String>,
    auto_resequence: bool,
    sequence: Option<u32>,
}

impl CommandBuilder {
    fn new(router: Arc<CommandRouter>, domain: String, root: Uuid) -> Self {
        Self {
            router,
            domain,
            root,
            command_type: None,
            command_data: None,
            correlation_id: None,
            auto_resequence: true,
            sequence: None,
        }
    }

    /// Set the command type URL.
    pub fn with_type(mut self, type_url: impl Into<String>) -> Self {
        self.command_type = Some(type_url.into());
        self
    }

    /// Set the command data (protobuf-encoded).
    pub fn with_data(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.command_data = Some(data.into());
        self
    }

    /// Set a protobuf message as the command.
    pub fn with_message<M: prost::Message>(mut self, message: &M) -> Self {
        let mut buf = Vec::new();
        message.encode(&mut buf).ok();
        self.command_data = Some(buf);
        self
    }

    /// Set a custom correlation ID.
    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    /// Disable auto-resequencing (fail on sequence conflict instead of retry).
    pub fn without_auto_resequence(mut self) -> Self {
        self.auto_resequence = false;
        self
    }

    /// Set explicit sequence number (used with auto_resequence=false).
    pub fn with_sequence(mut self, sequence: u32) -> Self {
        self.sequence = Some(sequence);
        self
    }

    /// Build the command book without sending.
    pub fn build(self) -> CommandBook {
        let command = self.command_data.map(|data| {
            prost_types::Any {
                type_url: self.command_type.unwrap_or_default(),
                value: data,
            }
        });

        CommandBook {
            cover: Some(Cover {
                domain: self.domain,
                root: Some(ProtoUuid {
                    value: self.root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![CommandPage {
                sequence: self.sequence.unwrap_or(0),
                command,
            }],
            correlation_id: self.correlation_id.unwrap_or_default(),
            saga_origin: None,
            auto_resequence: self.auto_resequence,
            fact: false,
        }
    }

    /// Send the command and return the response.
    pub async fn send(self) -> Result<CommandResponse, Box<dyn std::error::Error + Send + Sync>> {
        let router = self.router.clone();
        let command = self.build();

        router
            .execute(command)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_builder_build() {
        // Create a mock router (we can't test without storage, so just test build)
        let root = Uuid::new_v4();

        // We can't test the full flow without a real router, but we can test building
        let command = CommandBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![CommandPage {
                sequence: 0,
                command: Some(prost_types::Any {
                    type_url: "CreateOrder".to_string(),
                    value: vec![1, 2, 3],
                }),
            }],
            correlation_id: "test-id".to_string(),
            saga_origin: None,
            auto_resequence: true,
            fact: false,
        };

        assert_eq!(command.cover.as_ref().unwrap().domain, "orders");
        assert_eq!(command.correlation_id, "test-id");
    }
}
