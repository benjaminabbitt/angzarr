//! Static business logic client with hardcoded addresses.

use async_trait::async_trait;
use std::collections::HashMap;
use tonic::transport::Channel;

use crate::interfaces::business_client::{BusinessError, BusinessLogicClient, Result};
use crate::proto::{
    business_logic_client::BusinessLogicClient as GrpcClient, ContextualCommand, EventBook,
};

/// Business logic client with static address configuration.
///
/// Each domain maps to a specific gRPC address.
pub struct StaticBusinessLogicClient {
    clients: HashMap<String, GrpcClient<Channel>>,
}

impl StaticBusinessLogicClient {
    /// Create a new static client from domain -> address mappings.
    pub async fn new(addresses: HashMap<String, String>) -> Result<Self> {
        let mut clients = HashMap::new();

        for (domain, address) in addresses {
            let channel = Channel::from_shared(address.clone())
                .map_err(|e| BusinessError::Connection {
                    domain: domain.clone(),
                    message: e.to_string(),
                })?
                .connect()
                .await
                .map_err(|e| BusinessError::Connection {
                    domain: domain.clone(),
                    message: e.to_string(),
                })?;

            clients.insert(domain, GrpcClient::new(channel));
        }

        Ok(Self { clients })
    }
}

#[async_trait]
impl BusinessLogicClient for StaticBusinessLogicClient {
    async fn handle(&self, domain: &str, cmd: ContextualCommand) -> Result<EventBook> {
        let client = self
            .clients
            .get(domain)
            .ok_or_else(|| BusinessError::DomainNotFound(domain.to_string()))?;

        let mut client = client.clone();
        let response = client.handle(cmd).await?;

        Ok(response.into_inner())
    }

    fn has_domain(&self, domain: &str) -> bool {
        self.clients.contains_key(domain)
    }

    fn domains(&self) -> Vec<String> {
        self.clients.keys().cloned().collect()
    }
}
