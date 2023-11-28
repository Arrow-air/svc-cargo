use super::rest_types::{CargoScan, QueryParcelRequest, QueryParcelResponse};
use super::rest_types::{
    Occupation, QueryScheduleRequest, QueryScheduleResponse, TimeWindow, MAX_LANDINGS_TO_RETURN,
};
use super::rest_types::{QueryVertiportsRequest, Vertiport};
use super::utils::is_uuid;
use crate::grpc::client::GrpcClients;
use axum::{extract::Extension, Json};
use hyper::StatusCode;
use svc_storage_client_grpc::prelude::{AdvancedSearchFilter, SortOption, SortOrder};
use svc_storage_client_grpc::simple_service::Client;

/// Get Regional Vertiports
#[utoipa::path(
    post,
    path = "/cargo/vertiports",
    tag = "svc-cargo",
    request_body = QueryVertiportsRequest,
    responses(
        (status = 200, description = "List all cargo-accessible vertiports successfully", body = [Vertiport]),
        (status = 500, description = "Unable to get vertiports."),
        (status = 503, description = "Could not connect to other microservice dependencies")
    )
)]
pub async fn query_vertiports(
    Extension(grpc_clients): Extension<GrpcClients>,
    Json(payload): Json<QueryVertiportsRequest>,
) -> Result<Json<Vec<Vertiport>>, StatusCode> {
    rest_debug!("(query_vertiports) entry.");

    //
    // 1 degree of latitude ~= 69 miles
    // 1 degree of longitude ~= 55 miles
    //
    let degree_range: f32 = 2.0;
    let filter = AdvancedSearchFilter::search_between(
        "latitude".to_owned(),
        (payload.latitude + degree_range).to_string(),
        (payload.latitude - degree_range).to_string(),
    )
    .and_between(
        "longitude".to_owned(),
        (payload.longitude + degree_range).to_string(),
        (payload.longitude - degree_range).to_string(),
    );

    // Make request, process response
    let Ok(response) = grpc_clients.storage.vertiport.search(filter).await else {
        let error_msg = "error response from svc-storage.".to_string();
        rest_error!("(query_vertiports) {}.", &error_msg);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let mut vertiports: Vec<Vertiport> = vec![];
    for obj in response.into_inner().list {
        let Some(data) = obj.data else {
            rest_error!("(query_vertiports) vertiport data is None.");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        };

        let Some(location) = data.geo_location else {
            rest_error!("(query_vertiports) vertiport location is None.");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        };

        let Some(exterior) = location.exterior else {
            rest_error!("(query_vertiports) vertiport exterior is None.");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        };

        let points = exterior.points;
        let latitude = points.iter().map(|pt| pt.latitude).sum::<f64>() / points.len() as f64;
        let longitude = points.iter().map(|pt| pt.longitude).sum::<f64>() / points.len() as f64;

        vertiports.push(Vertiport {
            id: obj.id,
            label: data.description,
            latitude: latitude as f32,
            longitude: longitude as f32,
        })
    }

    rest_info!("(query_vertiports) found {} vertiports.", vertiports.len());
    Ok(Json(vertiports))
}

/// Request a list of occupations for a vertiport.
/// No more than [`MAX_LANDINGS_TO_RETURN`] occupations will be returned.
#[utoipa::path(
    get,
    path = "/cargo/occupations",
    tag = "svc-cargo",
    responses(
        (status = 200, description = "Occupations retrieved successfully"),
        (status = 400, description = "Request body is invalid format"),
        (status = 500, description = "Dependencies returned error"),
        (status = 503, description = "Could not connect to other microservice dependencies")
    ),
    request_body = QueryScheduleRequest
)]
pub async fn query_occupations(
    Extension(grpc_clients): Extension<GrpcClients>,
    Json(payload): Json<QueryScheduleRequest>,
) -> Result<Json<QueryScheduleResponse>, StatusCode> {
    rest_debug!("(query_occupations) entry.");

    if payload.limit > MAX_LANDINGS_TO_RETURN {
        let error_msg = format!(
            "requested number of occupations exceeds maximum of {}.",
            MAX_LANDINGS_TO_RETURN
        );
        rest_error!("(query_occupations) {}", &error_msg);
        return Err(StatusCode::BAD_REQUEST);
    }

    if !is_uuid(&payload.vertiport_id) {
        rest_error!(
            "(query_occupations) vertiport ID not in UUID format: {}",
            payload.vertiport_id
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    let Some(arrival_window) = payload.arrival_window else {
        let error_msg = "arrival window not specified.".to_string();
        rest_error!("(query_occupations) {}", &error_msg);
        return Err(StatusCode::BAD_REQUEST);
    };

    //
    // Request flight plans
    //
    let mut filter = AdvancedSearchFilter::search_equals(
        "destination_vertiport_id".to_string(),
        payload.vertiport_id.clone(),
    )
    .and_between(
        "scheduled_arrival".to_string(),
        arrival_window.timestamp_min.to_string(),
        arrival_window.timestamp_max.to_string(),
    );
    filter.results_per_page = payload.limit as i32;
    filter.order_by = vec![SortOption {
        sort_field: "scheduled_arrival".to_string(),
        sort_order: SortOrder::Asc as i32,
    }];

    let response = match grpc_clients.storage.flight_plan.search(filter).await {
        Ok(response) => response.into_inner(),
        Err(e) => {
            rest_error!("(query_occupations) svc-storage error. {:?}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let mut occupations: Vec<Occupation> = vec![];
    for fp in response.list {
        let Some(data) = fp.data else {
            rest_error!("(query_occupations) flight plan data is None.");

            // Skip this flight plan, other occupations should still be reported
            continue;
        };

        let Some(timestamp_min) = data.target_timeslot_start else {
            rest_error!("(query_occupations) flight plan has no scheduled arrival.");

            // Skip this flight plan, other occupations should still be reported
            continue;
        };

        let Some(timestamp_max) = data.target_timeslot_end else {
            rest_error!("(query_occupations) flight plan has no scheduled arrival.");

            // Skip this flight plan, other occupations should still be reported
            continue;
        };

        let vertipad_display_name =
            match super::utils::get_vertipad_details(&data.target_vertipad_id, &grpc_clients).await
            {
                Ok(vertipad) => Some(vertipad.name),
                Err(e) => {
                    let error_msg = "svc-storage error.".to_string();
                    rest_error!("(query_occupations) {} {:?}", &error_msg, e);
                    None
                }
            };

        let aircraft_nickname =
            match super::utils::get_vehicle_details(&data.vehicle_id, &grpc_clients).await {
                Ok(vehicle) => Some(vehicle.registration_number),
                Err(e) => {
                    let error_msg = "svc-storage error.".to_string();
                    rest_error!("(query_occupations) {} {:?}", &error_msg, e);
                    None
                }
            };

        occupations.push(Occupation {
            flight_plan_id: fp.id,
            vertipad_id: data.target_vertipad_id,
            vertipad_display_name,
            time_window: TimeWindow {
                timestamp_min: timestamp_min.into(),
                timestamp_max: timestamp_max.into(),
            },
            aircraft_id: data.vehicle_id,
            aircraft_nickname,
            cargo_acquire: vec![],
            cargo_deliver: vec![],
        })
    }

    Ok(Json(QueryScheduleResponse { occupations }))
}

/// Request a list of occupations for a vertiport.
/// No more than [`MAX_LANDINGS_TO_RETURN`] occupations will be returned.
#[utoipa::path(
    get,
    path = "/cargo/track",
    tag = "svc-cargo",
    responses(
        (status = 200, description = "Parcel scans retrieved successfully"),
        (status = 400, description = "Request body is invalid format"),
        (status = 500, description = "Dependencies returned error"),
        (status = 503, description = "Could not connect to other microservice dependencies")
    ),
    request_body = QueryParcelRequest
)]
pub async fn query_scans(
    Extension(grpc_clients): Extension<GrpcClients>,
    Json(payload): Json<QueryParcelRequest>,
) -> Result<Json<QueryParcelResponse>, StatusCode> {
    rest_debug!("(query_scans) entry.");
    if !is_uuid(&payload.parcel_id) {
        rest_error!(
            "(query_scans) parcel ID not in UUID format: {}",
            payload.parcel_id
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    //
    // Request flight plans
    //
    let mut filter =
        AdvancedSearchFilter::search_equals("parcel_id".to_string(), payload.parcel_id.clone());

    filter.order_by = vec![SortOption {
        sort_field: "created_at".to_string(),
        sort_order: SortOrder::Asc as i32,
    }];

    let response = match grpc_clients.storage.parcel_scan.search(filter).await {
        Ok(response) => response.into_inner().list,
        Err(e) => {
            let error_msg = "svc-storage error.".to_string();
            rest_error!("(query_scans) {} {:?}", &error_msg, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let scans: Vec<CargoScan> = response
        .into_iter()
        .filter_map(|scan| {
            let Some(data) = scan.data else {
                rest_error!("(query_scans) No data in parcel scan data for {}.", scan.id);
                return None;
            };

            let Some(geo_location) = data.geo_location else {
                rest_error!(
                    "(query_scans) No geo_location in parcel scan data for {}.",
                    scan.id
                );

                return None;
            };

            let Some(created_at) = data.created_at else {
                rest_error!(
                    "(query_scans) No created_at in parcel scan data for {}.",
                    scan.id
                );

                return None;
            };

            Some(CargoScan {
                cargo_id: scan.id,
                scanner_id: data.scanner_id,
                latitude: geo_location.latitude,
                longitude: geo_location.longitude,
                timestamp: created_at.into(),
            })
        })
        .collect::<Vec<CargoScan>>();

    Ok(Json(QueryParcelResponse { scans }))
}
