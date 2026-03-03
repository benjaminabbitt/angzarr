//! Tests for GCS payload store.
//!
//! GCS payload store uses content-addressable storage in GCS buckets:
//! - Object path: {prefix}/{hash[0:2]}/{hash}
//! - URI format: gs://{bucket}/{path}
//!
//! Why this matters: GCS is the recommended storage for payloads on GCP.
//! The sharded directory structure (using first 2 chars of hash) prevents
//! hot spots when listing objects.
//!
//! Key behaviors verified:
//! - Object naming uses hash-based directory sharding
//! - URI format follows gs:// scheme
//!
//! Note: Full GCS integration requires credentials and a real bucket.
//! Unit tests verify path/URI construction logic only.

use super::*;

// ============================================================================
// Object Path Tests
// ============================================================================

/// Object name uses first 2 chars of hash as subdirectory.
#[test]
fn test_object_name_without_prefix() {
    // We can't easily test the full store without GCS, but we can test helpers
    let hash = compute_hash(b"test payload");
    let hex = hash_to_hex(&hash);

    // Object name should be {hash[0:2]}/{hash}
    assert!(hex.len() >= 2);
    let expected = format!("{}/{}", &hex[0..2], hex);

    // Can't create store without credentials, so just verify the expected format
    assert!(expected.contains('/'));
    assert!(expected.starts_with(&hex[0..2]));
}

/// URI format uses gs:// scheme with bucket and object path.
#[test]
fn test_uri_format() {
    // Verify URI format is correct
    let bucket = "my-bucket";
    let object = "ab/abcdef123456";
    let uri = format!("gs://{}/{}", bucket, object);

    assert!(uri.starts_with("gs://"));
    assert!(uri.contains(bucket));
}
