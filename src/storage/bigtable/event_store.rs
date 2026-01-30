//! Bigtable EventStore implementation (placeholder).

use async_trait::async_trait;
use uuid::Uuid;

use crate::proto::EventPage;
use crate::storage::{EventStore, Result};

/// Bigtable implementation of EventStore.
pub struct BigtableEventStore;

#[async_trait]
impl EventStore for BigtableEventStore {
    async fn add(
        &self,
        _domain: &str,
        _root: Uuid,
        _events: Vec<EventPage>,
        _correlation_id: &str,
    ) -> Result<()> {
        todo!("BigtableEventStore::add")
    }

    async fn get(&self, _domain: &str, _root: Uuid) -> Result<Vec<EventPage>> {
        todo!("BigtableEventStore::get")
    }

    async fn get_from(&self, _domain: &str, _root: Uuid, _from: u32) -> Result<Vec<EventPage>> {
        todo!("BigtableEventStore::get_from")
    }

    async fn get_from_to(
        &self,
        _domain: &str,
        _root: Uuid,
        _from: u32,
        _to: u32,
    ) -> Result<Vec<EventPage>> {
        todo!("BigtableEventStore::get_from_to")
    }

    async fn list_roots(&self, _domain: &str) -> Result<Vec<Uuid>> {
        todo!("BigtableEventStore::list_roots")
    }

    async fn list_domains(&self) -> Result<Vec<String>> {
        todo!("BigtableEventStore::list_domains")
    }

    async fn get_next_sequence(&self, _domain: &str, _root: Uuid) -> Result<u32> {
        todo!("BigtableEventStore::get_next_sequence")
    }

    async fn get_until_timestamp(
        &self,
        _domain: &str,
        _root: Uuid,
        _until: &str,
    ) -> Result<Vec<EventPage>> {
        todo!("BigtableEventStore::get_until_timestamp")
    }

    async fn get_by_correlation(
        &self,
        _correlation_id: &str,
    ) -> Result<Vec<crate::proto::EventBook>> {
        todo!("BigtableEventStore::get_by_correlation")
    }
}
