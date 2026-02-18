//! Page extension traits for EventPage and CommandPage.
//!
//! Provides convenient accessors for sequence, type URL, and payload decoding.

use crate::proto::{CommandPage, EventPage, MergeStrategy};

/// Extension trait for EventPage proto type.
///
/// Provides convenient accessors for sequence, type URL, and payload decoding.
pub trait EventPageExt {
    /// Get the sequence number from this page.
    fn sequence_num(&self) -> u32;

    /// Get the type URL of the event, if present.
    fn type_url(&self) -> Option<&str>;

    /// Get the raw payload bytes, if present.
    fn payload(&self) -> Option<&[u8]>;

    /// Decode the event payload as a specific message type.
    ///
    /// Returns None if the event is missing, type URL doesn't match the suffix,
    /// or decoding fails.
    fn decode<M: prost::Message + Default>(&self, type_suffix: &str) -> Option<M>;
}

impl EventPageExt for EventPage {
    fn sequence_num(&self) -> u32 {
        self.sequence
    }

    fn type_url(&self) -> Option<&str> {
        match &self.payload {
            Some(crate::proto::event_page::Payload::Event(e)) => Some(e.type_url.as_str()),
            _ => None,
        }
    }

    fn payload(&self) -> Option<&[u8]> {
        match &self.payload {
            Some(crate::proto::event_page::Payload::Event(e)) => Some(e.value.as_slice()),
            _ => None,
        }
    }

    fn decode<M: prost::Message + Default>(&self, type_suffix: &str) -> Option<M> {
        let event = match &self.payload {
            Some(crate::proto::event_page::Payload::Event(e)) => e,
            _ => return None,
        };
        if !event.type_url.ends_with(type_suffix) {
            return None;
        }
        M::decode(event.value.as_slice()).ok()
    }
}

/// Extension trait for CommandPage proto type.
///
/// Provides convenient accessors for sequence, type URL, and payload decoding.
pub trait CommandPageExt {
    /// Get the sequence number from this page.
    fn sequence_num(&self) -> u32;

    /// Get the type URL of the command, if present.
    fn type_url(&self) -> Option<&str>;

    /// Get the raw payload bytes, if present.
    fn payload(&self) -> Option<&[u8]>;

    /// Decode the command payload as a specific message type.
    ///
    /// Returns None if the command is missing, type URL doesn't match the suffix,
    /// or decoding fails.
    fn decode<M: prost::Message + Default>(&self, type_suffix: &str) -> Option<M>;

    /// Get the merge strategy for this command.
    ///
    /// Returns the MergeStrategy enum value. Defaults to Commutative (0) if unset.
    fn merge_strategy(&self) -> MergeStrategy;
}

impl CommandPageExt for CommandPage {
    fn sequence_num(&self) -> u32 {
        self.sequence
    }

    fn type_url(&self) -> Option<&str> {
        match &self.payload {
            Some(crate::proto::command_page::Payload::Command(c)) => Some(c.type_url.as_str()),
            _ => None,
        }
    }

    fn payload(&self) -> Option<&[u8]> {
        match &self.payload {
            Some(crate::proto::command_page::Payload::Command(c)) => Some(c.value.as_slice()),
            _ => None,
        }
    }

    fn decode<M: prost::Message + Default>(&self, type_suffix: &str) -> Option<M> {
        let command = match &self.payload {
            Some(crate::proto::command_page::Payload::Command(c)) => c,
            _ => return None,
        };
        if !command.type_url.ends_with(type_suffix) {
            return None;
        }
        M::decode(command.value.as_slice()).ok()
    }

    fn merge_strategy(&self) -> MergeStrategy {
        MergeStrategy::try_from(self.merge_strategy()).unwrap_or(MergeStrategy::MergeCommutative)
    }
}
