//! gRPC
//! provides Redis implementations for caching layer

#[macro_use]
pub mod macros;
pub mod pool;

use crate::cache::pool::ItineraryPool;
use crate::rest::api::rest_types::Itinerary;
use deadpool_redis::redis::{
    ErrorKind, FromRedisValue, RedisError, RedisWrite, ToRedisArgs, Value,
};
use std::fmt::{Display, Formatter, Result as FmtResult};

impl FromRedisValue for Itinerary {
    fn from_redis_value(v: &Value) -> Result<Self, RedisError> {
        let Value::Data(data) = v else {
            return Err(RedisError::from((
                ErrorKind::TypeError,
                "Unexpected Redis value",
            )));
        };

        let Ok(itinerary): Result<Itinerary, serde_json::Error> = serde_json::from_slice(data)
        else {
            return Err(RedisError::from((ErrorKind::TypeError, "Invalid JSON")));
        };

        Ok(itinerary)
    }
}

impl ToRedisArgs for Itinerary {
    fn write_redis_args<W: ?Sized>(&self, out: &mut W)
    where
        W: RedisWrite,
    {
        let Ok(data) = serde_json::to_string(self) else {
            cache_warn!("(ToRedisArgs) error serializing task");
            return;
        };

        out.write_arg(data.as_bytes());
    }
}

/// Errors that can occur when processing a task
#[derive(Copy, Clone, Debug)]
pub enum ItineraryError {
    /// Task id was not found
    NotFound,

    /// Internal error with updating task
    Internal,

    /// Task was already processed
    AlreadyProcessed,

    /// Invalid data provided
    InvalidData,

    /// Schedule Conflict
    ScheduleConflict,
}

impl Display for ItineraryError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            ItineraryError::NotFound => write!(f, "Itinerary not found."),
            ItineraryError::Internal => write!(f, "Internal error."),
            ItineraryError::AlreadyProcessed => write!(f, "Itinerary already processed."),
            ItineraryError::InvalidData => write!(f, "Invalid data."),
            ItineraryError::ScheduleConflict => write!(f, "Schedule conflict."),
        }
    }
}

/// Gets a svc-cargo itinerary
pub async fn get_itinerary(itinerary_id: String) -> Result<Itinerary, ItineraryError> {
    let Some(mut pool) = crate::cache::pool::get_pool().await else {
        cache_error!("(get_itinerary) Couldn't get the redis pool.");
        return Err(ItineraryError::Internal);
    };

    match pool.get_itinerary(itinerary_id).await {
        Ok(itinerary) => Ok(itinerary),
        Err(e) => {
            cache_warn!("(get_itinerary) error getting itinerary: {}", e);
            Err(ItineraryError::NotFound)
        }
    }
}
