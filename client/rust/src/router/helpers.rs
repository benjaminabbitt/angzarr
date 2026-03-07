//! Helper functions for building events and commands.

use prost::Message;
use prost_types::Any;

use crate::proto::{
    event_page, page_header::SequenceType, CommandBook, EventBook, EventPage, PageHeader,
};

/// Helper to create an event page with proper sequence.
pub fn event_page(seq: u32, event: Any) -> EventPage {
    EventPage {
        header: Some(PageHeader {
            sequence_type: Some(SequenceType::Sequence(seq)),
        }),
        created_at: Some(crate::now()),
        payload: Some(event_page::Payload::Event(event)),
    }
}

/// Helper to create an EventBook from command book cover and events.
pub fn event_book_from(command_book: &CommandBook, pages: Vec<EventPage>) -> EventBook {
    EventBook {
        cover: command_book.cover.clone(),
        pages,
        snapshot: None,
        next_sequence: 0,
    }
}

/// Helper to create an EventBook with a single event.
///
/// This is the common pattern for command handlers returning a single event.
pub fn new_event_book(command_book: &CommandBook, seq: u32, event: Any) -> EventBook {
    event_book_from(command_book, vec![event_page(seq, event)])
}

/// Helper to create an EventBook with multiple events.
///
/// Useful for handlers that emit multiple events (e.g., AwardPot + HandComplete).
pub fn new_event_book_multi(
    command_book: &CommandBook,
    start_seq: u32,
    events: Vec<Any>,
) -> EventBook {
    let pages = events
        .into_iter()
        .enumerate()
        .map(|(i, event)| event_page(start_seq + i as u32, event))
        .collect();
    event_book_from(command_book, pages)
}

/// Pack a protobuf message into an Any with the given type URL.
pub fn pack_event<M: Message>(msg: &M, type_name: &str) -> Any {
    Any {
        type_url: crate::type_url(type_name),
        value: msg.encode_to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto_ext::EventPageExt;

    #[test]
    fn event_page_sets_sequence() {
        let event = Any {
            type_url: "test".into(),
            value: vec![],
        };
        let page = event_page(42, event);
        assert_eq!(page.sequence_num(), 42);
        assert!(page.created_at.is_some());
    }

    #[test]
    fn new_event_book_creates_single_page() {
        let cmd = CommandBook::default();
        let event = Any {
            type_url: "test".into(),
            value: vec![],
        };
        let book = new_event_book(&cmd, 1, event);
        assert_eq!(book.pages.len(), 1);
        assert_eq!(book.pages[0].sequence_num(), 1);
    }

    #[test]
    fn new_event_book_multi_creates_sequential_pages() {
        let cmd = CommandBook::default();
        let events = vec![
            Any {
                type_url: "test1".into(),
                value: vec![],
            },
            Any {
                type_url: "test2".into(),
                value: vec![],
            },
        ];
        let book = new_event_book_multi(&cmd, 5, events);
        assert_eq!(book.pages.len(), 2);
        assert_eq!(book.pages[0].sequence_num(), 5);
        assert_eq!(book.pages[1].sequence_num(), 6);
    }
}
