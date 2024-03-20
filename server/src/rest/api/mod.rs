/// Types Used in REST Messages
pub mod rest_types {
    include!("../../../../openapi/types.rs");
}
pub mod cancel;
pub mod create;
pub mod health;
pub mod query;
pub mod request;
pub mod scan;
pub mod utils;
