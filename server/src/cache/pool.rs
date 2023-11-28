//! Redis connection pool implementation
use super::Itinerary;
use deadpool_redis::{
    redis::{AsyncCommands, FromRedisValue, Value},
    Pool, Runtime,
};
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};
use tonic::async_trait;
use uuid::Uuid;

/// A global static Redis pool.
static REDIS_POOL: OnceCell<Arc<Mutex<CargoPool>>> = OnceCell::const_new();

/// How long to keep a task in memory after it's been processed
const ITINERARY_KEEPALIVE_DURATION_SECONDS: usize = 120;

/// Number of UUID retries on collision
const UUID_COLLISION_RETRIES: u8 = 3;

/// Returns a Redis Pool.
/// Uses host and port configurations using a Config object generated from
/// environment variables.
/// Initializes the pool if it hasn't been initialized yet.
pub async fn get_pool() -> Option<CargoPool> {
    if !REDIS_POOL.initialized() {
        let config = crate::Config::try_from_env().unwrap_or_default();
        let Some(pool) = CargoPool::new(config.clone()) else {
            cache_error!("(get_pool) could not create Redis pool.");
            panic!("(get_pool) could not create Redis pool.");
        };

        let value = Arc::new(Mutex::new(pool));
        if let Err(e) = REDIS_POOL.set(value) {
            cache_error!("(get_pool) could not set Redis pool: {e}");
            panic!("(get_pool) could not set Redis pool: {e}");
        };
    }

    let Some(arc) = REDIS_POOL.get() else {
        cache_error!("(get_pool) could not get Redis pool.");
        return None;
    };

    let pool = arc.lock().await;
    Some((*pool).clone())
}

/// Represents errors that can occur during cache operations.
#[derive(Debug, Clone, Copy)]
pub enum CacheError {
    /// Could not build configuration for cache.
    CouldNotConfigure,

    /// Could not connect to the Redis pool.
    CouldNotConnect,

    /// Key was not found
    NotFound,

    /// The operation on the Redis cache failed.
    OperationFailed,
}

impl Display for CacheError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            CacheError::CouldNotConfigure => write!(f, "Could not configure cache."),
            CacheError::CouldNotConnect => write!(f, "Could not connect to cache."),
            CacheError::OperationFailed => write!(f, "Cache operation failed."),
            CacheError::NotFound => write!(f, "Key was not found."),
        }
    }
}

/// Represents a pool of connections to a Redis server.
///
/// The [`CargoPool`] struct provides a managed pool of connections to a Redis server.
/// It allows clients to acquire and release connections from the pool and handles
/// connection management, such as connection pooling and reusing connections.
#[derive(Clone)]
pub struct CargoPool {
    /// The underlying pool of Redis connections.
    pool: Pool,
}

impl Debug for CargoPool {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CargoPool").finish()
    }
}

impl CargoPool {
    /// Create a new CargoPool
    pub fn new(config: crate::config::Config) -> Option<CargoPool> {
        // the .env file must have REDIS__URL="redis://\<host\>:\<port\>"
        let cfg: deadpool_redis::Config = config.redis;
        let Some(details) = cfg.url.clone() else {
            cache_error!("(CargoPool new) no connection address found.");
            return None;
        };

        cache_info!(
            "(CargoPool new) creating pool with key folder 'cargo' at {:?}...",
            details
        );

        match cfg.create_pool(Some(Runtime::Tokio1)) {
            Ok(pool) => {
                cache_info!("(CargoPool new) pool created.");
                Some(CargoPool { pool })
            }
            Err(e) => {
                cache_error!("(CargoPool new) could not create pool: {}", e);
                None
            }
        }
    }
}

impl ItineraryPool for CargoPool {
    fn pool(&self) -> &Pool {
        &self.pool
    }
}

/// Trait for interacting with a cargo task pool
#[async_trait]
pub trait ItineraryPool {
    /// Returns a reference to the underlying pool.
    fn pool(&self) -> &Pool;

    /// Creates a new task and returns the task_id for it
    async fn store_itinerary(&mut self, draft_itinerary: &Itinerary) -> Result<Uuid, CacheError>
    where
        Self: Send + Sync + 'async_trait,
    {
        let mut connection = match self.pool().get().await {
            Ok(c) => c,
            Err(e) => {
                cache_error!(
                    "(ItineraryPool update_task) could not get connection from pool: {}",
                    e
                );

                return Err(CacheError::OperationFailed);
            }
        };

        let mut attempt = 0;
        let mut itinerary_id = Uuid::new_v4();
        while attempt < UUID_COLLISION_RETRIES {
            let key = format!("cargo:draft:{itinerary_id}");
            match connection.hset_nx(&key, "data", draft_itinerary).await {
                Ok(Value::Int(1)) => {
                    cache_debug!("(ItineraryPool new_task) successfully added itinerary with UUID {itinerary_id}.");
                }
                Ok(Value::Int(0)) => {
                    cache_debug!("(ItineraryPool new_task) key collision (attempt {attempt}): {itinerary_id}");
                    attempt += 1;
                    itinerary_id = Uuid::new_v4();
                    continue;
                }
                Ok(value) => {
                    cache_error!(
                        "(ItineraryPool new_task) unexpected redis response to hsetnx command: {:?}",
                        value
                    );
                    return Err(CacheError::OperationFailed);
                }
                Err(e) => {
                    cache_error!("(ItineraryPool new_task) unexpected redis response to hsetnx command: {:?}", e);
                    return Err(CacheError::OperationFailed);
                }
            };

            // Add expiration to itinerary
            match connection
                .expire(key.clone(), ITINERARY_KEEPALIVE_DURATION_SECONDS)
                .await
            {
                Ok(Value::Int(1)) => break,
                Ok(value) => {
                    cache_error!(
                        "(ItineraryPool new_task) unexpected redis response to expire command: {:?}",
                        value
                    );

                    return Err(CacheError::OperationFailed);
                }
                Err(e) => {
                    cache_error!("(ItineraryPool new_task) could not set itinerary #{itinerary_id} expiry: {e}",);

                    return Err(CacheError::OperationFailed);
                }
            }
        }

        if attempt == UUID_COLLISION_RETRIES {
            cache_error!("(ItineraryPool new_task) failed to generate UUID for draft itinerary, multiple key collisions.");
            return Err(CacheError::OperationFailed);
        }

        cache_info!("(ItineraryPool new_task) created new draft itinerary #{itinerary_id}.",);
        cache_debug!(
            "(ItineraryPool new_task) new itinerary #{itinerary_id} data: {:?}",
            draft_itinerary
        );

        Ok(itinerary_id)
    }

    /// Gets task information
    async fn get_itinerary(&mut self, itinerary_id: String) -> Result<Itinerary, CacheError>
    where
        Self: Send + Sync + 'async_trait,
    {
        let key = format!("cargo:draft:{itinerary_id}");
        let mut connection = match self.pool().get().await {
            Ok(c) => c,
            Err(e) => {
                cache_error!(
                    "(ItineraryPool update_task) could not get connection from pool: {}",
                    e
                );
                return Err(CacheError::OperationFailed);
            }
        };

        let result = connection.hget(key, "data".to_string()).await;

        // Have to separate the Value type check from the main match case
        match result {
            Ok(Value::Nil) => {
                cache_error!("(ItineraryPool get_task_data) key expired or does not exist.");
                return Err(CacheError::NotFound);
            }
            Ok(value) => {
                let Ok(itinerary) = Itinerary::from_redis_value(&value) else {
                    cache_error!(
                        "(ItineraryPool get_task_data) could not deserialize itinerary: {:?}",
                        value
                    );
                    return Err(CacheError::OperationFailed);
                };

                Ok(itinerary)
            }
            Err(e) => {
                cache_error!(
                    "(ItineraryPool get_task_data) error getting itinerary: {:?}",
                    e
                );
                return Err(CacheError::OperationFailed);
            }
        }
    }
}
