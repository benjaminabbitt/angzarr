//! Error types for the Angzarr client library.

use tonic::{Code, Status};

/// Result type for client operations.
pub type Result<T> = std::result::Result<T, ClientError>;

/// Errors that can occur during client operations.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Failed to establish connection to the server.
    #[error("connection failed: {0}")]
    Connection(String),

    /// Transport-level error from tonic.
    #[error("transport error: {0}")]
    Transport(#[from] tonic::transport::Error),

    /// gRPC error from the server.
    #[error("grpc error: {0}")]
    Grpc(Box<Status>),

    /// Invalid argument provided by caller.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// Failed to parse timestamp.
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(String),
}

impl From<Status> for ClientError {
    fn from(status: Status) -> Self {
        ClientError::Grpc(Box::new(status))
    }
}

impl ClientError {
    /// Returns the error message.
    pub fn message(&self) -> String {
        match self {
            ClientError::Connection(msg) => msg.clone(),
            ClientError::Transport(e) => e.to_string(),
            ClientError::Grpc(s) => s.message().to_string(),
            ClientError::InvalidArgument(msg) => msg.clone(),
            ClientError::InvalidTimestamp(msg) => msg.clone(),
        }
    }

    /// Returns the gRPC status code if this is a gRPC error.
    pub fn code(&self) -> Option<Code> {
        match self {
            ClientError::Grpc(s) => Some(s.code()),
            _ => None,
        }
    }

    /// Returns the underlying gRPC Status if this is a gRPC error.
    pub fn status(&self) -> Option<&Status> {
        match self {
            ClientError::Grpc(s) => Some(s),
            _ => None,
        }
    }

    /// Returns true if this is a "not found" error.
    pub fn is_not_found(&self) -> bool {
        matches!(self.code(), Some(Code::NotFound))
    }

    /// Returns true if this is a "precondition failed" error.
    pub fn is_precondition_failed(&self) -> bool {
        matches!(self.code(), Some(Code::FailedPrecondition))
    }

    /// Returns true if this is an "invalid argument" error.
    pub fn is_invalid_argument(&self) -> bool {
        matches!(self.code(), Some(Code::InvalidArgument))
            || matches!(self, ClientError::InvalidArgument(_))
    }

    /// Returns true if this is a connection or transport error.
    pub fn is_connection_error(&self) -> bool {
        matches!(self, ClientError::Connection(_) | ClientError::Transport(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_error_display() {
        let err = ClientError::Connection("refused".to_string());
        assert_eq!(err.to_string(), "connection failed: refused");
    }

    #[test]
    fn test_connection_error_message() {
        let err = ClientError::Connection("timeout".to_string());
        assert_eq!(err.message(), "timeout");
    }

    #[test]
    fn test_invalid_argument_error_display() {
        let err = ClientError::InvalidArgument("missing field".to_string());
        assert_eq!(err.to_string(), "invalid argument: missing field");
    }

    #[test]
    fn test_invalid_argument_error_message() {
        let err = ClientError::InvalidArgument("bad value".to_string());
        assert_eq!(err.message(), "bad value");
    }

    #[test]
    fn test_invalid_timestamp_error_display() {
        let err = ClientError::InvalidTimestamp("bad format".to_string());
        assert_eq!(err.to_string(), "invalid timestamp: bad format");
    }

    #[test]
    fn test_invalid_timestamp_error_message() {
        let err = ClientError::InvalidTimestamp("parse failed".to_string());
        assert_eq!(err.message(), "parse failed");
    }

    #[test]
    fn test_grpc_error_from_status() {
        let status = Status::not_found("resource not found");
        let err: ClientError = status.into();
        assert!(matches!(err, ClientError::Grpc(_)));
    }

    #[test]
    fn test_grpc_error_message() {
        let status = Status::internal("server error");
        let err = ClientError::Grpc(Box::new(status));
        assert_eq!(err.message(), "server error");
    }

    #[test]
    fn test_grpc_error_code() {
        let status = Status::not_found("missing");
        let err = ClientError::Grpc(Box::new(status));
        assert_eq!(err.code(), Some(Code::NotFound));
    }

    #[test]
    fn test_grpc_error_status() {
        let status = Status::permission_denied("access denied");
        let err = ClientError::Grpc(Box::new(status));
        let s = err.status().unwrap();
        assert_eq!(s.code(), Code::PermissionDenied);
        assert_eq!(s.message(), "access denied");
    }

    #[test]
    fn test_non_grpc_error_code_is_none() {
        let err = ClientError::Connection("refused".to_string());
        assert_eq!(err.code(), None);
    }

    #[test]
    fn test_non_grpc_error_status_is_none() {
        let err = ClientError::InvalidArgument("bad".to_string());
        assert!(err.status().is_none());
    }

    #[test]
    fn test_is_not_found_true() {
        let status = Status::not_found("missing");
        let err = ClientError::Grpc(Box::new(status));
        assert!(err.is_not_found());
    }

    #[test]
    fn test_is_not_found_false_other_code() {
        let status = Status::internal("error");
        let err = ClientError::Grpc(Box::new(status));
        assert!(!err.is_not_found());
    }

    #[test]
    fn test_is_not_found_false_non_grpc() {
        let err = ClientError::Connection("refused".to_string());
        assert!(!err.is_not_found());
    }

    #[test]
    fn test_is_precondition_failed_true() {
        let status = Status::failed_precondition("conflict");
        let err = ClientError::Grpc(Box::new(status));
        assert!(err.is_precondition_failed());
    }

    #[test]
    fn test_is_precondition_failed_false() {
        let status = Status::not_found("missing");
        let err = ClientError::Grpc(Box::new(status));
        assert!(!err.is_precondition_failed());
    }

    #[test]
    fn test_is_invalid_argument_grpc_true() {
        let status = Status::invalid_argument("bad input");
        let err = ClientError::Grpc(Box::new(status));
        assert!(err.is_invalid_argument());
    }

    #[test]
    fn test_is_invalid_argument_client_error_true() {
        let err = ClientError::InvalidArgument("missing".to_string());
        assert!(err.is_invalid_argument());
    }

    #[test]
    fn test_is_invalid_argument_false() {
        let err = ClientError::Connection("refused".to_string());
        assert!(!err.is_invalid_argument());
    }

    #[test]
    fn test_is_connection_error_connection_true() {
        let err = ClientError::Connection("refused".to_string());
        assert!(err.is_connection_error());
    }

    #[test]
    fn test_is_connection_error_grpc_false() {
        let status = Status::internal("error");
        let err = ClientError::Grpc(Box::new(status));
        assert!(!err.is_connection_error());
    }

    #[test]
    fn test_is_connection_error_invalid_argument_false() {
        let err = ClientError::InvalidArgument("bad".to_string());
        assert!(!err.is_connection_error());
    }
}
