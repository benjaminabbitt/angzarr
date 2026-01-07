pub mod google {
    pub mod protobuf {
        pub use prost_types::Timestamp;
    }
}

#[allow(clippy::all)]
mod examples;
pub use examples::*;
