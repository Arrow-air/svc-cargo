# Interface Control Document (ICD) - `svc-cargo`

<center>

<img src="https://github.com/Arrow-air/tf-github/raw/main/src/templates/doc-banner-services.png" style="height:250px" />

</center>

## Overview

This document defines the GRPC and REST interfaces unique to the `svc-cargo` microservice.

Attribute | Description
--- | ---
Status | Development

## Related Documents

Document | Description
--- | ---
| [High-Level Concept of Operations (CONOPS)](https://github.com/Arrow-air/se-services/blob/develop/docs/conops.md) | Overview of Arrow microservices.                             |
| [High-Level Interface Control Document (ICD)](https://github.com/Arrow-air/se-services/blob/develop/docs/icd.md)  | Interfaces and frameworks common to all Arrow microservices. |
[Requirements - `svc-cargo`](https://docs.google.com/spreadsheets/d/1OliSp9BDvMuVvGmSRh1z_Z58QtjlSknLxGVdVZs2l7A/edit#gid=0) | Requirements for this service.
[Software Design Document (SDD)](./sdd.md) | Implementation description of this service.

## Frameworks

See the High-Level Services ICD.

## REST

### Files

Filename | Description
--- | ---
`openapi/types.rs` | Data types used for REST requests and replies.
`cargo-rest/src/lib.rs` | Imports the REST types file to create the `svc-cargo-client-rest` library, usable by other Rust crates.
`cargo-grpc/src/grpc.rs` | Autogenerated GRPC client stubs usable by other Rust crates to easily communicate with the GRPC server of this service.

### Authentication

See the High-Level Services ICD.

### Endpoints

:construction: This API will move to a more readable format.

| Endpoint | Type | Arguments | Description |
| ---- | --- | ---- | ---- |
| `/cargo/query` | POST | vertiport_depart_id<br>vertiport_arrive_id<br>timestamp_depart_min<br>timestamp_depart_max<br>timestamp_arrive_min<br>timestamp_arrive_max<br>cargo_weight_kg | Queries for a flight with the given characteristics
| `/cargo/confirm` | PUT | flight_plan_id | Customer confirmation of a possible flight plan
| `/cargo/cancel` | DELETE | flight_plan_id | Cancel a flight plan
| `/cargo/vertiports` | POST | latitude, longitude | Get vertiports for a user


## GRPC

### Files

These interfaces are defined in a protocol buffer file, [`svc-cargo-grpc.proto`](../proto/svc-cargo-grpc.proto).

### Integrated Authentication & Encryption

See Services ICD.

### GRPC Server Methods ("Services")

GRPC server methods are called "services", an unfortunate name clash with the broader concept of web services.

| Service | Description |
| ---- | ---- |
| `IsReady` | Returns a message indicating if this service is ready for requests.<br>Similar to a health check, if a server is not "ready" it could be considered dead by the client making the request.

### GRPC Client Messages ("Requests")

| Request | Description |
| ------    | ------- |
| `FlightQuery` | A message to the svc-scheduler in particular
