use super::rest_types::ItineraryCancelRequest;
use super::utils::is_uuid;
use crate::grpc::client::GrpcClients;
use axum::{extract::Extension, Json};
use hyper::StatusCode;
use svc_scheduler_client_grpc::prelude::scheduler_storage::flight_plan::FlightPriority;
use svc_scheduler_client_grpc::prelude::SchedulerServiceClient;
use svc_storage_client_grpc::prelude::*;

/// Cancel a Flight
#[utoipa::path(
    delete,
    path = "/cargo/cancel",
    tag = "svc-cargo",
    responses(
        (status = 200, description = "Flight cancelled successfully"),
        (status = 400, description = "Request body is invalid format"),
        (status = 500, description = "svc-scheduler returned error"),
        (status = 503, description = "Could not connect to other microservice dependencies")
    ),
    request_body = ItineraryCancelRequest
)]
pub async fn cancel_itinerary(
    Extension(grpc_clients): Extension<GrpcClients>,
    Json(payload): Json<ItineraryCancelRequest>,
) -> Result<(), StatusCode> {
    rest_debug!("(cancel_itinerary) entry.");
    let itinerary_id = payload.id;
    if !is_uuid(&itinerary_id) {
        let error_msg = "itinerary ID not in UUID format.".to_string();
        rest_error!("(cancel_itinerary) {}", &error_msg);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Make request, process response
    if let Err(e) = grpc_clients
        .scheduler
        .cancel_itinerary(svc_scheduler_client_grpc::client::CancelItineraryRequest {
            priority: FlightPriority::Medium as i32,
            itinerary_id: itinerary_id.clone(),
            user_id: payload.user_id,
        })
        .await
    {
        rest_error!("(cancel_itinerary) svc-scheduler request fail. {:?}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    rest_info!("(cancel_itinerary) cancellation added to scheduler queue.");

    //
    // Get parcel from id
    //
    let filter =
        AdvancedSearchFilter::search_equals("itinerary_id".to_string(), itinerary_id.clone());

    let list = match grpc_clients.storage.parcel.search(filter).await {
        Ok(response) => response.into_inner().list,
        Err(e) => {
            let error_msg = "svc-parcel-storage error.".to_string();
            rest_error!("(cancel_itinerary) {} {:?}", &error_msg, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // An itinerary might have multiple parcels
    // TODO(R4): Push these onto a queue in case any one fails
    let mut ok = true;
    for parcel in list.into_iter() {
        let _ = grpc_clients
            .storage
            .parcel
            .delete(Id { id: parcel.id })
            .await
            .map_err(|e| {
                let error_msg = "svc-parcel-storage error.".to_string();
                rest_error!("(cancel_itinerary) {} {:?}", &error_msg, e);
                // Still try to delete other parcels
                ok = false;
            });
    }

    if !ok {
        rest_error!("(cancel_itinerary) could not delete all parcels.");
    }

    // If the customer's itinerary was cancelled, but the parcels were not, it's still a success for them
    Ok(())
}
