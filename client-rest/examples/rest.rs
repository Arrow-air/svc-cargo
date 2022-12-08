//! Example communication with this service

use hyper::{Body, Client, Method, Request, Response};
use hyper::{Error, StatusCode};
use std::time::{Duration, SystemTime};
use svc_cargo_client_rest::types::*;

fn evaluate(resp: Result<Response<Body>, Error>, expected_code: StatusCode) -> (bool, String) {
    let mut ok = true;
    let result_str: String = match resp {
        Ok(r) => {
            let tmp = r.status() == expected_code;
            ok &= tmp;
            println!("{:?}", r.body());

            r.status().to_string()
        }
        Err(e) => {
            ok = false;
            e.to_string()
        }
    };

    (ok, result_str)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("NOTE: Ensure the server is running, or this example will fail.");

    let rest_port = std::env::var("HOST_PORT_REST").unwrap_or_else(|_| "8000".to_string());

    // let host_port = env!("HOST_PORT");
    let url = format!("http://0.0.0.0:{rest_port}");
    let mut ok = true;
    let client = Client::builder()
        .pool_idle_timeout(std::time::Duration::from_secs(10))
        .build_http();

    // POST /cargo/vertiports
    {
        let data = VertiportsQuery {
            latitude: 32.7262,
            longitude: 117.1544,
        };
        let data_str = serde_json::to_string(&data).unwrap();
        let uri = format!("{}/cargo/vertiports", url);
        let req = Request::builder()
            .method(Method::POST)
            .uri(uri.clone())
            .header("content-type", "application/json")
            .body(Body::from(data_str))
            .unwrap();

        let resp = client.request(req).await;
        let (success, result_str) = evaluate(resp, StatusCode::OK);
        ok &= success;

        println!("{}: {}", uri, result_str);
    }

    // PUT /cargo/confirm
    {
        let data = FlightConfirm {
            // Arbitrary UUID
            fp_id: "cabcdd14-03ab-4ac0-b58c-dd4175bc587e".to_string(),
        };
        let data_str = serde_json::to_string(&data).unwrap();
        let uri = format!("{}/cargo/confirm", url);
        let req = Request::builder()
            .method(Method::PUT)
            .uri(uri.clone())
            .header("content-type", "application/json")
            .body(Body::from(data_str))
            .unwrap();

        let resp = client.request(req).await;
        let (success, result_str) = evaluate(resp, StatusCode::OK);
        ok &= success;

        println!("{}: {}", uri, result_str);
    }

    // DELETE /cargo/cancel
    {
        let data = FlightCancel {
            // arbitrary UUID
            fp_id: "cabcdd14-03ab-4ac0-b58c-dd4175bc587e".to_string(),
        };
        let data_str = serde_json::to_string(&data).unwrap();
        let uri = format!("{}/cargo/cancel", url);
        let req = Request::builder()
            .method(Method::DELETE)
            .uri(uri.clone())
            .header("content-type", "application/json")
            .body(Body::from(data_str))
            .unwrap();

        let resp = client.request(req).await;
        let (success, result_str) = evaluate(resp, StatusCode::OK);
        ok &= success;

        println!("{}: {}", uri, result_str);
    }

    // POST /cargo/query
    {
        let depart_timestamp_min = SystemTime::now() + Duration::from_secs(60);
        let data = FlightQuery {
            // Arbitrary UUIDs
            vertiport_depart_id: "cabcdd14-03ab-4ac0-b58c-dd4175bc587e".to_string(),
            vertiport_arrive_id: "59e51ad1-d57d-4d2c-bc2d-e2387367d17f".to_string(),
            timestamp_depart_min: Some(depart_timestamp_min),
            timestamp_depart_max: Some(depart_timestamp_min + Duration::from_secs(360)),
            timestamp_arrive_min: None,
            timestamp_arrive_max: None,
            cargo_weight_kg: 1.0,
        };
        let data_str = serde_json::to_string(&data).unwrap();
        let uri = format!("{}/cargo/query", url);
        let req = Request::builder()
            .method(Method::POST)
            .uri(uri.clone())
            .header("content-type", "application/json")
            .body(Body::from(data_str))
            .unwrap();

        let resp = client.request(req).await;
        let (success, result_str) = evaluate(resp, StatusCode::ACCEPTED);
        ok &= success;

        println!("{}: {}", uri, result_str);
    }

    if ok {
        println!("\u{1F9c1} All endpoints responded!");
    } else {
        eprintln!("\u{2620} Errors");
    }

    Ok(())
}
