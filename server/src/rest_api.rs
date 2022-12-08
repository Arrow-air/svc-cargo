use axum::{extract::Extension, Json};
use hyper::{HeaderMap, StatusCode};
use std::time::SystemTime;

use crate::{req_debug, req_error, req_info};

use crate::grpc_clients::{
    Channel, GrpcClients, Id, PricingClient, PricingRequest, QueryFlightPlan, QueryFlightRequest,
    SchedulerRpcClient, SearchFilter, ServiceType,
};

/// Types Used in REST Messages
pub mod rest_types {
    include!("../../openapi/types.rs");
}

pub use rest_types::{
    FlightCancel, FlightConfirm, FlightOption, FlightQuery, Vertiport, VertiportsQuery,
};

///////////////////////////////////////////////////////////////////////
/// Constants
///////////////////////////////////////////////////////////////////////

/// Don't allow excessively heavy loads
const MAX_CARGO_WEIGHT_G: u32 = 1_000_000; // 1000 kg

///////////////////////////////////////////////////////////////////////
/// Helpers
///////////////////////////////////////////////////////////////////////

/// Verifies if a given string is UUID format
/// # Arguments
/// * s - string to check
/// # Returns
/// true if UUID format, false otherwise
fn is_uuid(s: &str) -> bool {
    uuid::Uuid::parse_str(s).is_ok()
}

/// Parses the incoming flight plans for information the customer wants
///
/// # Arguments
/// * plan - The flight plan to parse for customer-relevant data
///
/// # Returns
/// Some FlightOption object or None if the flight plan could not be parsed.
fn parse_flight(plan: &QueryFlightPlan) -> Option<FlightOption> {
    let time_depart: SystemTime;
    let time_arrive: SystemTime;

    if let Some(prost_time) = plan.estimated_departure.clone() {
        if let Ok(sys_time) = SystemTime::try_from(prost_time) {
            time_depart = sys_time;
        } else {
            req_error!("(parse_flight) could not convert departure time; discarding.");
            return None;
        };
    } else {
        req_error!("(parse_flight) no departure time in flight plan; discarding.");
        return None;
    };

    if let Some(prost_time) = plan.estimated_arrival.clone() {
        if let Ok(sys_time) = SystemTime::try_from(prost_time) {
            time_arrive = sys_time;
        } else {
            req_error!("(parse_flight) could not convert arrival time; discarding.");
            return None;
        };
    } else {
        req_error!("(parse_flight) no arrival time in flight plan; discarding.");
        return None;
    };

    Some(FlightOption {
        fp_id: plan.id.clone(),
        vertiport_depart_id: plan.vertiport_depart_id.to_string(),
        vertiport_arrive_id: plan.vertiport_arrive_id.to_string(),
        timestamp_depart: time_depart,
        timestamp_arrive: time_arrive,
        distance_m: plan.estimated_distance as f32,
        base_pricing: None,
        currency_type: None,
    })
}

///////////////////////////////////////////////////////////////////////
/// API Handlers
///////////////////////////////////////////////////////////////////////
/// Get all vertiports in a region
///
/// List all vertiport items from svc-storage
#[utoipa::path(
    post,
    path = "/cargo/vertiports",
    request_body = VertiportsQuery,
    responses(
        (status = 200, description = "List all cargo-accessible vertiports successfully", body = [Vertiport]),
        (status = 409, description = "Unable to get vertiports."),
        (status = 503, description = "Could not connect to other microservice dependencies")
    )
)]
pub async fn query_vertiports(
    Extension(mut grpc_clients): Extension<GrpcClients>,
    Json(_payload): Json<VertiportsQuery>,
) -> Result<Json<Vec<Vertiport>>, (StatusCode, String)> {
    req_debug!("(query_vertiports) entry.");

    // Will provide Lat, Long
    let request = tonic::Request::new(SearchFilter {
        search_field: "".to_string(),
        search_value: "".to_string(),
        page_number: 1,
        results_per_page: 50,
    });

    // Get Client
    let client_option = grpc_clients.storage.get_client().await;
    if client_option.is_none() {
        let error_msg = "svc-storage unavailable.".to_string();
        req_error!("(query_vertiports) {}", &error_msg);
        return Err((StatusCode::SERVICE_UNAVAILABLE, error_msg));
    }
    let mut client = client_option.unwrap();

    // Make request, process response
    let response = client.vertiports(request).await;
    match response {
        Ok(r) => {
            let ret: Vec<Vertiport> = r
                .into_inner()
                .vertiports
                .into_iter()
                .map(|x| {
                    let data = x.data.unwrap();
                    Vertiport {
                        id: x.id,
                        label: data.description,
                        latitude: data.latitude,
                        longitude: data.longitude,
                    }
                })
                .collect();

            req_info!("(query_vertiports) found {} vertiports.", ret.len());
            Ok(Json(ret))
        }
        Err(e) => {
            let error_msg = format!("error response from svc-storage: {e}");
            req_error!("(query_vertiports) {}", &error_msg);
            Err((StatusCode::CONFLICT, error_msg))
        }
    }
}

/// Search FlightOptions by query params.
///
/// Search `FlightOption`s by query params and return matching `FlightOption`s.
#[utoipa::path(
    post,
    path = "/cargo/query",
    request_body = FlightQuery,
    responses(
        (status = 200, description = "List available flight plans", body = [FlightOption]),
        (status = 400, description = "Request body is invalid format"),
        (status = 409, description = "svc-scheduler or svc-pricing returned error"),
        (status = 503, description = "Could not connect to other microservice dependencies")
    )
)]
pub async fn query_flight(
    Extension(mut grpc_clients): Extension<GrpcClients>,
    Json(payload): Json<FlightQuery>,
) -> Result<Json<Vec<FlightOption>>, (StatusCode, String)> {
    req_debug!("(query_flight) entry.");

    // Reject extreme weights
    let weight_g: u32 = (payload.cargo_weight_kg * 1000.0) as u32;
    if weight_g >= MAX_CARGO_WEIGHT_G {
        let error_msg = format!("request cargo weight exceeds {MAX_CARGO_WEIGHT_G}.");
        req_error!("(query_flight) {}", &error_msg);
        return Err((StatusCode::BAD_REQUEST, error_msg));
    }

    // Check UUID validity
    if !is_uuid(&payload.vertiport_arrive_id) {
        let error_msg = "arrival port ID not UUID format.".to_string();
        req_error!("(query_flight) {}", &error_msg);
        return Err((StatusCode::BAD_REQUEST, error_msg));
    }

    if !is_uuid(&payload.vertiport_depart_id) {
        let error_msg = "departure port ID not UUID format.".to_string();
        req_error!("(query_flight) {}", &error_msg);
        return Err((StatusCode::BAD_REQUEST, error_msg));
    }

    let mut flight_query = QueryFlightRequest {
        is_cargo: true,
        persons: None,
        weight_grams: Some(weight_g),
        vertiport_depart_id: payload.vertiport_depart_id,
        vertiport_arrive_id: payload.vertiport_arrive_id,
        arrival_time: None,
        departure_time: None,
    };

    let current_time = SystemTime::now();

    // Time windows are properly specified
    if let Some(timestamp) = payload.timestamp_arrive_max {
        if timestamp <= current_time {
            req_info!("(query flight) current time: {:?}", current_time);
            let error_msg = format!("max arrival time is in the past: {:?}", timestamp);
            req_error!("(query_flight) {}", &error_msg);
            return Err((StatusCode::BAD_REQUEST, error_msg));
        }

        flight_query.arrival_time = Some(timestamp.into());
    }

    if let Some(timestamp) = payload.timestamp_depart_max {
        if timestamp <= current_time {
            req_info!("(query flight) current time: {:?}", current_time);
            let error_msg = format!("max depart time is in the past: {:?}", timestamp);
            req_error!("(query_flight) {}", &error_msg);
            return Err((StatusCode::BAD_REQUEST, error_msg));
        }

        flight_query.departure_time = Some(timestamp.into());
    }

    if flight_query.departure_time.is_none() && flight_query.arrival_time.is_none() {
        let error_msg = "invalid time window.".to_string();
        req_error!("(query_flight) {}", &error_msg);
        return Err((StatusCode::BAD_REQUEST, error_msg));
    }

    // Get Clients
    let mut scheduler_client: SchedulerRpcClient<Channel>;
    let mut pricing_client: PricingClient<Channel>;

    match grpc_clients.scheduler.get_client().await {
        Some(c) => scheduler_client = c,
        None => {
            let error_msg = "svc-scheduler unavailable.".to_string();
            req_error!("(query_flight) {}", &error_msg);
            return Err((StatusCode::SERVICE_UNAVAILABLE, error_msg));
        }
    };

    match grpc_clients.pricing.get_client().await {
        Some(c) => pricing_client = c,
        None => {
            let error_msg = "svc-pricing unavailable.".to_string();
            req_error!("(query_flight) {}", &error_msg);
            return Err((StatusCode::SERVICE_UNAVAILABLE, error_msg));
        }
    };

    // Make request, process response
    let request = tonic::Request::new(flight_query);
    let response = scheduler_client.query_flight(request).await;
    let mut flights: Vec<FlightOption>;

    match response {
        Ok(r) => {
            flights = r
                .into_inner()
                .flights
                .into_iter()
                .filter_map(|x| parse_flight(&x))
                .collect();

            req_info!("(query_flight) found {} flight options.", flights.len());
        }
        Err(e) => {
            let error_msg = format!("svc-scheduler error: {e}");
            req_error!("(query_flight) {}", &error_msg);
            req_error!("(query_flight) invalidating svc-scheduler client.");
            grpc_clients.scheduler.invalidate().await;
            return Err((StatusCode::CONFLICT, error_msg));
        }
    };

    // StatusUpdate message to customer?
    // e.g. Got your flights! Calculating prices...
    for mut fp in &mut flights {
        let pricing_query = PricingRequest {
            service_type: ServiceType::Cargo as i32,
            distance_km: fp.distance_m / 1000.0,
        };

        // Make request, process response
        let request = tonic::Request::new(pricing_query);
        let response = pricing_client.get_pricing(request).await;
        match response {
            Ok(r) => {
                fp.base_pricing = Some(r.into_inner().price);
                fp.currency_type = Some("usd".to_string());
            }
            Err(e) => {
                let error_msg = format!("svc-pricing error: {e}");
                req_error!("(query_flight) {}", &error_msg);
                req_error!("(query_flight) invalidating svc-pricing client.");
                grpc_clients.pricing.invalidate().await;
                return Err((StatusCode::CONFLICT, error_msg));
            }
        }
    }

    req_debug!("(query_flight) exit with {} flight options", flights.len());
    Ok(Json(flights))
}

/// Confirm a Flight
///
/// Tries to confirm a flight with the svc-scheduler
#[utoipa::path(
    put,
    path = "/cargo/confirm",
    request_body = FlightConfirm,
    responses(
        (status = 200, description = "Flight Confirmed", body = String),
        (status = 400, description = "Request body is invalid format"),
        (status = 409, description = "svc-scheduler returned error"),
        (status = 503, description = "Could not connect to other microservice dependencies")
    )
)]
pub async fn confirm_flight(
    Extension(mut grpc_clients): Extension<GrpcClients>,
    Json(payload): Json<FlightConfirm>,
    _headers: HeaderMap,
) -> Result<String, (StatusCode, String)> {
    req_debug!("(confirm_flight) entry.");

    if !is_uuid(&payload.fp_id) {
        let error_msg = "flight plan ID not in UUID format.".to_string();
        req_error!("(confirm_flight) {}", &error_msg);
        return Err((StatusCode::BAD_REQUEST, error_msg));
    }

    // Get Client
    let client_option = grpc_clients.scheduler.get_client().await;
    if client_option.is_none() {
        let error_msg = "svc-scheduler unavailable.".to_string();
        req_error!("(confirm_flight) {}", &error_msg);
        return Err((StatusCode::SERVICE_UNAVAILABLE, error_msg));
    }
    let mut client = client_option.unwrap();

    // Make request, process response
    let request = tonic::Request::new(Id { id: payload.fp_id });
    let response = client.confirm_flight(request).await;
    match response {
        Ok(r) => {
            let ret = r.into_inner();
            if ret.confirmed {
                req_info!("(confirm_flight) svc-scheduler confirm success.");
                Ok(ret.id)
            } else {
                let error_msg = "svc-scheduler confirm fail.".to_string();
                req_error!("(confirm_flight) {}", &error_msg);
                Err((StatusCode::CONFLICT, error_msg))
            }
        }
        Err(e) => {
            let error_msg = format!("svc-scheduler error: {e}.");
            req_error!("(confirm_flight) {}", &error_msg);
            Err((StatusCode::CONFLICT, error_msg))
        }
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
        (status = 400, description = "Request body is invalid format"),
        (status = 409, description = "svc-scheduler returned error"),
        (status = 503, description = "Could not connect to other microservice dependencies")
    ),
    request_body = FlightCancel
)]
pub async fn cancel_flight(
    Extension(mut grpc_clients): Extension<GrpcClients>,
    Json(payload): Json<FlightCancel>,
    _headers: HeaderMap,
) -> Result<String, (StatusCode, String)> {
    req_debug!("(cancel_flight) entry.");
    if !is_uuid(&payload.fp_id) {
        let error_msg = "flight plan ID not in UUID format.".to_string();
        req_error!("(cancel_flight) {}", &error_msg);
        return Err((StatusCode::BAD_REQUEST, error_msg));
    }

    // Get Client
    let client_option = grpc_clients.scheduler.get_client().await;
    if client_option.is_none() {
        let error_msg = "svc-scheduler unavailable.".to_string();
        req_error!("(cancel_flight) {}", &error_msg);
        return Err((StatusCode::SERVICE_UNAVAILABLE, error_msg));
    }
    let mut client = client_option.unwrap();

    // Make request, process response
    let request = tonic::Request::new(Id { id: payload.fp_id });
    let response = client.cancel_flight(request).await;
    match response {
        Ok(r) => {
            let ret = r.into_inner();
            if ret.cancelled {
                req_info!("(cancel_flight) svc-scheduler cancel success.");
                Ok(ret.id)
            } else {
                let error_msg = format!("svc-scheduler cancel fail: {}", ret.reason);
                req_error!("(cancel_flight) {}", &error_msg);
                Err((StatusCode::CONFLICT, error_msg))
            }
        }
        Err(e) => {
            let error_msg = format!("svc-scheduler request fail: {e}");
            req_error!("(cancel_flight) {}", &error_msg);
            Err((StatusCode::CONFLICT, error_msg))
        }
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn ut_parse_flight() {
        let fp = QueryFlightPlan {
            id: "".to_string(),
            pilot_id: "".to_string(),
            vehicle_id: "".to_string(),
            cargo: [123].to_vec(),
            weather_conditions: "Sunny, no wind :)".to_string(),
            vertiport_depart_id: "".to_string(),
            pad_depart_id: "".to_string(),
            vertiport_arrive_id: "".to_string(),
            pad_arrive_id: "".to_string(),
            estimated_departure: Some(SystemTime::now().into()),
            estimated_arrival: Some(SystemTime::now().into()),
            actual_departure: None,
            actual_arrival: None,
            flight_release_approval: None,
            flight_plan_submitted: None,
            flight_status: 0,
            flight_priority: 0,
            estimated_distance: 1000,
        };
        let ret = parse_flight(&fp);
        assert!(ret.is_some());
        let opt = ret.unwrap();
        assert_eq!(fp.id, opt.fp_id);
        assert_eq!(fp.vertiport_depart_id, opt.vertiport_depart_id);
        assert_eq!(fp.vertiport_arrive_id, opt.vertiport_arrive_id);

        // Bad time arguments
        {
            let mut fp2 = fp.clone();
            fp2.estimated_departure = None;
            assert!(parse_flight(&fp2).is_none());
        }

        {
            let mut fp2 = fp.clone();
            fp2.estimated_arrival = None;
            assert!(parse_flight(&fp2).is_none());
        }
    }
}
