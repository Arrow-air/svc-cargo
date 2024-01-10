pub use super::rest_types::{
    CargoInfo, CurrencyUnit, Itinerary, ItineraryCreateRequest, SchedulerFlightPlan,
};
use crate::cache::pool::ItineraryPool;
use crate::grpc::client::GrpcClients;
use axum::{extract::Extension, Json};
use chrono::{DateTime, Duration, Utc};
use hyper::StatusCode;
use num_traits::FromPrimitive;
use svc_scheduler_client_grpc::client::{
    CreateItineraryRequest, TaskRequest, TaskResponse, TaskStatus, TaskStatusRationale,
};
use svc_scheduler_client_grpc::prelude::scheduler_storage::flight_plan::FlightPriority;
use svc_scheduler_client_grpc::prelude::SchedulerServiceClient;
use svc_storage_client_grpc::link_service::Client as LinkClient;
use svc_storage_client_grpc::prelude::flight_plan_parcel::RowData as FlightPlanParcel;
use svc_storage_client_grpc::prelude::Id as StorageId;
use svc_storage_client_grpc::prelude::{AdvancedSearchFilter, SortOption, SortOrder};
use svc_storage_client_grpc::simple_service::Client as SimpleClient;
use svc_storage_client_grpc::simple_service_linked::Client as SimpleLinkedClient;

/// Polling interval for scheduler task statuses
const SCHEDULER_TASK_POLL_INTERVAL_SECONDS: u64 = 5;

/// Timeout for scheduler task statuses
const SCHEDULER_TASK_TIMEOUT_SECONDS: i64 = 60;

// use svc_storage_client_grpc::resources::itinerary;
use svc_storage_client_grpc::resources::parcel::{Data as ParcelData, ParcelStatus};

///
/// Charge the customer for the itinerary
///  If 'dry_run' is true, only check the validity of the
///  payment option first.
async fn payment_confirm(
    // TODO(R5): user credential or UUID
    _total: f32,
    _currency_unit: CurrencyUnit,
    dry_run: bool,
) -> Result<(), StatusCode> {
    rest_debug!("(payment_method_check) entry.");
    //
    // TODO(R5): Check if payment options are valid
    //
    // Possibly query storage for payment information
    // Credit Card, ACH, Cryptocurrency Wallet, etc.
    // In the case of crypto, verify wallet has sufficient
    // funds

    if dry_run {
        return Ok(());
    }

    //
    // TODO(R5): payment service confirm
    // If payment doesn't work here for some reason,
    //  add a scheduler task to cancel the itinerary

    Ok(())
}

/// Pull the itinerary details from Redis
async fn get_draft_itinerary(itinerary_id: &str) -> Result<Itinerary, StatusCode> {
    rest_debug!("(get_draft_itinerary) getting itinerary from redis.");

    let Some(mut pool) = crate::cache::pool::get_pool().await else {
        rest_error!("(get_draft_itinerary) unable to get redis pool.");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    pool.get_itinerary(itinerary_id.to_string())
        .await
        .map_err(|e| {
            rest_error!("(get_draft_itinerary) unable to get itinerary from redis: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

/// Make a request to the scheduler to create an itinerary
async fn scheduler_request(
    itinerary: &Itinerary,
    expiry: DateTime<Utc>,
    grpc_clients: &GrpcClients,
) -> Result<TaskResponse, StatusCode> {
    rest_debug!("(scheduler_request) creating itinerary with scheduler.");

    let Ok(flight_plans) = itinerary
        .flight_plans
        .clone()
        .into_iter()
        .map(|fp| fp.try_into())
        .collect::<Result<Vec<SchedulerFlightPlan>, _>>()
    else {
        rest_error!("(scheduler_request) invalid flight plan data.");
        return Err(StatusCode::BAD_REQUEST);
    };

    let data = CreateItineraryRequest {
        flight_plans,
        priority: FlightPriority::Low as i32,
        expiry: Some(expiry.into()),
        user_id: itinerary.user_id.clone(),
    };

    grpc_clients
        .scheduler
        .create_itinerary(data)
        .await
        .map_err(|e| {
            let error_msg = "svc-scheduler error.".to_string();
            rest_error!("(scheduler_request) {} {:?}", &error_msg, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
        .map(|response| response.into_inner())
}

/// Poll the scheduler for the task status for a set amount of time
async fn scheduler_poll(
    task_id: i64,
    expiry: DateTime<Utc>,
    grpc_clients: GrpcClients,
) -> Result<(), StatusCode> {
    rest_debug!("(scheduler_poll) polling scheduler for task status.");

    // Poll scheduler every few seconds
    let interval = tokio::time::Duration::from_secs(SCHEDULER_TASK_POLL_INTERVAL_SECONDS);
    let request = TaskRequest { task_id };

    // TODO(R4): Tasks from svc-cargo should expire after N seconds
    //  Provide expiry in request to the scheduler.
    while Utc::now() < expiry {
        // give the scheduler time to process the request
        tokio::time::sleep(interval).await;

        let task = match grpc_clients.scheduler.get_task_status(request).await {
            Ok(response) => response.into_inner(),
            Err(e) => {
                let error_msg = "svc-scheduler error.".to_string();
                rest_error!("(create_itinerary) {} {:?}", &error_msg, e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        let Some(metadata) = task.task_metadata else {
            rest_error!("(create_itinerary) no metadata for task: {:?}", task);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        };

        let Some(status) = FromPrimitive::from_i32(metadata.status) else {
            rest_error!(
                "(create_itinerary) unrecognized task status: {:?}",
                metadata.status
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        };

        match status {
            TaskStatus::Queued => {
                // Do nothing
            }
            TaskStatus::Complete => return Ok(()),
            TaskStatus::NotFound => {
                rest_error!("(create_itinerary) svc-scheduler error.");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            TaskStatus::Rejected => {
                rest_warn!(
                    "(create_itinerary) task was rejected by the scheduler: {}",
                    metadata
                        .status_rationale
                        .unwrap_or(TaskStatusRationale::InvalidAction as i32)
                );
                return Err(StatusCode::NOT_MODIFIED);
            }
        }
    }

    rest_warn!("(create_itinerary) task timed out.");

    // Fire off task cancellation and don't wait
    tokio::spawn(async move {
        let _ = grpc_clients.scheduler.cancel_task(request).await;
    });

    Err(StatusCode::REQUEST_TIMEOUT)
}

/// Create the parcel/book the seat
async fn create_cargo(
    itinerary: &Itinerary,
    acquisition_vertiport_id: &str,
    delivery_vertiport_id: &str,
    grpc_clients: &GrpcClients,
) -> Result<CargoInfo, StatusCode> {
    //
    // TODO(R5): Doing all of these in a transaction would be
    //  nice, to rollback any changes if there are errors at
    //  any point. For now we'll have orphaned records if there
    //  are issues.
    //

    ///////////////////////////////
    // Register Parcel with Storage
    ///////////////////////////////
    let data = ParcelData {
        user_id: itinerary.user_id.clone(),
        weight_grams: itinerary.cargo_weight_g,
        status: ParcelStatus::Notdroppedoff as i32,
    };

    // TODO(R4): Push to queue, in case this call fails need a retry mechanism
    // Make request, process response
    let response = match grpc_clients.storage.parcel.insert(data).await {
        Ok(response) => response.into_inner(),
        Err(e) => {
            let error_msg = "svc-parcel-storage error.".to_string();
            rest_error!("(create_itinerary) {} {:?}", &error_msg, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let Some(object) = response.object else {
        let error_msg = "svc-parcel-storage insert fail.".to_string();
        rest_error!("(create_itinerary) {}", &error_msg);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let cargo_id = object.id;

    ///////////////////////////////
    // Get the itinerary and flight plans just created for user
    ///////////////////////////////

    //
    // TODO(R5): this is a bit hacky. with the asynchronous task-based scheduler approach,
    //  maybe return the itinerary id in the TaskResponse in the future
    let mut filter =
        AdvancedSearchFilter::search_equals("user_id".to_string(), itinerary.user_id.clone());
    filter.order_by = vec![SortOption {
        sort_field: "created_at".to_string(),
        sort_order: SortOrder::Desc as i32,
    }];
    filter.results_per_page = 1;

    let db_itinerary = match grpc_clients.storage.itinerary.search(filter).await {
        Ok(response) => match response.into_inner().list.pop() {
            Some(itinerary) => itinerary,
            None => {
                let error_msg = "svc-storage error, no itineraries found.".to_string();
                rest_error!("(create_itinerary) {}", &error_msg);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        },
        Err(e) => {
            let error_msg = "error on request to svc-storage.".to_string();
            rest_error!("(create_itinerary) {} {:?}", &error_msg, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    //
    // Get the linked flight plans
    // Need the IDs of the flight plans to update the flight_plan_parcel table
    let flight_plans = match grpc_clients
        .storage
        .itinerary_flight_plan_link
        .get_linked(StorageId {
            id: db_itinerary.id,
        })
        .await
    {
        Ok(response) => response.into_inner().list,
        Err(e) => {
            let error_msg = "error on request to svc-storage.".to_string();
            rest_error!("(create_itinerary) {} {:?}", &error_msg, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    //
    // Acquisition flight plan
    //  The itinerary itself has no knowledge of what the "start point" is for
    //  a package. Could be the first flight, or the second flight (with
    //  the first being deadhead). Similarly, the delivery flight
    //  could be the last or second to last flight plan.
    //
    // We could maybe indicate which flight(s) are the acquisition and delivery
    //  in the itinerary record itself, so we don't have to search here.
    let Some(flight_plan) = flight_plans.iter().find(|fp| match &fp.data {
        Some(data) => data.origin_vertiport_id == Some(acquisition_vertiport_id.to_string()),
        None => false,
    }) else {
        let error_msg = "flight plan not found.".to_string();
        rest_error!("(create_itinerary) {}", &error_msg);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let acquisition = FlightPlanParcel {
        flight_plan_id: flight_plan.id.clone(),
        parcel_id: cargo_id.clone(),
        acquire: true,
        deliver: false,
    };

    let Some(flight_plan) = flight_plans.iter().find(|fp| match &fp.data {
        Some(data) => data.target_vertiport_id == Some(delivery_vertiport_id.to_string()),
        None => false,
    }) else {
        let error_msg = "flight plan not found.".to_string();
        rest_error!("(create_itinerary) {}", &error_msg);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let delivery = FlightPlanParcel {
        flight_plan_id: flight_plan.id.clone(),
        parcel_id: cargo_id.clone(),
        acquire: false,
        deliver: true,
    };

    let _ = match grpc_clients
        .storage
        .flight_plan_parcel
        .insert(acquisition)
        .await
    {
        Ok(response) => response.into_inner(),
        Err(e) => {
            let error_msg =
                "svc-storage error inserting flight_plan_parcel link (acquisition).".to_string();
            rest_error!("(create_itinerary) {} {:?}", &error_msg, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let _ = match grpc_clients
        .storage
        .flight_plan_parcel
        .insert(delivery)
        .await
    {
        Ok(response) => response.into_inner(),
        Err(e) => {
            let error_msg =
                "svc-storage error inserting flight_plan_parcel link (delivery).".to_string();
            rest_error!("(create_itinerary) {} {:?}", &error_msg, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    Ok(CargoInfo {
        cargo_id,
        // cargo_nickname
    })
}

/// Confirm an itinerary
/// This will create an itinerary with the scheduler, and will register the parcel with
///  the storage service.
#[utoipa::path(
    put,
    path = "/cargo/create",
    tag = "svc-cargo",
    request_body = ItineraryCreateRequest,
    responses(
        (status = 200, description = "Itinerary created."),
        (status = 400, description = "Request body is invalid format"),
        (status = 500, description = "Microservice dependency returned error"),
        (status = 503, description = "Could not connect to other microservice dependencies")
    )
)]
pub async fn create_itinerary(
    Extension(grpc_clients): Extension<GrpcClients>,
    Json(payload): Json<ItineraryCreateRequest>,
) -> Result<(), StatusCode> {
    rest_debug!("(create_itinerary) entry.");

    //
    // See if itinerary id exists
    let itinerary = get_draft_itinerary(&payload.id).await?;
    let invoice_total = itinerary.invoice.iter().map(|i| i.cost).sum::<f32>();

    //
    // Check if payment options are valid/sufficient funds
    //  in the case of cryptocurrency
    payment_confirm(
        invoice_total,
        itinerary.currency_unit,
        true, // dry run, don't charge the customer
    )
    .await?;

    //
    // Ask the scheduler to attempt to create the itinerary
    // This will "reserve" the weight/seats as well so we can
    //  create the parcel record in storage later without conflicts.
    let expiry = Utc::now() + Duration::seconds(SCHEDULER_TASK_TIMEOUT_SECONDS);
    let task_id = scheduler_request(&itinerary, expiry, &grpc_clients)
        .await?
        .task_id;

    //
    // Poll the scheduler for the task status for a set amount of time
    scheduler_poll(task_id, expiry, grpc_clients.clone()).await?;

    //
    // Create the parcel/book the seat
    //
    let _ = create_cargo(
        &itinerary,
        &itinerary.acquisition_vertiport_id,
        &itinerary.delivery_vertiport_id,
        &grpc_clients,
    )
    .await?;

    //
    // If the scheduler task was successful, charge the customer
    //
    payment_confirm(invoice_total, itinerary.currency_unit, false).await?;

    Ok(())
}
