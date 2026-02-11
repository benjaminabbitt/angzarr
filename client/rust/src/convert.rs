//! Conversion helpers for protobuf types.

use crate::error::{ClientError, Result};
use crate::proto::Uuid as ProtoUuid;
use chrono::{DateTime, Utc};
use prost_types::Timestamp;
use uuid::Uuid;

/// Default type URL prefix for protocol buffer messages.
pub const TYPE_URL_PREFIX: &str = "type.googleapis.com";

/// Build a fully-qualified type URL from a message type name.
///
/// # Examples
/// ```
/// use angzarr_client::convert::type_url;
/// assert_eq!(type_url("examples.AddItemToCart"), "type.googleapis.com/examples.AddItemToCart");
/// ```
pub fn type_url(type_name: &str) -> String {
    format!("{}/{}", TYPE_URL_PREFIX, type_name)
}

/// Extract the type name suffix from a type URL.
///
/// Returns the part after the last `/` or the whole string if no `/` present.
pub fn type_name_from_url(type_url: &str) -> &str {
    type_url.rsplit('/').next().unwrap_or(type_url)
}

/// Check if a type URL ends with the given suffix.
pub fn type_url_matches(type_url: &str, suffix: &str) -> bool {
    type_url.ends_with(suffix)
}

/// Convert a UUID to its protobuf representation.
pub fn uuid_to_proto(uuid: Uuid) -> ProtoUuid {
    ProtoUuid {
        value: uuid.as_bytes().to_vec(),
    }
}

/// Convert a protobuf UUID to a standard UUID.
pub fn proto_to_uuid(proto: &ProtoUuid) -> Result<Uuid> {
    Uuid::from_slice(&proto.value)
        .map_err(|e| ClientError::InvalidArgument(format!("invalid UUID: {}", e)))
}

/// Parse an RFC3339 timestamp string into a protobuf Timestamp.
///
/// # Examples
/// ```
/// use angzarr_client::convert::parse_timestamp;
/// let ts = parse_timestamp("2024-01-15T10:30:00Z").unwrap();
/// assert_eq!(ts.seconds, 1705314600);
/// ```
pub fn parse_timestamp(rfc3339: &str) -> Result<Timestamp> {
    let dt: DateTime<Utc> = rfc3339
        .parse()
        .map_err(|e| ClientError::InvalidTimestamp(format!("{}: {}", rfc3339, e)))?;

    Ok(Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    })
}

/// Get the current time as a protobuf Timestamp.
pub fn now() -> Timestamp {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before unix epoch");

    Timestamp {
        seconds: now.as_secs() as i64,
        nanos: now.subsec_nanos() as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_url() {
        assert_eq!(
            type_url("examples.AddItemToCart"),
            "type.googleapis.com/examples.AddItemToCart"
        );
    }

    #[test]
    fn test_type_name_from_url() {
        assert_eq!(
            type_name_from_url("type.googleapis.com/examples.AddItemToCart"),
            "examples.AddItemToCart"
        );
        assert_eq!(type_name_from_url("AddItemToCart"), "AddItemToCart");
    }

    #[test]
    fn test_type_url_matches() {
        assert!(type_url_matches(
            "type.googleapis.com/examples.AddItemToCart",
            "AddItemToCart"
        ));
        assert!(!type_url_matches(
            "type.googleapis.com/examples.AddItemToCart",
            "RemoveItem"
        ));
    }

    #[test]
    fn test_uuid_conversion() {
        let uuid = Uuid::new_v4();
        let proto = uuid_to_proto(uuid);
        let back = proto_to_uuid(&proto).unwrap();
        assert_eq!(uuid, back);
    }

    #[test]
    fn test_parse_timestamp() {
        let ts = parse_timestamp("2024-01-15T10:30:00Z").unwrap();
        assert_eq!(ts.seconds, 1705314600);
        assert_eq!(ts.nanos, 0);
    }

    #[test]
    fn test_parse_timestamp_with_nanos() {
        let ts = parse_timestamp("2024-01-15T10:30:00.123456789Z").unwrap();
        assert_eq!(ts.seconds, 1705314600);
        assert_eq!(ts.nanos, 123456789);
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        assert!(parse_timestamp("not a timestamp").is_err());
    }
}
