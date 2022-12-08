pub use svc_pricing_client::pricing_grpc::{
    pricing_client::PricingClient, pricing_request::ServiceType, PricingRequest,
};

pub use svc_scheduler_client_grpc::grpc::{
    scheduler_rpc_client::SchedulerRpcClient, Id, QueryFlightPlan, QueryFlightRequest,
};

pub use svc_storage_client_grpc::client::{
    vertiport_rpc_client::VertiportRpcClient, SearchFilter, VertiportData,
};

use crate::{grpc_debug, grpc_error, grpc_info};
use futures::lock::Mutex;
use log::{debug, error};
use std::sync::Arc;
pub use tonic::transport::Channel;

#[derive(Clone, Debug)]
pub struct GrpcClients {
    pub scheduler: GrpcClient<SchedulerRpcClient<Channel>>,
    pub storage: GrpcClient<VertiportRpcClient<Channel>>,
    pub pricing: GrpcClient<PricingClient<Channel>>,
}

#[derive(Debug, Clone)]
pub struct GrpcClient<T> {
    inner: Arc<Mutex<Option<T>>>,
    address: String,
}

/// Returns a string in http://<host>:<port> format from provided
/// environment variables
fn get_grpc_endpoint(env_host: &str, env_port: &str) -> String {
    debug!("(get_grpc_endpoint) entry");
    let port = match std::env::var(env_port) {
        Ok(s) => s,
        Err(_) => {
            error!("(env) {} undefined.", env_port);
            "".to_string()
        }
    };
    let host = match std::env::var(env_host) {
        Ok(s) => s,
        Err(_) => {
            error!("(env) {} undefined.", env_host);
            "".to_string()
        }
    };

    format!("http://{host}:{port}")
}

impl<T> GrpcClient<T> {
    pub async fn invalidate(&mut self) {
        let arc = Arc::clone(&self.inner);
        let mut client = arc.lock().await;
        *client = None;
    }

    pub fn new(env_host: &str, env_port: &str) -> Self {
        let opt: Option<T> = None;
        GrpcClient {
            inner: Arc::new(Mutex::new(opt)),
            address: get_grpc_endpoint(env_host, env_port),
        }
    }
}

// TODO Figure out how to collapse these three implementations for each client into
//   one generic impl. VertiportRpcClient does not simply impl a trait,
//   it wraps the tonic::client::Grpc<T> type so it's a bit tricky
impl GrpcClient<VertiportRpcClient<Channel>> {
    pub async fn get_client(&mut self) -> Option<VertiportRpcClient<Channel>> {
        grpc_debug!("(get_client) storage::vertiport entry");

        let arc = Arc::clone(&self.inner);
        let mut client = arc.lock().await;

        if client.is_none() {
            grpc_info!(
                "(grpc) connecting to svc-storage vertiport server at {}",
                self.address.clone()
            );
            let client_option = match VertiportRpcClient::connect(self.address.clone()).await {
                Ok(s) => Some(s),
                Err(e) => {
                    grpc_error!(
                        "(grpc) couldn't connect to svc-storage vertiport server at {}; {}",
                        self.address,
                        e
                    );
                    None
                }
            };

            *client = client_option;
        }

        client.clone()
    }
}

impl GrpcClient<PricingClient<Channel>> {
    pub async fn get_client(&mut self) -> Option<PricingClient<Channel>> {
        grpc_debug!("(get_client) pricing entry");
        let arc = Arc::clone(&self.inner);
        let mut client = arc.lock().await;
        if client.is_none() {
            grpc_info!(
                "(grpc) connecting to svc-pricing server at {}",
                self.address.clone()
            );
            let client_option = match PricingClient::connect(self.address.clone()).await {
                Ok(s) => Some(s),
                Err(e) => {
                    grpc_error!(
                        "(grpc) couldn't connect to svc-pricing server at {}; {}",
                        self.address,
                        e
                    );
                    None
                }
            };

            *client = client_option;
        }

        client.clone()
    }
}

impl GrpcClient<SchedulerRpcClient<Channel>> {
    pub async fn get_client(&mut self) -> Option<SchedulerRpcClient<Channel>> {
        grpc_debug!("(get_client) scheduler entry");
        let arc = Arc::clone(&self.inner);
        let mut client = arc.lock().await;
        if client.is_none() {
            grpc_info!(
                "(grpc) connecting to svc-scheduler server at {}",
                self.address.clone()
            );
            let client_option =
                match SchedulerRpcClient::<Channel>::connect(self.address.clone()).await {
                    Ok(s) => Some(s),
                    Err(e) => {
                        grpc_error!(
                            "(grpc) couldn't connect to svc-scheduler server at {}; {}",
                            self.address,
                            e
                        );
                        None
                    }
                };

            *client = client_option;
        }

        client.clone()
    }
}

impl GrpcClients {
    pub fn default() -> Self {
        GrpcClients {
            scheduler: GrpcClient::<SchedulerRpcClient<Channel>>::new(
                "SCHEDULER_HOST_GRPC",
                "SCHEDULER_PORT_GRPC",
            ),
            storage: GrpcClient::<VertiportRpcClient<Channel>>::new(
                "STORAGE_HOST_GRPC",
                "STORAGE_PORT_GRPC",
            ),
            pricing: GrpcClient::<PricingClient<Channel>>::new(
                "PRICING_HOST_GRPC",
                "PRICING_PORT_GRPC",
            ),
        }
    }
}
