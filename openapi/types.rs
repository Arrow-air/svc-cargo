/// Types used for REST communication with the svc-cargo server

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use std::time::SystemTime;

/// Request Body Information for Flight Query
#[allow(dead_code)]
#[derive(Debug, Clone, IntoParams, ToSchema)]
#[derive(Deserialize, Serialize)]
pub struct FlightQuery {
    /// The String ID of the vertiport to leave from
    pub vertiport_depart_id: String,

    /// The String ID of the destination vertiport
    pub vertiport_arrive_id: String,

    /// The start of the pad departure window
    pub timestamp_depart_min: Option<SystemTime>,

    /// The end of the pad departure window
    pub timestamp_depart_max: Option<SystemTime>,

    /// The start of the pad arrival window
    pub timestamp_arrive_min: Option<SystemTime>,

    /// The end of the pad arrival window
    pub timestamp_arrive_max: Option<SystemTime>,

    /// The estimated weight of cargo
    pub cargo_weight_kg: f32
}

/// Request Body Information to Cancel a Flight
#[derive(Debug, Clone)]
#[derive(Deserialize, Serialize)]
#[derive(ToSchema)]
pub struct FlightCancel {

    /// Flight Plan ID to Cancel
    pub fp_id: String,
    // TODO optional reason
}

/// Request Body Information for Region Query
#[derive(Debug, Copy, Clone)]
#[derive(Deserialize, Serialize)]
#[derive(ToSchema)]
pub struct VertiportsQuery {
    /// Latitude of Client
    pub latitude: f32,

    /// Longitude of Client
    pub longitude: f32,
}

/// Flight Plan Option
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct FlightOption {
    /// Flight Plan ID
    pub fp_id: String,

    /// Departure Vertiport ID
    pub vertiport_depart_id: String,

    /// Arrival Vertiport ID
    pub vertiport_arrive_id: String,

    /// Estimated departure timestamp
    pub timestamp_depart: SystemTime,

    /// Estimated arrival timestamp
    pub timestamp_arrive: SystemTime,

    /// The estimated trip distance in meters
    pub distance_m: f32,
    
    /// The currency type, e.g. USD, EUR
    pub currency_type: Option<String>,

    /// The cost of the trip for the customer
    pub base_pricing: Option<f32>
}


/// Customer Flight Confirm Option
#[derive(Debug, Clone)]
#[derive(Serialize, Deserialize)]
#[derive(ToSchema)]
pub struct FlightConfirm {

    /// Flight Plan ID
    pub fp_id: String
}
/// Vertiport Information
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct Vertiport {
    /// The unique ID of the vertiport
    pub id: String,

    /// The human-readable label of the vertiport
    #[schema(example = "Mercy Hospital (Public)")]
    pub label: String,

    /// The latitude (float value) of the vertiport
    pub latitude: f64,

    /// The longitude (float value) of the vertiport
    pub longitude: f64,
}

// #[derive(Serialize, Deserialize, ToSchema, Clone)]
// pub struct VertiportInstructions {
//     id: String,
//     #[schema(example = "Check-In at Arrow Office, Floor 10 of West Tower")]
//     description_depart: String,
//     #[schema(example = "To Hamilton Street: Elevator to floor 2, take the pedestrian bridge to the street.")]
//     description_arrive: HashMap<String, String>
// }

/// Confirm Flight Operation Status
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub enum ConfirmStatus {
    /// Successful confirmation of flight
    #[schema(example = "Flight successfully confirmed.")]
    Success(String),

    /// FlightOption already exists conflict.
    #[schema(example = "Could not confirm flight.")]
    Conflict(String),
    /// FlightOption not found by id.
    #[schema(example = "Provided flight plan ID doesn't match an existing flight.")]
    NotFound(String),
    /// Unauthorized Attempt to Confirm Flight
    #[schema(example = "Unauthorized confirmation by someone other than the customer.")]
    Unauthorized(String),

    /// Unavailable Service
    Unavailable,
}
