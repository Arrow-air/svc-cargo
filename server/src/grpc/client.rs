//! gRPC client helpers implementation
use tokio::sync::OnceCell;

use lib_common::grpc::Client;
use svc_pricing_client_grpc::prelude::PricingClient;
use svc_scheduler_client_grpc::prelude::SchedulerClient;
use svc_storage_client_grpc::prelude::Clients;

pub(crate) static CLIENTS: OnceCell<GrpcClients> = OnceCell::const_new();

/// Returns CLIENTS, a GrpcClients object with default values.
/// Uses host and port configurations using a Config object generated from
/// environment variables.
/// Initializes CLIENTS if it hasn't been initialized yet.
pub async fn get_clients() -> &'static GrpcClients {
    CLIENTS
        .get_or_init(|| async move {
            let config = crate::Config::try_from_env().unwrap_or_default();
            GrpcClients::default(config)
        })
        .await
}

/// Struct to hold all gRPC client connections
#[derive(Clone, Debug)]
pub struct GrpcClients {
    /// All clients enabled from the svc_storage_grpc_client module
    pub storage: Clients,
    /// A GrpcClient provided by the svc_scheduler_grpc_client module
    pub scheduler: SchedulerClient,
    /// A GrpcClient provided by the svc_pricing_grpc_client module
    pub pricing: PricingClient,
}

impl GrpcClients {
    /// Create new GrpcClients with defaults
    pub fn default(config: crate::Config) -> Self {
        let storage_clients = Clients::new(config.storage_host_grpc, config.storage_port_grpc);

        GrpcClients {
            storage: storage_clients,
            scheduler: SchedulerClient::new_client(
                &config.scheduler_host_grpc,
                config.scheduler_port_grpc,
                "scheduler",
            ),
            pricing: PricingClient::new_client(
                &config.pricing_host_grpc,
                config.pricing_port_grpc,
                "pricing",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use lib_common::grpc::Client as CommonClient;

    use super::*;

    #[tokio::test]
    async fn test_grpc_clients_default() {
        crate::get_log_handle().await;
        ut_info!("(test_grpc_clients_default) Start.");

        let config = crate::Config::default();
        let clients = GrpcClients::default(config);

        let flight_plan = &clients.storage.flight_plan;
        ut_debug!("(test_grpc_clients_default) flight_plan: {:?}", flight_plan);
        assert_eq!(flight_plan.get_name(), "flight_plan");

        let vertipad = &clients.storage.vertipad;
        ut_debug!("(test_grpc_clients_default) vertipad: {:?}", vertipad);
        assert_eq!(vertipad.get_name(), "vertipad");

        let vertiport = &clients.storage.vertiport;
        ut_debug!("(test_grpc_clients_default) vertiport: {:?}", vertiport);
        assert_eq!(vertiport.get_name(), "vertiport");

        let parcel = &clients.storage.parcel;
        ut_debug!("(test_grpc_clients_default) parcel: {:?}", parcel);
        assert_eq!(parcel.get_name(), "parcel");

        let parcel_scan = &clients.storage.parcel_scan;
        ut_debug!("(test_grpc_clients_default) parcel_scan: {:?}", parcel_scan);
        assert_eq!(parcel_scan.get_name(), "parcel_scan");

        let vehicle = &clients.storage.vehicle;
        ut_debug!("(test_grpc_clients_default) vehicle: {:?}", vehicle);
        assert_eq!(vehicle.get_name(), "vehicle");

        let pricing = &clients.pricing;
        ut_debug!("(test_grpc_clients_default) pricing: {:?}", pricing);
        assert_eq!(pricing.get_name(), "pricing");

        let scheduler = &clients.scheduler;
        ut_debug!("(test_grpc_clients_default) scheduler: {:?}", scheduler);
        assert_eq!(scheduler.get_name(), "scheduler");

        ut_info!("(test_grpc_clients_default) Success.");
    }
}
