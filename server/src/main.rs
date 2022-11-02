//! svc-cargo
//! Processes flight requests from client applications
/// Types Used in REST Messages
mod rest_types {
    include!("../../openapi/types.rs");
}

/// Autogenerated GRPC Server Stubs
pub mod cargo_grpc {
    #![allow(unused_qualifications)]
    include!("grpc.rs");
}

/// Client connections to other GRPC Servers
mod grpc_clients;

use axum::{extract::Extension, handler::Handler, response::IntoResponse, routing, Json, Router};
use cargo_grpc::cargo_rpc_server::{CargoRpc, CargoRpcServer};
use grpc_clients::GrpcClients;
use hyper::{HeaderMap, StatusCode};
use std::time::SystemTime;
use utoipa::OpenApi;

///////////////////////////////////////////////////////////////////////
/// Constants
///////////////////////////////////////////////////////////////////////
const MAX_CARGO_WEIGHT_G: u32 = 1_000_000; // 1000 kg

///////////////////////////////////////////////////////////////////////
/// Helpers
///////////////////////////////////////////////////////////////////////
fn is_uuid(s: &str) -> bool {
    uuid::Uuid::parse_str(s).is_ok()
}

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
        let response = cargo_grpc::ReadyResponse { ready: true };
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

    println!("gRPC Server Listening at {}", addr);
    tonic::transport::Server::builder()
        .add_service(health_service)
        .add_service(CargoRpcServer::new(imp))
        .serve_with_shutdown(addr, shutdown_signal())
        .await
        .unwrap();
}

///////////////////////////////////////////////////////////////////////
/// REST SERVER
///////////////////////////////////////////////////////////////////////

/// Get all vertiports in a region
///
/// List all vertiport items from svc-storage
#[utoipa::path(
    post,
    path = "/cargo/vertiports",
    request_body = rest_types::VertiportsQuery,
    responses(
        (status = 202, description = "List all cargo-accessible vertiports successfully", body = [rest_types::Vertiport]),
        (status = 404, description = "Unable to get vertiports.")
    )
)]
pub async fn query_vertiports(
    Extension(mut grpc_clients): Extension<GrpcClients>,
    Json(_payload): Json<rest_types::VertiportsQuery>,
) -> Result<Json<Vec<rest_types::Vertiport>>, (StatusCode, String)> {
    // Will provide Lat, Long
    let request = tonic::Request::new(grpc_clients::SearchFilter {
        search_field: "".to_string(),
        search_value: "".to_string(),
        page_number: 1,
        results_per_page: 50,
    });

    // Get Client
    let client_option = grpc_clients.storage.get_client().await;
    if client_option.is_none() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "storage service is unavailable.".to_string(),
        ));
    }
    let mut client = client_option.unwrap();

    // Make request, process response
    let response = client.vertiports(request).await;
    match response {
        Ok(r) => {
            let ret = r
                .into_inner()
                .vertiports
                .into_iter()
                .map(|x| {
                    let data = x.data.unwrap();
                    rest_types::Vertiport {
                        id: x.id,
                        label: data.description,
                        latitude: data.latitude,
                        longitude: data.longitude,
                    }
                })
                .collect();

            Ok(Json(ret))
        }
        Err(e) => Err((StatusCode::CONFLICT, e.to_string())),
    }
}

/// Search FlightOptions by query params.
///
/// Search `FlightOption`s by query params and return matching `FlightOption`s.
#[utoipa::path(
    post,
    path = "/cargo/query",
    request_body = rest_types::FlightQuery,
    responses(
        (status = 202, description = "List possible flights", body = [rest_types::FlightOption])
    )
)]
pub async fn query_flight(
    Extension(mut grpc_clients): Extension<GrpcClients>,
    Json(payload): Json<rest_types::FlightQuery>,
) -> Result<Json<Vec<rest_types::FlightOption>>, (StatusCode, String)> {
    // Reject extreme weights
    let weight_g: u32 = (payload.cargo_weight_kg * 1000.0) as u32;
    if weight_g >= MAX_CARGO_WEIGHT_G {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Request cargo weight exceeds {MAX_CARGO_WEIGHT_G}."),
        ));
    }

    // Check UUID validity
    if !is_uuid(&payload.vertiport_arrive_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Arrival vertiport ID is not UUID format.".to_string(),
        ));
    }

    if !is_uuid(&payload.vertiport_depart_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Departure vertiport ID is not UUID format.".to_string(),
        ));
    }

    let mut flight_query = grpc_clients::QueryFlightRequest {
        is_cargo: true,
        persons: None,
        weight_grams: Some(weight_g),
        vertiport_depart_id: payload.vertiport_depart_id,
        vertiport_arrive_id: payload.vertiport_arrive_id,
        arrival_time: None,
        departure_time: None,
    };

    let current_time = SystemTime::now();

    let by_arrival: bool =
        payload.timestamp_arrive_min.is_some() && payload.timestamp_arrive_max.is_some();
    let by_departure: bool =
        payload.timestamp_depart_min.is_some() && payload.timestamp_depart_max.is_some();

    // Time windows are properly specified
    if by_arrival {
        let timestamp = payload.timestamp_arrive_max.unwrap();
        if timestamp <= current_time {
            return Err((
                StatusCode::BAD_REQUEST,
                "Provided time is in the past.".to_string(),
            ));
        }

        flight_query.arrival_time = Some(timestamp.into());
    } else if by_departure {
        let timestamp = payload.timestamp_depart_max.unwrap();

        if timestamp <= current_time {
            return Err((
                StatusCode::BAD_REQUEST,
                "Provided time is in the past.".to_string(),
            ));
        }

        flight_query.departure_time = Some(timestamp.into());
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid time window provided.".to_string(),
        ));
    }

    let request = tonic::Request::new(flight_query);
    // Get Client
    let client_option = grpc_clients.scheduler.get_client().await;
    if client_option.is_none() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "scheduler service unavailable.".to_string(),
        ));
    }
    let mut client = client_option.unwrap();

    // Make request, process response
    let response = client.query_flight(request).await;
    match response {
        Ok(r) => {
            let ret = r
                .into_inner()
                .flights
                .into_iter()
                .map(|x| rest_types::FlightOption {
                    fp_id: x.id,
                    vertiport_depart_id: x.vertiport_id_departure.to_string(),
                    vertiport_arrive_id: x.vertiport_id_destination.to_string(),
                    timestamp_depart: SystemTime::try_from(x.estimated_departure.unwrap()).unwrap(),
                    timestamp_arrive: SystemTime::try_from(x.estimated_arrival.unwrap()).unwrap(),
                })
                .collect();

            Ok(Json(ret))
        }
        Err(e) => Err((StatusCode::CONFLICT, e.to_string())),
    }
}

/// Confirm a Flight
///
/// Tries to confirm a flight with the svc-scheduler
#[utoipa::path(
    put,
    path = "/cargo/confirm",
    request_body = rest_types::FlightConfirm,
    responses(
        (status = 201, description = "Flight Confirmed", body = String),
        (status = 409, description = "Flight Confirmation Failed", body = rest_types::ConfirmError)
    ),
    security(
        (), // <-- make optional authentication
        ("api_key" = [])
    )
)]
pub async fn confirm_flight(
    Extension(mut grpc_clients): Extension<GrpcClients>,
    Json(payload): Json<rest_types::FlightConfirm>,
    _headers: HeaderMap,
) -> Result<(), (StatusCode, String)> {
    if !is_uuid(&payload.fp_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid flight plan UUID.".to_string(),
        ));
    }

    let request = tonic::Request::new(grpc_clients::Id { id: payload.fp_id });

    // Get Client
    let client_option = grpc_clients.scheduler.get_client().await;
    if client_option.is_none() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "scheduler service is unavailable.".to_string(),
        ));
    }
    let mut client = client_option.unwrap();

    // Make request, process response
    let response = client.confirm_flight(request).await;
    match response {
        Ok(r) => {
            let ret = r.into_inner();
            if ret.confirmed {
                Ok(())
            } else {
                Err((
                    StatusCode::CONFLICT,
                    "Could not confirm flight.".to_string(),
                ))
            }
        }
        Err(e) => Err((StatusCode::CONFLICT, e.to_string())),
    }
}

/// Cancel flight
///
/// Tell svc-scheduler to cancel a flight
#[utoipa::path(
    delete,
    path = "/cargo/cancel",
    responses(
        (status = 200, description = "Flight cancelled successfully"),
        (status = 404, description = "FlightOption not found")
    ),
    request_body = rest_types::FlightCancel,
    security(
        (), // <-- make optional authentication
        ("api_key" = [])
    )
)]
pub async fn cancel_flight(
    Extension(mut grpc_clients): Extension<GrpcClients>,
    Json(payload): Json<rest_types::FlightCancel>,
    _headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, String)> {
    if !is_uuid(&payload.fp_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid flight plan UUID.".to_string(),
        ));
    }

    let request = tonic::Request::new(grpc_clients::Id { id: payload.fp_id });

    // Get Client
    let client_option = grpc_clients.scheduler.get_client().await;
    if client_option.is_none() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "scheduler service unavailable.".to_string(),
        ));
    }
    let mut client = client_option.unwrap();

    // Make request, process response
    let response = client.cancel_flight(request).await;
    match response {
        Ok(r) => {
            let ret = r.into_inner();
            if ret.cancelled {
                Ok(StatusCode::OK)
            } else {
                Err((StatusCode::CONFLICT, ret.reason))
            }
        }
        Err(e) => Err((StatusCode::CONFLICT, e.to_string())),
    }
}

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
pub async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("expect tokio signal ctrl-c");
    println!("signal shutdown!");
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
            query_flight,
            query_vertiports,
            confirm_flight,
            cancel_flight
        ),
        components(
            schemas(
                rest_types::FlightOption,
                rest_types::Vertiport,
                rest_types::ConfirmStatus,
                rest_types::VertiportsQuery,
                rest_types::FlightQuery
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
        .route(rest_types::ENDPOINT_CANCEL, routing::delete(cancel_flight))
        .route(rest_types::ENDPOINT_QUERY, routing::post(query_flight))
        .route(rest_types::ENDPOINT_CONFIRM, routing::put(confirm_flight))
        .route(
            rest_types::ENDPOINT_VERTIPORTS,
            routing::post(query_vertiports),
        )
        .layer(Extension(grpc_clients)); // Extension layer must be last

    println!("REST API Hosted at 0.0.0.0:{rest_port}");
    let address = format!("[::]:{rest_port}").parse().unwrap();
    axum::Server::bind(&address)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

#[tokio::main]
async fn main() -> Result<(), tonic::transport::Error> {
    // Start GRPC Server
    tokio::spawn(grpc_server());

    // Wait for other GRPC Servers
    let grpc_clients = grpc_clients::GrpcClients::default();
    // grpc_clients.connect_all().await;

    // Start REST API
    rest_server(grpc_clients).await;

    println!("Server shutdown.");
    Ok(())
}
