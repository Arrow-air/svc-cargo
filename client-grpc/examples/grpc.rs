//! gRPC client implementation
use svc_cargo_client_grpc::client::{cargo_rpc_client::CargoRpcClient, QueryIsReady};

/// Provide GRPC endpoint to use
pub fn get_grpc_endpoint() -> String {
    //parse socket address from env variable or take default value
    let address = match std::env::var("SERVER_HOSTNAME") {
        Ok(val) => val,
        Err(_) => "localhost".to_string(), // default value
    };

    let port = match std::env::var("SERVER_PORT_GRPC") {
        Ok(val) => val,
        Err(_) => "50051".to_string(), // default value
    };

    format!("http://{}:{}", address, port)
}

/// Example svc-cargo-client
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("NOTE: Ensure the server is running, or this example will fail.");

    let grpc_endpoint = get_grpc_endpoint();
    let mut client = CargoRpcClient::connect(grpc_endpoint).await?;

    // "Test" Status
    let mut ok = true;

    // IsReady Service
    let request = tonic::Request::new(QueryIsReady {});
    let response = client.is_ready(request).await;
    if response.is_err() {
        ok = false;
        println!("IsReady: FAIL");
    } else {
        println!("IsReady: PASS");
    }

    // Add more here
    if ok {
        println!("\u{1F9c1} All endpoints responded!");
    } else {
        eprintln!("\u{2620} Errors");
    }

    Ok(())
}
