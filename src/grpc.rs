//! gRPC utilities.

use tonic::transport::Channel;

/// Connect to a gRPC endpoint.
///
/// Creates a channel connected to the given address.
/// The address should be in the format "host:port".
pub async fn connect_channel(address: &str) -> Result<Channel, String> {
    Channel::from_shared(format!("http://{}", address))
        .map_err(|e| format!("Invalid URI: {}", e))?
        .connect()
        .await
        .map_err(|e| format!("Connection failed: {}", e))
}
