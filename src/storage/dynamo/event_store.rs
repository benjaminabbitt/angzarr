//! DynamoDB EventStore implementation (placeholder).

use async_trait::async_trait;
use uuid::Uuid;

use crate::proto::EventPage;
use crate::storage::{EventStore, Result};

/// DynamoDB implementation of EventStore.
pub struct DynamoEventStore;

#[async_trait]
impl EventStore for DynamoEventStore {
    async fn add(
        &self,
        _domain: &str,
        _edition: &str,
        _root: Uuid,
        _events: Vec<EventPage>,
        _correlation_id: &str,
    ) -> Result<()> {
        todo!("DynamoEventStore::add")
    }

    async fn get(&self, _domain: &str, _edition: &str, _root: Uuid) -> Result<Vec<EventPage>> {
        todo!("DynamoEventStore::get")
    }

    async fn get_from(&self, _domain: &str, _edition: &str, _root: Uuid, _from: u32) -> Result<Vec<EventPage>> {
        todo!("DynamoEventStore::get_from")
    }

    async fn get_from_to(
        &self,
        _domain: &str,
        _edition: &str,
        _root: Uuid,
        _from: u32,
        _to: u32,
    ) -> Result<Vec<EventPage>> {
        todo!("DynamoEventStore::get_from_to")
    }

    async fn list_roots(&self, _domain: &str, _edition: &str) -> Result<Vec<Uuid>> {
        todo!("DynamoEventStore::list_roots")
    }

    async fn list_domains(&self) -> Result<Vec<String>> {
        todo!("DynamoEventStore::list_domains")
    }

    async fn get_next_sequence(&self, _domain: &str, _edition: &str, _root: Uuid) -> Result<u32> {
        todo!("DynamoEventStore::get_next_sequence")
    }

    async fn get_until_timestamp(
        &self,
        _domain: &str,
        _edition: &str,
        _root: Uuid,
        _until: &str,
    ) -> Result<Vec<EventPage>> {
        todo!("DynamoEventStore::get_until_timestamp")
    }

    async fn get_by_correlation(
        &self,
        _correlation_id: &str,
    ) -> Result<Vec<crate::proto::EventBook>> {
        todo!("DynamoEventStore::get_by_correlation")
    }

    async fn delete_edition_events(&self, _domain: &str, _edition: &str) -> Result<u32> {
        todo!("DynamoEventStore::delete_edition_events")
    }
}
