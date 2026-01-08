//! Customer bounded context gRPC server.
//!
//! Handles customer lifecycle and loyalty points management.

use std::env;
use std::net::SocketAddr;

use tonic::{transport::Server, Request, Response, Status};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use customer::CustomerLogic;
use evented::interfaces::business_client::BusinessLogicClient;
use evented::proto::business_logic_server::{BusinessLogic, BusinessLogicServer};
use evented::proto::{ContextualCommand, EventBook};

const DOMAIN: &str = "customer";

/// gRPC service implementation wrapping the business logic.
struct CustomerService {
    logic: CustomerLogic,
}

impl CustomerService {
    fn new() -> Self {
        Self {
            logic: CustomerLogic::new(),
        }
    }
}

#[tonic::async_trait]
impl BusinessLogic for CustomerService {
    async fn handle(
        &self,
        request: Request<ContextualCommand>,
    ) -> Result<Response<EventBook>, Status> {
        let cmd = request.into_inner();

        match self.logic.handle(DOMAIN, cmd).await {
            Ok(event_book) => Ok(Response::new(event_book)),
            Err(e) => Err(Status::failed_precondition(e.to_string())),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .json().init();

    let port = env::var("PORT").unwrap_or_else(|_| "50052".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    let service = CustomerService::new();

    info!(
        domain = DOMAIN,
        port = %port,
        "server_started"
    );

    Server::builder()
        .add_service(BusinessLogicServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
