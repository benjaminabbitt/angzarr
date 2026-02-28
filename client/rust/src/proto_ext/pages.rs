//! Page extension traits for EventPage and CommandPage.
//!
//! Provides convenient accessors for sequence, type URL, and payload decoding.

use crate::convert::TYPE_URL_PREFIX;
use crate::proto::{CommandPage, EventPage, MergeStrategy};
use prost::Name;

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

    /// Type-safe decode using prost::Name reflection.
    ///
    /// Returns None if the event is missing, type URL doesn't match exactly,
    /// or decoding fails. The expected type URL is derived from M::full_name().
    fn decode_typed<M: prost::Message + Default + Name>(&self) -> Option<M>;
}

impl EventPageExt for EventPage {
    fn sequence_num(&self) -> u32 {
        match &self.sequence_type {
            Some(crate::proto::event_page::SequenceType::Sequence(seq)) => *seq,
            Some(crate::proto::event_page::SequenceType::Fact(_)) => 0, // Facts get sequence assigned by coordinator
            None => 0,
        }
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

    fn decode_typed<M: prost::Message + Default + Name>(&self) -> Option<M> {
        let event = match &self.payload {
            Some(crate::proto::event_page::Payload::Event(e)) => e,
            _ => return None,
        };
        let expected = format!("{}{}", TYPE_URL_PREFIX, M::full_name());
        if event.type_url != expected {
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

    /// Type-safe decode using prost::Name reflection.
    ///
    /// Returns None if the command is missing, type URL doesn't match exactly,
    /// or decoding fails. The expected type URL is derived from M::full_name().
    fn decode_typed<M: prost::Message + Default + Name>(&self) -> Option<M>;

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

    fn decode_typed<M: prost::Message + Default + Name>(&self) -> Option<M> {
        let command = match &self.payload {
            Some(crate::proto::command_page::Payload::Command(c)) => c,
            _ => return None,
        };
        let expected = format!("{}{}", TYPE_URL_PREFIX, M::full_name());
        if command.type_url != expected {
            return None;
        }
        M::decode(command.value.as_slice()).ok()
    }

    fn merge_strategy(&self) -> MergeStrategy {
        MergeStrategy::try_from(self.merge_strategy).unwrap_or(MergeStrategy::MergeCommutative)
    }
}
