//! Transaction bounded context gRPC server.
//!
//! Handles purchases, discounts, and transaction lifecycle.

use std::env;
use std::net::SocketAddr;

use tonic::{transport::Server, Request, Response, Status};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use evented::interfaces::business_client::BusinessLogicClient;
use evented::proto::business_logic_server::{BusinessLogic, BusinessLogicServer};
use evented::proto::{ContextualCommand, EventBook};
use transaction::TransactionLogic;

const DOMAIN: &str = "transaction";

/// gRPC service implementation wrapping the business logic.
struct TransactionService {
    logic: TransactionLogic,
}

impl TransactionService {
    fn new() -> Self {
        Self {
            logic: TransactionLogic::new(),
        }
    }
}

#[tonic::async_trait]
impl BusinessLogic for TransactionService {
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

    let port = env::var("PORT").unwrap_or_else(|_| "50053".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    let service = TransactionService::new();

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
