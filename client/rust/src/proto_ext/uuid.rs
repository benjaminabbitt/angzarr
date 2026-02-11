//! UUID conversion traits.
//!
//! Provides bidirectional conversion between proto UUID and standard UUID types.

use crate::proto::Uuid as ProtoUuid;

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
