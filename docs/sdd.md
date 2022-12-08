# `svc-cargo` - Software Design Document (SDD)

<center>

<img src="https://github.com/Arrow-air/tf-github/raw/main/src/templates/doc-banner-services.png" style="height:250px" />

</center>

## Overview

This document details the software implementation of `svc-cargo`.

This process is responsible for handling interactions with clients for cargo shipments.

Interactions include querying for available flights, confirming flights, and cancelling flights.

This service will primarily communicate with a client-side user interface, such as a website or mobile application. It stands between clients and the core Arrow services network, denying ill-formed requests and limiting traffic.

Attribute | Description
--- | ---
Status | Development
Stuckee | A.M. Smith ([@ServiceDog](https://github.com/ServiceDog))

## Related Documents

Document | Description
--- | ---
| [High-Level Concept of Operations (CONOPS)](https://github.com/Arrow-air/se-services/blob/develop/docs/conops.md) | Overview of Arrow microservices.                             |
| [High-Level Interface Control Document (ICD)](https://github.com/Arrow-air/se-services/blob/develop/docs/icd.md)  | Interfaces and frameworks common to all Arrow microservices. |
[Requirements - `svc-cargo`](https://docs.google.com/spreadsheets/d/1OliSp9BDvMuVvGmSRh1z_Z58QtjlSknLxGVdVZs2l7A/edit#gid=0) | Requirements and user stories for this microservice.
[Concept of Operations - `svc-cargo`](./conops.md) | Defines the motivation and duties of this microservice.
[Interface Control Document (ICD) - `svc-cargo`](./icd.md) | Defines the inputs and outputs of this microservice.

## Frameworks

See the [High-Level Services ICD](https://github.com/Arrow-air/se-services/blob/develop/docs/icd.md).

## Location

Server-side service.

## Module Attributes

Attribute | Applies | Explanation
--- | --- | ---
Safety Critical | No | This is a client-facing process that is not essential to the function of the underlying services network.

## Global Variables

None

## Logic 

### Initialization

At initialization this service creates two servers on separate threads: a GRPC server and a REST server.

The REST server expects the following environment variables to be set:
- `DOCKER_PORT_REST` (default: `8000`)

The GRPC server expects the following environment variables to be set:
- `DOCKER_PORT_GRPC` (default: `50051`)
### Control Loop

As a REST and GRPC server, this service awaits requests and executes handlers.

Some handlers **require** the following environment variables to be set:
- `SCHEDULER_HOST_GRPC`
- `SCHEDULER_PORT_GRPC`
- `PRICING_HOST_GRPC`
- `PRICING_PORT_GRPC`
- `STORAGE_HOST_GRPC`
- `STORAGE_PORT_GRPC`

This information allows `svc-cargo` to connect to other microservices to obtain information requested by the client.

:exclamation: These environment variables will *not* default to anything if not found. In this case, requests involving the handler will result in a `503 SERVICE UNAVAILABLE`.

For detailed sequence diagrams regarding request handlers, see [REST Handlers](#rest-handlers).

### Cleanup

None

## REST Handlers

### `vertiports` Handler

The client will request a list of vertiports for their region, so they can choose their departure and destination ports.

This handler makes a request to `svc-storage`.

**(vertiports) Nominal**
```mermaid
sequenceDiagram
    autonumber
    participant client as Client App
    participant cargo as svc-cargo
    participant storage as svc-storage
    client-->>cargo: (REST) POST /cargo/vertiports
    cargo-->>cargo: Connect to svc-storage
    cargo-->>storage: (GRPC REQ) get_vertiports
    storage-->>cargo: (GRPC REP) <list of vertiports>
    cargo-->>client: (200 OK) <list of vertiports>
```

**(vertiports) Off-Nominal**: Failed to connect to svc-storage
```mermaid
sequenceDiagram
    autonumber
    participant client as Client App
    participant cargo as svc-cargo
    participant storage as svc-storage
    client-->>cargo: (REST) POST /cargo/vertiports
    cargo-->>cargo: Connect to svc-storage
    note over cargo: Failed to connect
    cargo-->>client: (503 SERVICE UNAVAILABLE)
```

**(vertiports)  Off-Nominal**: Request to svc-storage failed
```mermaid
sequenceDiagram
    autonumber
    participant client as Client App
    participant cargo as svc-cargo
    participant storage as svc-storage
    client-->>cargo: (REST) POST /cargo/vertiports
    cargo-->>cargo: Connect to svc-storage
    cargo-->>storage: (GRPC REQ) get_vertiports
    storage-->>cargo: (GRPC REP) Error
    cargo-->>client: (409 CONFLICT)
```

### `query` Handler

The client will send a query to `svc-cargo` including vertiports and time of departure. `svc-cargo` will forward valid requests to `svc-scheduler`

This handler makes requests to `svc-scheduler` and `svc-pricing`.

**(query) Nominal**
```mermaid
sequenceDiagram
    autonumber
    participant client as Client App
    participant cargo as svc-cargo
    participant scheduler as svc-scheduler
    participant pricing as svc-pricing
    client-->>cargo: (REST) POST /cargo/query
    cargo-->>cargo: Validate request
    cargo-->>cargo: Connect to svc-scheduler and svc-pricing
    cargo-->>scheduler: (GRPC REQ) query_flight
    scheduler-->>cargo: (GRPC REP) <list of flight plans>

    loop per plan
        cargo-->>pricing: (GRPC REQ) get_pricing
        pricing-->>cargo: (GRPC REP) <pricing>
    end

    cargo-->>client: (200 OK) <list of priced flight plans>
```

**(query) Off-Nominal**: Invalid request body

This can occur if invalid time windows or vertiport IDs are provided by the client.

```mermaid
sequenceDiagram
    autonumber
    participant client as Client App
    participant cargo as svc-cargo
    participant scheduler as svc-scheduler
    participant pricing as svc-pricing
    client-->>cargo: (REST) POST /cargo/query
    cargo-->>cargo: Validate request
    note over cargo: Invalid request body
    cargo-->>client: (400 BAD REQUEST)
```

**(query) Off-Nominal**: Unable to connect to svc-scheduler or svc-pricing

```mermaid
sequenceDiagram
    autonumber
    participant client as Client App
    participant cargo as svc-cargo
    participant scheduler as svc-scheduler
    participant pricing as svc-pricing
    client-->>cargo: (REST) POST /cargo/query
    cargo-->>cargo: Validate request
    cargo-->>cargo: Connect to svc-scheduler and svc-pricing
    note over cargo: Failed to connect
    cargo-->>client: (503 SERVICE UNAVAILABLE)
```

**(query) Off-Nominal**: Request to svc-scheduler fails

```mermaid
sequenceDiagram
    autonumber
    participant client as Client App
    participant cargo as svc-cargo
    participant scheduler as svc-scheduler
    participant pricing as svc-pricing
    client-->>cargo: (REST) POST /cargo/query
    cargo-->>cargo: Validate request
    cargo-->>cargo: Connect to svc-scheduler and svc-pricing
    cargo-->>scheduler: (GRPC REQ) query_flight
    scheduler-->>cargo: (GRPC REP) Error
    cargo-->>client: (409 CONFLICT)
```

**(query) Off-Nominal**: Request to svc-pricing fails

```mermaid
sequenceDiagram
    autonumber
    participant client as Client App
    participant cargo as svc-cargo
    participant scheduler as svc-scheduler
    participant pricing as svc-pricing
    client-->>cargo: (REST) POST /cargo/query
    cargo-->>cargo: Validate request
    cargo-->>cargo: Connect to svc-scheduler and svc-pricing
    cargo-->>scheduler: (GRPC REQ) query_flight
    scheduler-->>cargo: (GRPC REP) <list of flight plans>

    loop per plan
        cargo-->>pricing: (GRPC REQ) get_pricing
        pricing-->>cargo: (GRPC REP) Error
        note over cargo: break loop
    end

    cargo-->>client: (409 CONFLICT)
```

### `confirm` Handler

The client will choose a flight plan from their list of options and confirm it through its unique *draft* UUID.

:exclamation: A nominal reply to the client will contain confirmation and a *new* flight plan UUID that the client must use for future requests (such as cancelling). The original `draft` UUID used to confirm the flight is discarded when a flight is confirmed.

This handler makes a request to `svc-scheduler`.

**(confirm) Nominal**
```mermaid
sequenceDiagram
    autonumber
    participant client as Client App
    participant cargo as svc-cargo
    participant scheduler as svc-scheduler
    client-->>cargo: (REST) PUT /cargo/confirm
    cargo-->>cargo: Validate request
    cargo-->>cargo: Connect to svc-scheduler
    cargo-->>scheduler: (GRPC REQ) confirm_flight
    scheduler-->>cargo: (GRPC REP) <confirmation, new flight plan ID>
    cargo-->>client: (200 OK) <confirmation, new flight plan ID>
```

**(confirm) Off-Nominal**: Invalid request body

This can occur if an invalid flight plan ID format is provided by the client.

```mermaid
sequenceDiagram
    autonumber
    participant client as Client App
    participant cargo as svc-cargo
    participant scheduler as svc-scheduler
    client-->>cargo: (REST) PUT /cargo/confirm
    cargo-->>cargo: Validate request
    note over cargo: Invalid request
    cargo-->>client: (400 BAD REQUEST)
```


**(confirm) Off-Nominal**: Unable to connect to svc-scheduler

```mermaid
sequenceDiagram
    autonumber
    participant client as Client App
    participant cargo as svc-cargo
    participant scheduler as svc-scheduler
    client-->>cargo: (REST) PUT /cargo/confirm
    cargo-->>cargo: Validate request
    cargo-->>cargo: Connect to svc-scheduler
    note over cargo: Failed to connect
    cargo-->>client: (503 SERVICE UNAVAILABLE)
```

**(confirm) Off-Nominal**: Request to svc-scheduler fails

```mermaid
sequenceDiagram
    autonumber
    participant client as Client App
    participant cargo as svc-cargo
    participant scheduler as svc-scheduler
    client-->>cargo: (REST) PUT /cargo/confirm
    cargo-->>cargo: Validate request
    cargo-->>cargo: Connect to svc-scheduler
    cargo-->>scheduler: (GRPC REQ) confirm_flight
    scheduler-->>cargo: (GRPC REP) Error
    cargo-->>client: (409 CONFLICT)
```

### `cancel` Handler

The client may cancel a flight plan through its unique UUID.

This handler makes a request to `svc-scheduler`.

**(cancel) Nominal**
```mermaid
sequenceDiagram
    autonumber
    participant client as Client App
    participant cargo as svc-cargo
    participant scheduler as svc-scheduler
    client-->>cargo: (REST) DELETE /cargo/cancel
    cargo-->>cargo: Validate request
    cargo-->>cargo: Connect to svc-scheduler
    cargo-->>scheduler: (GRPC REQ) confirm_flight
    scheduler-->>cargo: (GRPC REP) <confirmation, new flight plan ID>
    cargo-->>client: (200 OK) <list of priced flight plans>
```

**(cancel) Off-Nominal**: Invalid request body

This can occur if an invalid flight plan ID format is provided by the client.

```mermaid
sequenceDiagram
    autonumber
    participant client as Client App
    participant cargo as svc-cargo
    participant scheduler as svc-scheduler
    client-->>cargo: (REST) DELETE /cargo/cancel
    cargo-->>cargo: Validate request
    note over cargo: Invalid request
    cargo-->>client: (400 BAD REQUEST)
```


**(cancel) Off-Nominal**: Unable to connect to svc-scheduler

```mermaid
sequenceDiagram
    autonumber
    participant client as Client App
    participant cargo as svc-cargo
    participant scheduler as svc-scheduler
    client-->>cargo: (REST) DELETE /cargo/cancel
    cargo-->>cargo: Validate request
    cargo-->>cargo: Connect to svc-scheduler
    note over cargo: Failed to connect
    cargo-->>client: (503 SERVICE UNAVAILABLE)
```

**(cancel) Off-Nominal**: Request to svc-scheduler fails
```mermaid
sequenceDiagram
    autonumber
    participant client as Client App
    participant cargo as svc-cargo
    participant scheduler as svc-scheduler
    client-->>cargo: (REST) DELETE /cargo/cancel
    cargo-->>cargo: Validate request
    cargo-->>cargo: Connect to svc-scheduler
    cargo-->>scheduler: (GRPC REQ) confirm_flight
    scheduler-->>cargo: (GRPC REP) Error
    cargo-->>client: (409 CONFLICT)
```
