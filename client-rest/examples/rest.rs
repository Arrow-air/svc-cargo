//! Example communication with this service

use chrono::{Duration, Utc};
use hyper::{Body, Client, Method, Request, Response};
use hyper::{Error, StatusCode};
use lib_common::grpc::get_endpoint_from_env;
use svc_cargo_client_rest::types::*;
use uuid::Uuid;

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

    let rate_limit = std::env::var("REQUEST_LIMIT_PER_SECOND")
        .unwrap_or_else(|_| "2".to_string())
        .parse::<u64>()
        .unwrap_or(2);

    let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_REST");
    let url = format!("http://{host}:{port}");
    let mut ok = true;
    let client = Client::builder()
        .pool_idle_timeout(std::time::Duration::from_secs(10))
        .build_http();

    // Too many requests
    {
        let data = QueryVertiportsRequest {
            latitude: 52.37488619450752,
            longitude: 4.916048576268328,
        };

        let Ok(data) = serde_json::to_string(&data) else {
            panic!("Failed to serialize data");
        };

        let uri = format!("{}/cargo/vertiports", url);

        for x in 0..=rate_limit {
            let Ok(request) = Request::builder()
                .method(Method::POST)
                .uri(uri.clone())
                .header("content-type", "application/json")
                .body(Body::from(data.clone()))
            else {
                panic!("Failed to build request");
            };
            if x >= rate_limit {
                let resp: Result<Response<Body>, Error> = client.request(request).await;
                let (success, result_str) = evaluate(resp, StatusCode::TOO_MANY_REQUESTS);
                ok &= success;
                println!("{}: {}", uri, result_str);
            } else {
                let _ = client.request(request).await;
            }
        }
    }

    std::thread::sleep(std::time::Duration::from_secs(1));

    // POST /cargo/vertiports
    {
        let data = QueryVertiportsRequest {
            latitude: 52.37488619450752,
            longitude: 4.916048576268328,
        };

        let Ok(data_str) = serde_json::to_string(&data) else {
            panic!("Failed to serialize data");
        };

        let uri = format!("{}/cargo/vertiports", url);
        let Ok(req) = Request::builder()
            .method(Method::POST)
            .uri(uri.clone())
            .header("content-type", "application/json")
            .body(Body::from(data_str))
        else {
            panic!("Failed to build request");
        };

        let resp = client.request(req).await;
        let (success, result_str) = evaluate(resp, StatusCode::OK);
        ok &= success;

        println!("{}: {}", uri, result_str);
    }

    // Avoid too many requests
    std::thread::sleep(std::time::Duration::from_secs(1));

    // POST /cargo/request
    {
        let depart_timestamp_min = Utc::now() + Duration::seconds(60);
        let data = QueryItineraryRequest {
            // Arbitrary UUIDs
            origin_vertiport_id: "cabcdd14-03ab-4ac0-b58c-dd4175bc587e".to_string(),
            target_vertiport_id: "59e51ad1-d57d-4d2c-bc2d-e2387367d17f".to_string(),
            time_depart_window: Some(TimeWindow {
                timestamp_min: depart_timestamp_min,
                timestamp_max: depart_timestamp_min + Duration::seconds(360),
            }),
            time_arrive_window: None,
            cargo_weight_g: 200,
            user_id: Uuid::new_v4().to_string(),
        };

        let Ok(data_str) = serde_json::to_string(&data) else {
            panic!("Failed to serialize data");
        };

        let uri = format!("{}/cargo/request", url);
        let Ok(req) = Request::builder()
            .method(Method::POST)
            .uri(uri.clone())
            .header("content-type", "application/json")
            .body(Body::from(data_str))
        else {
            panic!("Failed to build request");
        };

        let resp = client.request(req).await;
        let (success, result_str) = evaluate(resp, StatusCode::ACCEPTED);
        ok &= success;

        println!("{}: {}", uri, result_str);
    }

    // Avoid too many requests
    std::thread::sleep(std::time::Duration::from_secs(1));

    // PUT /cargo/create
    {
        let data = ItineraryCreateRequest {
            // Arbitrary UUIDs
            id: Uuid::new_v4().to_string(),
            user_id: Uuid::new_v4().to_string(),
        };

        let Ok(data_str) = serde_json::to_string(&data) else {
            panic!("Failed to serialize data");
        };

        let uri = format!("{}/cargo/create", url);
        let Ok(req) = Request::builder()
            .method(Method::PUT)
            .uri(uri.clone())
            .header("content-type", "application/json")
            .body(Body::from(data_str))
        else {
            panic!("Failed to build request");
        };

        let resp = client.request(req).await;
        let (success, result_str) = evaluate(resp, StatusCode::OK);
        ok &= success;

        println!("{}: {}", uri, result_str);
    }

    // Avoid too many requests
    std::thread::sleep(std::time::Duration::from_secs(1));

    // DELETE /cargo/cancel
    {
        let data = ItineraryCancelRequest {
            // arbitrary UUIDs
            id: "cabcdd14-03ab-4ac0-b58c-dd4175bc587e".to_string(),
            user_id: Uuid::new_v4().to_string(),
        };

        let Ok(data_str) = serde_json::to_string(&data) else {
            panic!("Failed to serialize data");
        };

        let uri = format!("{}/cargo/cancel", url);
        let Ok(req) = Request::builder()
            .method(Method::DELETE)
            .uri(uri.clone())
            .header("content-type", "application/json")
            .body(Body::from(data_str))
        else {
            panic!("Failed to build request");
        };

        let resp = client.request(req).await;
        let (success, result_str) = evaluate(resp, StatusCode::OK);
        ok &= success;

        println!("{}: {}", uri, result_str);
    }

    // Avoid too many requests
    std::thread::sleep(std::time::Duration::from_secs(1));

    // PUT /cargo/scan
    {
        let data = CargoScan {
            scanner_id: Uuid::new_v4().to_string(),
            cargo_id: Uuid::new_v4().to_string(),
            latitude: 52.37474373455002,
            longitude: 4.9167298573581295,
            timestamp: Utc::now(),
        };

        let Ok(data) = serde_json::to_string(&data) else {
            panic!("Failed to serialize data");
        };

        let uri = format!("{}/cargo/scan", url);
        let Ok(request) = Request::builder()
            .method(Method::PUT)
            .uri(uri.clone())
            .header("content-type", "application/json")
            .body(Body::from(data))
        else {
            panic!("Failed to build request");
        };

        let resp = client.request(request).await;
        let (success, result_str) = evaluate(resp, StatusCode::ACCEPTED);
        ok &= success;

        println!("{}: {}", uri, result_str);
    }

    // Avoid too many requests
    std::thread::sleep(std::time::Duration::from_secs(1));

    // GET /cargo/flights
    {
        let data = QueryScheduleRequest {
            vertiport_id: Uuid::new_v4().to_string(),
            arrival_window: Some(TimeWindow {
                timestamp_min: Utc::now(),
                timestamp_max: Utc::now() + Duration::seconds(3600),
            }),
            limit: 20,
        };

        let Ok(data) = serde_json::to_string(&data) else {
            panic!("Failed to serialize data");
        };

        let uri = format!("{}/cargo/occupations", url);
        let Ok(request) = Request::builder()
            .method(Method::GET)
            .uri(uri.clone())
            .header("content-type", "application/json")
            .body(Body::from(data))
        else {
            panic!("Failed to build request");
        };

        let resp = client.request(request).await;
        let (success, result_str) = evaluate(resp, StatusCode::ACCEPTED);
        ok &= success;

        println!("{}: {}", uri, result_str);
    }

    // Avoid too many requests
    std::thread::sleep(std::time::Duration::from_secs(1));

    if ok {
        println!("\u{1F9c1} All endpoints responded!");
    } else {
        eprintln!("\u{2620} Errors");
    }

    Ok(())
}
