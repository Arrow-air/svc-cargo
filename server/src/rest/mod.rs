#[macro_use]
pub mod macros;
pub mod server;

pub(crate) mod api;
use api::*;

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        request::request_flight,
        query::query_vertiports,
        create::create_itinerary,
        cancel::cancel_itinerary,
        scan::scan_parcel,
        query::query_occupations,
        query::query_scans,
        health::health_check
    ),
    components(
        schemas(
            rest_types::Itinerary,
            rest_types::FlightPlan,
            rest_types::Vertiport,
            rest_types::QueryVertiportsRequest,
            rest_types::ItineraryCancelRequest,
            rest_types::QueryItineraryRequest,
            rest_types::DraftItinerary,
            rest_types::ItineraryCreateRequest,
            rest_types::CargoScan,
            rest_types::TimeWindow,
            rest_types::Occupation,
            rest_types::CargoInfo,
            rest_types::CurrencyUnit,
            rest_types::QueryScheduleRequest,
            rest_types::QueryScheduleResponse,
            rest_types::QueryParcelRequest,
            rest_types::QueryParcelResponse,
            rest_types::GeoPoint,
            rest_types::PaymentInfo,
            rest_types::InvoiceItem
        )
    ),
    tags(
        (name = "svc-cargo", description = "svc-cargo REST API")
    )
)]
struct ApiDoc;

/// Create OpenAPI3 Specification File
pub fn generate_openapi_spec(target: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = ApiDoc::openapi()
        .to_pretty_json()
        .expect("(ERROR) unable to write openapi specification to json.");

    std::fs::write(target, output).expect("(ERROR) unable to write json string to file.");

    Ok(())
}
