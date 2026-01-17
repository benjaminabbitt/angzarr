//! Receipt projector gRPC server.
//!
//! Generates Receipt projections from completed order events.

use std::env;
use std::net::SocketAddr;

use tonic::{transport::Server, Request, Response, Status};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use angzarr::proto::projector_coordinator_server::{
    ProjectorCoordinator, ProjectorCoordinatorServer,
};
use angzarr::proto::{EventBook, Projection};
use projector_receipt::{logmsg, ReceiptProjectorLogic};

const PROJECTOR_NAME: &str = "receipt";

/// gRPC service implementation wrapping the projector logic.
struct ReceiptProjectorService {
    logic: ReceiptProjectorLogic,
}

impl ReceiptProjectorService {
    fn new() -> Self {
        Self {
            logic: ReceiptProjectorLogic::new(),
        }
    }
}

#[tonic::async_trait]
impl ProjectorCoordinator for ReceiptProjectorService {
    async fn handle_sync(
        &self,
        request: Request<EventBook>,
    ) -> Result<Response<Projection>, Status> {
        let event_book = request.into_inner();

        let projection = self.logic.project(&event_book).unwrap_or_else(|| {
            // Return empty projection if order not completed
            Projection {
                cover: event_book.cover,
                projector: PROJECTOR_NAME.to_string(),
                sequence: 0,
                projection: None,
            }
        });

        if projection.projection.is_some() {
            let order_id = projection
                .cover
                .as_ref()
                .and_then(|c| c.root.as_ref())
                .map(|r| hex::encode(&r.value))
                .unwrap_or_default();

            let short_id = if order_id.len() > 16 {
                &order_id[..16]
            } else {
                &order_id
            };

            info!(
                message = logmsg::GENERATED_RECEIPT,
                order_id = %short_id,
                sequence = projection.sequence
            );
        }

        Ok(Response::new(projection))
    }

    async fn handle(&self, request: Request<EventBook>) -> Result<Response<()>, Status> {
        let event_book = request.into_inner();
        // Fire and forget - just project if possible
        let _ = self.logic.project(&event_book);
        Ok(Response::new(()))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .json()
        .init();

    let port = env::var("PORT").unwrap_or_else(|_| "50410".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    let service = ReceiptProjectorService::new();

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<ProjectorCoordinatorServer<ReceiptProjectorService>>()
        .await;

    info!(
        projector = PROJECTOR_NAME,
        port = %port,
        listens_to = "order domain",
        "server_started"
    );

    Server::builder()
        .add_service(health_service)
        .add_service(ProjectorCoordinatorServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
