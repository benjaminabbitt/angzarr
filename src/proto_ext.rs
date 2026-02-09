//! Extension traits for proto types.
//!
//! Provides convenient accessor methods for common patterns like extracting
//! domain, correlation_id, and root_id from Cover-bearing types.

use crate::proto::{
    CommandBook, CommandPage, Cover, Edition, EventBook, EventPage, Query, Snapshot,
    Uuid as ProtoUuid,
};

/// gRPC metadata key for correlation ID propagation.
pub const CORRELATION_ID_HEADER: &str = "x-correlation-id";

/// Fallback domain when cover is missing or has no domain set.
pub const UNKNOWN_DOMAIN: &str = "unknown";

/// Domain prefix for synthetic projection event books.
///
/// Projector output is published as `_projection.{projector_name}.{domain}`.
pub const PROJECTION_DOMAIN_PREFIX: &str = "_projection";

/// Protobuf type URL for serialized Projection messages in synthetic event books.
pub const PROJECTION_TYPE_URL: &str = "angzarr.Projection";

/// Wildcard domain for catch-all routing (matches any domain).
pub const WILDCARD_DOMAIN: &str = "*";

/// The meta domain for angzarr infrastructure.
pub const META_ANGZARR_DOMAIN: &str = "_angzarr";

/// Type URL for RegisterComponent command.
pub const REGISTER_COMPONENT_TYPE_URL: &str = "type.angzarr/angzarr.RegisterComponent";

/// Type URL for ComponentRegistered event.
pub const COMPONENT_REGISTERED_TYPE_URL: &str = "type.angzarr/angzarr.ComponentRegistered";

/// Derive a deterministic UUID from a component name.
///
/// Uses the angzarr namespace to ensure consistent root UUIDs across registrations.
pub fn component_name_to_uuid(name: &str) -> uuid::Uuid {
    use crate::orchestration::correlation::ANGZARR_UUID_NAMESPACE;
    uuid::Uuid::new_v5(&ANGZARR_UUID_NAMESPACE, name.as_bytes())
}

/// Build registration commands for component descriptors.
///
/// Returns a list of CommandBooks, one per descriptor, targeting the _angzarr
/// meta aggregate with root UUID derived from component name.
pub fn build_registration_commands(
    descriptors: &[crate::proto::ComponentDescriptor],
    pod_id: &str,
) -> Vec<crate::proto::CommandBook> {
    use prost::Message;

    descriptors
        .iter()
        .map(|descriptor| {
            let root_uuid = component_name_to_uuid(&descriptor.name);
            let cmd = crate::proto::RegisterComponent {
                descriptor: Some(descriptor.clone()),
                pod_id: pod_id.to_string(),
            };

            let mut buf = Vec::new();
            cmd.encode(&mut buf).expect("encode RegisterComponent");

            crate::proto::CommandBook {
                cover: Some(Cover {
                    domain: META_ANGZARR_DOMAIN.to_string(),
                    root: Some(crate::proto::Uuid {
                        value: root_uuid.as_bytes().to_vec(),
                    }),
                    correlation_id: format!("registration-{}", descriptor.name),
                    edition: None,
                }),
                pages: vec![crate::proto::CommandPage {
                    sequence: 0,
                    command: Some(prost_types::Any {
                        type_url: REGISTER_COMPONENT_TYPE_URL.to_string(),
                        value: buf,
                    }),
                }],
                saga_origin: None,
            }
        })
        .collect()
}

/// Get the current pod ID for component registration.
///
/// In K8s: uses POD_NAME environment variable.
/// Locally: uses hostname or "standalone".
pub fn get_pod_id() -> String {
    std::env::var("POD_NAME").unwrap_or_else(|_| "standalone".to_string())
}

/// Extension trait for types with an optional Cover.
///
/// Provides convenient accessors for domain, correlation_id, and root_id
/// without verbose `.cover.as_ref().map(...)` chains.
pub trait CoverExt {
    /// Get the cover, if present.
    fn cover(&self) -> Option<&Cover>;

    /// Get the domain from the cover, or [`UNKNOWN_DOMAIN`] if missing.
    fn domain(&self) -> &str {
        self.cover()
            .map(|c| c.domain.as_str())
            .unwrap_or(UNKNOWN_DOMAIN)
    }

    /// Get the correlation_id from the cover, or empty string if missing.
    fn correlation_id(&self) -> &str {
        self.cover()
            .map(|c| c.correlation_id.as_str())
            .unwrap_or("")
    }

    /// Get the root UUID as a hex-encoded string, if present.
    fn root_id_hex(&self) -> Option<String> {
        self.cover()
            .and_then(|c| c.root.as_ref())
            .map(|u| hex::encode(&u.value))
    }

    /// Get the root UUID, if present.
    fn root_uuid(&self) -> Option<uuid::Uuid> {
        self.cover()
            .and_then(|c| c.root.as_ref())
            .and_then(|u| uuid::Uuid::from_slice(&u.value).ok())
    }

    /// Check if correlation_id is present and non-empty.
    fn has_correlation_id(&self) -> bool {
        !self.correlation_id().is_empty()
    }

    /// Get the edition name from the cover.
    ///
    /// Returns the explicit edition name if set and non-empty, otherwise
    /// defaults to the canonical timeline name (`"angzarr"`).
    fn edition(&self) -> &str {
        self.cover()
            .and_then(|c| c.edition.as_ref())
            .map(|e| e.name.as_str())
            .filter(|e| !e.is_empty())
            .unwrap_or(crate::orchestration::aggregate::DEFAULT_EDITION)
    }

    /// Get the Edition struct from the cover, if present.
    fn edition_struct(&self) -> Option<&Edition> {
        self.cover().and_then(|c| c.edition.as_ref())
    }

    /// Get the edition name as an Option, without defaulting.
    ///
    /// Returns `Some(&str)` if edition is set and non-empty, `None` otherwise.
    fn edition_opt(&self) -> Option<&str> {
        self.cover()
            .and_then(|c| c.edition.as_ref())
            .map(|e| e.name.as_str())
            .filter(|n| !n.is_empty())
    }

    /// Compute the bus routing key: `"{domain}"`.
    ///
    /// The routing key is a transport concern used for bus subscription matching.
    /// Edition filtering is handled at the handler level, not the bus level.
    fn routing_key(&self) -> String {
        self.domain().to_string()
    }

    /// Generate a cache key for this entity based on domain + root.
    ///
    /// Used for caching aggregate state during saga retry to avoid redundant fetches.
    fn cache_key(&self) -> String {
        let domain = self.domain();
        let root = self.root_id_hex().unwrap_or_default();
        format!("{domain}:{root}")
    }
}

impl CoverExt for EventBook {
    fn cover(&self) -> Option<&Cover> {
        self.cover.as_ref()
    }
}

impl CoverExt for CommandBook {
    fn cover(&self) -> Option<&Cover> {
        self.cover.as_ref()
    }
}

impl CoverExt for Query {
    fn cover(&self) -> Option<&Cover> {
        self.cover.as_ref()
    }
}

impl CoverExt for Cover {
    fn cover(&self) -> Option<&Cover> {
        Some(self)
    }
}

// ============================================================================
// Edition Extension Trait
// ============================================================================

/// Extension trait for Edition proto type.
///
/// Provides convenience methods for checking timeline status and accessing
/// divergence information. Constructors remain as associated functions on Edition.
pub trait EditionExt {
    /// Get reference to the edition.
    fn edition_inner(&self) -> &Edition;

    /// Check if this edition has an empty name.
    fn is_empty(&self) -> bool {
        self.edition_inner().name.is_empty()
    }

    /// Check if this is the main timeline (empty or default edition name).
    fn is_main_timeline(&self) -> bool {
        let name = &self.edition_inner().name;
        name.is_empty() || name == crate::orchestration::aggregate::DEFAULT_EDITION
    }

    /// Get the edition name, returning the default edition name if empty.
    fn name_or_default(&self) -> &str {
        let edition = self.edition_inner();
        if edition.name.is_empty() {
            crate::orchestration::aggregate::DEFAULT_EDITION
        } else {
            &edition.name
        }
    }

    /// Get explicit divergence for a specific domain, if any.
    fn divergence_for(&self, domain: &str) -> Option<u32> {
        self.edition_inner()
            .divergences
            .iter()
            .find(|d| d.domain == domain)
            .map(|d| d.sequence)
    }
}

impl EditionExt for Edition {
    fn edition_inner(&self) -> &Edition {
        self
    }
}

/// Constructors for Edition (cannot be in trait).
impl Edition {
    /// Create an Edition for the main timeline (empty name).
    pub fn main_timeline() -> Self {
        Self {
            name: String::new(),
            divergences: vec![],
        }
    }

    /// Create an Edition with implicit divergence (name only, no explicit divergences).
    pub fn implicit(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            divergences: vec![],
        }
    }

    /// Create an Edition with explicit divergence points.
    pub fn explicit(
        name: impl Into<String>,
        divergences: Vec<crate::proto::DomainDivergence>,
    ) -> Self {
        Self {
            name: name.into(),
            divergences,
        }
    }
}

impl From<&str> for Edition {
    fn from(name: &str) -> Self {
        Edition::implicit(name)
    }
}

impl From<String> for Edition {
    fn from(name: String) -> Self {
        Edition::implicit(name)
    }
}

// ============================================================================
// ProtoUuid Extension Trait
// ============================================================================

/// Extension trait for ProtoUuid proto type.
///
/// Provides conversion methods to standard Uuid types.
pub trait ProtoUuidExt {
    /// Convert to a standard UUID.
    fn to_uuid(&self) -> Result<uuid::Uuid, uuid::Error>;

    /// Convert to a hex-encoded string.
    fn to_hex(&self) -> String;
}

impl ProtoUuidExt for ProtoUuid {
    fn to_uuid(&self) -> Result<uuid::Uuid, uuid::Error> {
        uuid::Uuid::from_slice(&self.value)
    }

    fn to_hex(&self) -> String {
        hex::encode(&self.value)
    }
}

// ============================================================================
// Uuid Extension Trait (reverse direction)
// ============================================================================

/// Extension trait for uuid::Uuid to convert to proto types.
pub trait UuidExt {
    /// Convert to a ProtoUuid.
    fn to_proto_uuid(&self) -> ProtoUuid;
}

impl UuidExt for uuid::Uuid {
    fn to_proto_uuid(&self) -> ProtoUuid {
        ProtoUuid {
            value: self.as_bytes().to_vec(),
        }
    }
}

// ============================================================================
// EventPage Extension Trait
// ============================================================================

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

// ============================================================================
// CommandPage Extension Trait
// ============================================================================

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

// ============================================================================
// EventBook Extension Trait
// ============================================================================

/// Extension trait for EventBook proto type (beyond CoverExt).
///
/// Provides convenience methods for working with event pages.
pub trait EventBookExt: CoverExt {
    /// Compute the next sequence number based on existing pages.
    ///
    /// Returns 0 if no pages exist, otherwise max(page.sequence) + 1.
    fn next_sequence(&self) -> u32;

    /// Check if the event book has no pages.
    fn is_empty(&self) -> bool;

    /// Get the last event page, if any.
    fn last_page(&self) -> Option<&EventPage>;

    /// Get the first event page, if any.
    fn first_page(&self) -> Option<&EventPage>;
}

/// Compute next sequence number from pages and optional snapshot.
///
/// Returns (last page sequence + 1) OR (snapshot sequence + 1) if no pages, OR 0 if neither.
pub fn calculate_next_sequence(pages: &[EventPage], snapshot: Option<&Snapshot>) -> u32 {
    if let Some(last_page) = pages.last() {
        last_page.sequence_num() + 1
    } else {
        snapshot.map(|s| s.sequence + 1).unwrap_or(0)
    }
}

/// Calculate and set the next_sequence field on an EventBook.
pub fn calculate_set_next_seq(book: &mut EventBook) {
    book.next_sequence = calculate_next_sequence(&book.pages, book.snapshot.as_ref());
}

impl EventBookExt for EventBook {
    fn next_sequence(&self) -> u32 {
        calculate_next_sequence(&self.pages, self.snapshot.as_ref())
    }

    fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    fn last_page(&self) -> Option<&EventPage> {
        self.pages.last()
    }

    fn first_page(&self) -> Option<&EventPage> {
        self.pages.first()
    }
}

// ============================================================================
// CommandBook Extension Trait
// ============================================================================

/// Extension trait for CommandBook proto type (beyond CoverExt).
///
/// Provides convenience methods for working with command pages.
pub trait CommandBookExt: CoverExt {
    /// Get the sequence number from the first command page.
    fn command_sequence(&self) -> u32;

    /// Get the first command page, if any.
    fn first_command(&self) -> Option<&CommandPage>;
}

impl CommandBookExt for CommandBook {
    fn command_sequence(&self) -> u32 {
        self.pages.first().map(|p| p.sequence_num()).unwrap_or(0)
    }

    fn first_command(&self) -> Option<&CommandPage> {
        self.pages.first()
    }
}

/// Create a tonic Request with `x-correlation-id` gRPC metadata.
///
/// Propagates the correlation_id into gRPC request headers so that
/// server-side tower middleware can create tracing spans before
/// protobuf deserialization.
///
/// When the `otel` feature is enabled, also injects W3C `traceparent`
/// header for distributed trace context propagation.
pub fn correlated_request<T>(msg: T, correlation_id: &str) -> tonic::Request<T> {
    let mut req = tonic::Request::new(msg);
    if !correlation_id.is_empty() {
        if let Ok(val) = correlation_id.parse() {
            req.metadata_mut().insert(CORRELATION_ID_HEADER, val);
        }
    }

    #[cfg(feature = "otel")]
    {
        inject_trace_context(req.metadata_mut());
    }

    req
}

/// Inject W3C trace context into tonic metadata from the current tracing span.
#[cfg(feature = "otel")]
fn inject_trace_context(metadata: &mut tonic::metadata::MetadataMap) {
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    let cx = tracing::Span::current().context();

    opentelemetry::global::get_text_map_propagator(|propagator| {
        let mut injector = MetadataInjector(metadata);
        propagator.inject_context(&cx, &mut injector);
    });
}

/// Adapter to inject OTel context into tonic gRPC metadata.
#[cfg(feature = "otel")]
struct MetadataInjector<'a>(&'a mut tonic::metadata::MetadataMap);

#[cfg(feature = "otel")]
impl opentelemetry::propagation::Injector for MetadataInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(key) = tonic::metadata::MetadataKey::from_bytes(key.as_bytes()) {
            if let Ok(val) = value.parse() {
                self.0.insert(key, val);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cover(domain: &str, correlation_id: &str, root: Option<uuid::Uuid>) -> Cover {
        Cover {
            domain: domain.to_string(),
            correlation_id: correlation_id.to_string(),
            root: root.map(|u| ProtoUuid {
                value: u.as_bytes().to_vec(),
            }),
            edition: None,
        }
    }

    #[test]
    fn test_event_book_with_cover() {
        let root = uuid::Uuid::new_v4();
        let book = EventBook {
            cover: Some(make_cover("orders", "corr-123", Some(root))),
            pages: vec![],
            snapshot: None,
            ..Default::default()
        };

        assert_eq!(book.domain(), "orders");
        assert_eq!(book.correlation_id(), "corr-123");
        assert!(book.has_correlation_id());
        assert_eq!(book.root_uuid(), Some(root));
        assert_eq!(book.root_id_hex(), Some(hex::encode(root.as_bytes())));
    }

    #[test]
    fn test_event_book_without_cover() {
        let book = EventBook {
            cover: None,
            pages: vec![],
            snapshot: None,
            ..Default::default()
        };

        assert_eq!(book.domain(), "unknown");
        assert_eq!(book.correlation_id(), "");
        assert!(!book.has_correlation_id());
        assert_eq!(book.root_uuid(), None);
        assert_eq!(book.root_id_hex(), None);
    }

    #[test]
    fn test_command_book_with_cover() {
        let book = CommandBook {
            cover: Some(make_cover("inventory", "corr-456", None)),
            pages: vec![],
            saga_origin: None,
        };

        assert_eq!(book.domain(), "inventory");
        assert_eq!(book.correlation_id(), "corr-456");
        assert!(book.has_correlation_id());
        assert_eq!(book.root_uuid(), None);
    }

    #[test]
    fn test_edition_main_timeline() {
        let edition = Edition::main_timeline();
        assert!(edition.is_main_timeline());
        assert_eq!(edition.name_or_default(), "angzarr");
    }

    #[test]
    fn test_edition_implicit() {
        let edition = Edition::implicit("v2");
        assert!(!edition.is_main_timeline());
        assert_eq!(edition.name, "v2");
        assert!(edition.divergences.is_empty());
    }

    #[test]
    fn test_edition_explicit_divergence() {
        let edition = Edition::explicit(
            "v2",
            vec![
                crate::proto::DomainDivergence {
                    domain: "order".to_string(),
                    sequence: 50,
                },
                crate::proto::DomainDivergence {
                    domain: "inventory".to_string(),
                    sequence: 75,
                },
            ],
        );
        assert_eq!(edition.divergence_for("order"), Some(50));
        assert_eq!(edition.divergence_for("inventory"), Some(75));
        assert_eq!(edition.divergence_for("other"), None);
    }

    #[test]
    fn test_edition_from_string() {
        let edition: Edition = "v2".into();
        assert_eq!(edition.name, "v2");
        assert!(edition.divergences.is_empty());
    }
}
