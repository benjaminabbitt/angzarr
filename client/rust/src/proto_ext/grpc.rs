//! gRPC utilities for correlation ID propagation.

use super::constants::CORRELATION_ID_HEADER;

/// Create a tonic Request with `x-correlation-id` gRPC metadata.
///
/// Propagates the correlation_id into gRPC request headers so that
/// server-side tower middleware can create tracing spans before
/// protobuf deserialization.
pub fn correlated_request<T>(msg: T, correlation_id: &str) -> tonic::Request<T> {
    let mut req = tonic::Request::new(msg);
    if !correlation_id.is_empty() {
        if let Ok(val) = correlation_id.parse() {
            req.metadata_mut().insert(CORRELATION_ID_HEADER, val);
        }
    }
    req
}
