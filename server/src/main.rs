//! svc-cargo
//! Processes flight requests from client applications

mod grpc_clients;
mod rest_api;
mod cargo_grpc {
    #![allow(unused_qualifications)]
    include!("grpc.rs");
}

#[macro_use]
mod loggers;

use axum::{extract::Extension, handler::Handler, response::IntoResponse, routing, Router};
use cargo_grpc::cargo_rpc_server::{CargoRpc, CargoRpcServer};
use grpc_clients::GrpcClients;
use log::{info, warn};
use utoipa::OpenApi;

///////////////////////////////////////////////////////////////////////
/// GRPC SERVER
///////////////////////////////////////////////////////////////////////

/// Struct that implements the CargoRpc trait.
///
/// This is the main struct that implements the gRPC service.
#[derive(Default, Debug, Clone, Copy)]
pub struct CargoGrpcImpl {}

// Implementing gRPC interfaces for this microservice
#[tonic::async_trait]
impl CargoRpc for CargoGrpcImpl {
    /// Replies true if this server is ready to serve others.
    /// # Arguments
    /// * `request` - the query object with no arguments
    /// # Returns
    /// * `ReadyResponse` - Returns true
    async fn is_ready(
        &self,
        _request: tonic::Request<cargo_grpc::QueryIsReady>,
    ) -> Result<tonic::Response<cargo_grpc::ReadyResponse>, tonic::Status> {
        info!("is_ready() enter");
        let response = cargo_grpc::ReadyResponse { ready: true };

        info!("is_ready() exit");
        Ok(tonic::Response::new(response))
    }
}

/// Starts the grpc server for this microservice
async fn grpc_server() {
    // GRPC Server
    let grpc_port = std::env::var("DOCKER_PORT_GRPC")
        .unwrap_or_else(|_| "50051".to_string())
        .parse::<u16>()
        .unwrap_or(50051);

    let addr = format!("[::]:{grpc_port}").parse().unwrap();
    let imp = CargoGrpcImpl::default();
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<CargoRpcServer<CargoGrpcImpl>>()
        .await;

    info!("(grpc) hosted at {}", addr);
    tonic::transport::Server::builder()
        .add_service(health_service)
        .add_service(CargoRpcServer::new(imp))
        .serve_with_shutdown(addr, shutdown_signal("grpc"))
        .await
        .unwrap();
}

///////////////////////////////////////////////////////////////////////
/// REST SERVER
///////////////////////////////////////////////////////////////////////

/// Responds a NOT_FOUND status and error string
///
/// # Arguments
///
/// # Examples
///
/// ```
/// let app = Router::new()
///         .fallback(not_found.into_service());
/// ```
pub async fn not_found(uri: axum::http::Uri) -> impl IntoResponse {
    (
        axum::http::StatusCode::NOT_FOUND,
        format!("No route {}", uri),
    )
}

/// Tokio signal handler that will wait for a user to press CTRL+C.
/// We use this in our hyper `Server` method `with_graceful_shutdown`.
///
/// # Arguments
///
/// # Examples
///
/// ```
/// Server::bind(&"0.0.0.0:8000".parse().unwrap())
/// .serve(app.into_make_service())
/// .with_graceful_shutdown(shutdown_signal())
/// .await
/// .unwrap();
/// ```
pub async fn shutdown_signal(server: &str) {
    tokio::signal::ctrl_c()
        .await
        .expect("expect tokio signal ctrl-c");
    warn!("({}) shutdown signal", server);
}

/// Starts the REST API server for this microservice
pub async fn rest_server(grpc_clients: GrpcClients) {
    let rest_port = std::env::var("DOCKER_PORT_REST")
        .unwrap_or_else(|_| "8000".to_string())
        .parse::<u16>()
        .unwrap_or(8000);

    #[derive(OpenApi)]
    #[openapi(
        paths(
            rest_api::query_flight,
            rest_api::query_vertiports,
            rest_api::confirm_flight,
            rest_api::cancel_flight
        ),
        components(
            schemas(
                rest_api::rest_types::FlightOption,
                rest_api::rest_types::Vertiport,
                rest_api::rest_types::ConfirmStatus,
                rest_api::rest_types::VertiportsQuery,
                rest_api::rest_types::FlightQuery
            )
        ),
        tags(
            (name = "svc-cargo", description = "svc-cargo API")
        )
    )]
    struct ApiDoc;

    let app = Router::new()
        // .merge(SwaggerUi::new("/swagger-ui/*tail").url("/api-doc/openapi.json", ApiDoc::openapi()))
        .fallback(not_found.into_service())
        .route("/cargo/cancel", routing::delete(rest_api::cancel_flight))
        .route("/cargo/query", routing::post(rest_api::query_flight))
        .route("/cargo/confirm", routing::put(rest_api::confirm_flight))
        .route(
            "/cargo/vertiports",
            routing::post(rest_api::query_vertiports),
        )
        .layer(Extension(grpc_clients)); // Extension layer must be last

    let address = format!("[::]:{rest_port}").parse().unwrap();
    info!("(rest) hosted at {:?}", address);
    axum::Server::bind(&address)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal("rest"))
        .await
        .unwrap();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    {
        let log_cfg: &str = "log4rs.yaml";
        if let Err(e) = log4rs::init_file(log_cfg, Default::default()) {
            println!("(logger) could not parse {}. {}", log_cfg, e);
            panic!();
        }
    }

    // Start GRPC Server
    tokio::spawn(grpc_server());

    // Wait for other GRPC Servers
    let grpc_clients = GrpcClients::default();

    // Start REST API
    rest_server(grpc_clients).await;

    info!("Successful shutdown.");
    Ok(())
}
