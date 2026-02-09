//! Page extension traits for EventPage and CommandPage.
//!
//! Provides convenient accessors for sequence, type URL, and payload decoding.

use crate::proto::{CommandPage, EventPage};

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
        match &self.sequence {
            Some(crate::proto::event_page::Sequence::Num(n)) => *n,
            Some(crate::proto::event_page::Sequence::Force(_)) => 0,
            None => 0,
        }
    }

    fn type_url(&self) -> Option<&str> {
        self.event.as_ref().map(|e| e.type_url.as_str())
    }

    fn payload(&self) -> Option<&[u8]> {
        self.event.as_ref().map(|e| e.value.as_slice())
    }

    fn decode<M: prost::Message + Default>(&self, type_suffix: &str) -> Option<M> {
        let event = self.event.as_ref()?;
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
}

impl CommandPageExt for CommandPage {
    fn sequence_num(&self) -> u32 {
        self.sequence
    }

    fn type_url(&self) -> Option<&str> {
        self.command.as_ref().map(|c| c.type_url.as_str())
    }

    fn payload(&self) -> Option<&[u8]> {
        self.command.as_ref().map(|c| c.value.as_slice())
    }

    fn decode<M: prost::Message + Default>(&self, type_suffix: &str) -> Option<M> {
        let command = self.command.as_ref()?;
        if !command.type_url.ends_with(type_suffix) {
            return None;
        }
        M::decode(command.value.as_slice()).ok()
    }
}
