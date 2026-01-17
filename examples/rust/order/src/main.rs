//! Order bounded context gRPC server.
//!
//! Handles order lifecycle, loyalty discounts, and payment processing.

use std::env;
use std::net::SocketAddr;

use tonic::{transport::Server, Request, Response, Status};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use angzarr::interfaces::business_client::BusinessLogicClient;
use angzarr::proto::business_logic_server::{BusinessLogic, BusinessLogicServer};
use angzarr::proto::{BusinessResponse, ContextualCommand};
use order::OrderLogic;

const DOMAIN: &str = "order";

/// gRPC service implementation wrapping the business logic.
struct OrderService {
    logic: OrderLogic,
}

impl OrderService {
    fn new() -> Self {
        Self {
            logic: OrderLogic::new(),
        }
    }
}

#[tonic::async_trait]
impl BusinessLogic for OrderService {
    async fn handle(
        &self,
        request: Request<ContextualCommand>,
    ) -> Result<Response<BusinessResponse>, Status> {
        let cmd = request.into_inner();

        match self.logic.handle(DOMAIN, cmd).await {
            Ok(response) => Ok(Response::new(response)),
            Err(e) => Err(Status::failed_precondition(e.to_string())),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .json()
        .init();

    let port = env::var("PORT").unwrap_or_else(|_| "50056".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    let service = OrderService::new();

    // Create gRPC health service
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<BusinessLogicServer<OrderService>>()
        .await;

    info!(
        domain = DOMAIN,
        port = %port,
        "server_started"
    );

    Server::builder()
        .add_service(health_service)
        .add_service(BusinessLogicServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
