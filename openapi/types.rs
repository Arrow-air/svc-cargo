use chrono::{DateTime, Utc};
/// Types used for REST communication with the svc-cargo server
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use num_traits::FromPrimitive;

/// TODO(R4): Import payment type enums from svc-payment
/// pub use svc_payment_client_grpc::prelude::payment::{PaymentType, CreditCardInfo};

pub use svc_scheduler_client_grpc::prelude::scheduler_storage::{GeoPoint, GeoLineString, flight_plan::Data as SchedulerFlightPlan};

/// Don't allow overly large numbers of occupations to be returned
pub const MAX_LANDINGS_TO_RETURN: u32 = 50;

/// Non-privileged flight plan information
#[derive(Debug, Clone, IntoParams, ToSchema, Deserialize, Serialize)]
pub struct FlightPlan {
    /// The String ID of the vertiport to leave from
    pub origin_vertiport_id: String,

    /// The String ID of the vertipad to leave from
    pub origin_vertipad_id: String,

    /// The String ID of the destination vertiport
    pub target_vertiport_id: String,

    /// The String ID of the destination vertipad
    pub target_vertipad_id: String,

    /// The path of the flight plan
    pub path: Vec<GeoPoint>,

    /// The end of the window of departure
    pub origin_timeslot_start: DateTime<Utc>,

    /// The end of the window of departure
    pub origin_timeslot_end: DateTime<Utc>,

    /// The start of the window of arrival
    pub target_timeslot_start: DateTime<Utc>,

    /// The end of the window of arrival
    pub target_timeslot_end: DateTime<Utc>,

    /// The unique ID of the aircraft
    pub vehicle_id: String,

    /// The priority of the flight plan
    pub flight_priority: i32
}

#[derive(Debug, Copy, Clone)]
/// Flight Plan Error
pub enum FlightPlanError {
    /// Invalid Origin Vertiport ID
    OriginVertiportId,

    /// Invalid Target Vertiport ID
    TargetVertiportId,

    /// Invalid Path
    Path,

    /// Invalid Target Timeslot Start
    TargetTimeslotStart,

    /// Invalid Target Timeslot End
    TargetTimeslotEnd,

    /// Invalid Origin Timeslot Start
    OriginTimeslotStart,

    /// Invalid Origin Timeslot End
    OriginTimeslotEnd,

    /// Invalid Flight Priority
    FlightPriority,
}

impl TryFrom<FlightPlan> for SchedulerFlightPlan {
    type Error = FlightPlanError;

    fn try_from(flight_plan: FlightPlan) -> Result<Self, Self::Error> {
        let Some(flight_priority) = FromPrimitive::from_i32(flight_plan.flight_priority) else {
            return Err(FlightPlanError::FlightPriority);
        };

        Ok(SchedulerFlightPlan {
            origin_vertiport_id: Some(flight_plan.origin_vertiport_id),
            origin_vertipad_id: flight_plan.origin_vertipad_id,
            target_vertiport_id: Some(flight_plan.target_vertiport_id),
            target_vertipad_id: flight_plan.target_vertipad_id,
            path: Some(GeoLineString {
                points: flight_plan.path,
            }),
            target_timeslot_start: Some(flight_plan.target_timeslot_start.into()),
            target_timeslot_end: Some(flight_plan.target_timeslot_end.into()),
            origin_timeslot_start: Some(flight_plan.origin_timeslot_start.into()),
            origin_timeslot_end: Some(flight_plan.origin_timeslot_end.into()),
            vehicle_id: flight_plan.vehicle_id,
            flight_priority,
            ..Default::default()
        })
    }
}

impl TryFrom<SchedulerFlightPlan> for FlightPlan {
    type Error = FlightPlanError;

    fn try_from(flight_plan: SchedulerFlightPlan) -> Result<Self, Self::Error> {
        let Some(origin_vertiport_id) = flight_plan.origin_vertiport_id else {
            return Err(FlightPlanError::OriginVertiportId);
        };

        let Some(target_vertiport_id) = flight_plan.target_vertiport_id else {
            return Err(FlightPlanError::TargetVertiportId);
        };

        let path = match flight_plan.path {
            Some(path) => path.points,
            None => {
                return Err(FlightPlanError::Path);
            }
        };

        let Some(target_timeslot_start) = flight_plan.target_timeslot_start else {
            return Err(FlightPlanError::TargetTimeslotStart);
        };

        let Some(target_timeslot_end) = flight_plan.target_timeslot_end else {
            return Err(FlightPlanError::TargetTimeslotEnd);
        };

        let Some(origin_timeslot_start) = flight_plan.origin_timeslot_start else {
            return Err(FlightPlanError::OriginTimeslotStart);
        };

        let Some(origin_timeslot_end) = flight_plan.origin_timeslot_end else {
            return Err(FlightPlanError::OriginTimeslotEnd);
        };

        Ok(FlightPlan {
            origin_vertiport_id,
            origin_vertipad_id: flight_plan.origin_vertipad_id,
            target_vertipad_id: flight_plan.target_vertipad_id,
            target_vertiport_id,
            vehicle_id: flight_plan.vehicle_id,
            path,
            target_timeslot_start: target_timeslot_start.into(),
            target_timeslot_end: target_timeslot_end.into(),
            origin_timeslot_start: origin_timeslot_start.into(),
            origin_timeslot_end: origin_timeslot_end.into(),
            flight_priority: flight_plan.flight_priority,
        })
    }
}

/// Request Body Information for Flight Query
#[derive(Debug, Clone, IntoParams, ToSchema, Deserialize, Serialize)]
pub struct QueryItineraryRequest {
    /// The String ID of the vertiport to leave from
    pub origin_vertiport_id: String,

    /// The String ID of the destination vertiport
    pub target_vertiport_id: String,

    /// The window of departure
    pub time_depart_window: Option<TimeWindow>,

    /// The window of arrival
    pub time_arrive_window: Option<TimeWindow>,

    /// The estimated weight of cargo
    pub cargo_weight_g: u32,

    /// The User ID
    /// TODO(R5): Get his from ACL module
    pub user_id: String
}

/// Request Body Information for Flight Query
#[derive(Debug, Clone, IntoParams, ToSchema, Deserialize, Serialize)]
pub struct DraftItinerary {
    /// The draft ID
    pub id: String,

    /// The itinerary information
    pub itinerary: Itinerary,
}

/// Time window (min and max)
#[derive(Debug, Copy, Clone, IntoParams, ToSchema, Deserialize, Serialize)]
pub struct TimeWindow {
    /// The start of the pad window
    pub timestamp_min: DateTime<Utc>,

    /// The end of the pad window
    pub timestamp_max: DateTime<Utc>,
}

/// Request body information to cancel an itinerary
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ItineraryCancelRequest {
    /// Itinerary UUID to Cancel
    pub id: String,

    /// User ID
    /// TODO(R5): Get this from ACL module
    pub user_id: String,
}

/// Request Body Information for Region Query
#[derive(Debug, Copy, Clone, Deserialize, Serialize, ToSchema)]
pub struct QueryVertiportsRequest {
    /// Latitude of Client
    pub latitude: f32,

    /// Longitude of Client
    pub longitude: f32,
}

/// Supported Currencies
#[derive(Debug, Serialize, Deserialize, Copy, Clone, ToSchema)]
pub enum CurrencyUnit {
    /// One U.S. Dollar
    Usd,

    /// One E.U. Euro
    Euro,
}

///
/// TODO(R5): Import this from svc-pricing
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]

pub struct InvoiceItem {
    /// The item name
    pub item: String,

    /// The item cost
    pub cost: f32,
}

/// Itinerary
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct Itinerary {
    /// Each leg of the itinerary
    pub flight_plans: Vec<FlightPlan>,

    /// The currency type, e.g. Usd, EUR
    pub currency_unit: CurrencyUnit,

    /// The cost of the trip for the customer
    /// List of "item": "cost"
    pub invoice: Vec<InvoiceItem>,

    /// Cargo Weight
    pub cargo_weight_g: u32,

    /// User ID
    pub user_id: String,

    /// acquisition vertiport ID
    pub acquisition_vertiport_id: String,

    /// delivery vertiport ID
    pub delivery_vertiport_id: String,
}

impl Default for Itinerary {
    fn default() -> Self {
        Itinerary {
            flight_plans: Vec::new(),
            currency_unit: CurrencyUnit::Euro,
            invoice: Vec::new(),
            user_id: String::new(),
            acquisition_vertiport_id: String::new(),
            delivery_vertiport_id: String::new(),
            cargo_weight_g: 0,
        }
    }
}

/// Customer Itinerary Confirm Option
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ItineraryCreateRequest {
    /// The svc-cargo itinerary ID to create
    pub id: String,

    /// User ID
    /// TODO(R5): Get this from ACL module
    pub user_id: String,
}

/// Vertiport Information
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct Vertiport {
    /// The unique ID of the vertiport
    pub id: String,

    /// The human-readable label of the vertiport
    #[schema(example = "Mercy Hospital (Public)")]
    pub label: String,

    /// The latitude (float value) of the vertiport (centroid)
    pub latitude: f32,

    /// The longitude (float value) of the vertiport (centroid)
    pub longitude: f32,
}

/// Successful Payment Record
#[derive(Debug, Serialize, Deserialize, ToSchema, Copy, Clone)]
pub struct PaymentInfo {
    /// The payment total
    pub total: f32,

    /// The currency type, e.g. Usd, EUR
    pub currency_unit: CurrencyUnit,

    /// Date
    pub timestamp: DateTime<Utc>

    // /// Method
    // pub method: PaymentType
}

/// Request Body Information for Occupations at a Given Vertiport
#[derive(Debug, Clone, IntoParams, ToSchema, Deserialize, Serialize)]
pub struct QueryScheduleRequest {
    /// The String ID of the vertiport
    pub vertiport_id: String,

    /// The window to search for occupations
    pub arrival_window: Option<TimeWindow>,

    /// The maximum number of occupations to return (max: [`MAX_LANDINGS_TO_RETURN`]])
    pub limit: u32,
}

/// Occupations Response
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct QueryScheduleResponse {
    /// list of landing information
    pub occupations: Vec<Occupation>,
}


/// Information about a parcel
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct CargoInfo {
    /// the unique UUID of the parcel or passenger
    pub cargo_id: String,

    // /// the nickname of the parcel or passenger
    // pub cargo_nickname: Option<String>

    // TODO(R5): weight, etc.
}

/// Vertipad Occupation
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct Occupation {
    /// The unique UUID of the flight plan
    pub flight_plan_id: String,

    /// Unique vertipad UUID
    pub vertipad_id: String,

    /// The human-readable label of the vertipad
    pub vertipad_display_name: Option<String>,

    /// The time window of occupation
    pub time_window: TimeWindow,

    /// The callsign of the aircraft
    pub aircraft_id: String,

    /// The nickname of the aircraft
    pub aircraft_nickname: Option<String>,

    /// Parcels being picked up during this occupation
    pub cargo_acquire: Vec<CargoInfo>,

    /// Parcels being delivered during this occupation
    pub cargo_deliver: Vec<CargoInfo>,
}

/// Request Body Information for Tracking a Parcel
#[derive(Debug, Clone, IntoParams, ToSchema, Deserialize, Serialize)]
pub struct QueryParcelRequest {
    /// The UUID of the parcel
    pub parcel_id: String,
}

/// CargoScan information
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct CargoScan {
    /// The unique ID (UUID) of the scanner device
    pub scanner_id: String,

    /// The unique ID (UUID) of the parcel or passenger
    pub cargo_id: String,

    /// The latitude (float value) of the scan location
    pub latitude: f64,

    /// The longitude (float value) of the scan location
    pub longitude: f64,

    /// The timestamp of the scan
    pub timestamp: DateTime<Utc>
}

/// Tracking Information Response
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct QueryParcelResponse {
    /// list of scans
    pub scans: Vec<CargoScan>,
}
