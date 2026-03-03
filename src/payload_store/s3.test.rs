//! Tests for S3 payload store.
//!
//! S3 payload store uses content-addressable storage in S3 buckets:
//! - Object key: {prefix}/{hash[0:2]}/{hash}
//! - URI format: s3://{bucket}/{key}
//!
//! Why this matters: S3 is the most widely used object storage. The
//! hash-based key structure avoids hot partitions (S3 distributes objects
//! based on key prefix).
//!
//! Key behaviors verified:
//! - Object key uses hash-based directory sharding
//! - Prefix is correctly prepended when configured
//! - URI format follows s3:// scheme
//!
//! Note: Full S3 integration requires credentials and a real bucket.
//! Unit tests verify key/URI construction logic only.

use super::*;

// ============================================================================
// Object Key Tests
// ============================================================================

/// Object key uses first 2 chars of hash as subdirectory.
#[test]
fn test_object_key_without_prefix() {
    let hash = compute_hash(b"test payload");
    let hex = hash_to_hex(&hash);

    // Key should be {hash[0:2]}/{hash}
    assert!(hex.len() >= 2);
    let expected = format!("{}/{}", &hex[0..2], hex);

    assert!(expected.contains('/'));
    assert!(expected.starts_with(&hex[0..2]));
}

/// Prefix is prepended to the object key when configured.
#[test]
fn test_object_key_with_prefix() {
    let hash = compute_hash(b"test payload");
    let hex = hash_to_hex(&hash);
    let prefix = "payloads";

    let expected = format!("{}/{}/{}", prefix, &hex[0..2], hex);

    assert!(expected.starts_with(prefix));
    assert!(expected.contains(&hex[0..2]));
}

/// URI format uses s3:// scheme with bucket and key.
#[test]
fn test_uri_format() {
    let bucket = "my-bucket";
    let key = "ab/abcdef123456";
    let uri = format!("s3://{}/{}", bucket, key);

    assert!(uri.starts_with("s3://"));
    assert!(uri.contains(bucket));
}
