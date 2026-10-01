//! Lets `Arm<FfPolicy>` keep its rustopus rate-limiting/retry/backoff
//! machinery unchanged while the actual bytes go out over `wreq` instead of
//! `reqwest`.
//!
//! How the pieces fit together: `ForexFactoryClient` still builds requests
//! with a plain `reqwest::Client` (see `client.rs`) purely so rustopus's
//! `Arm::send` — which is signed to take a `reqwest::Request` — has
//! something to clone on each retry attempt. This `WreqTransport` is what
//! `Arm::send` actually calls: it unpacks that `reqwest::Request` (method,
//! URL, headers, body), re-issues it through a `wreq::Client` running a
//! Chrome TLS/JA3/JA4 emulation profile, and repackages the response as a
//! rustopus `Outcome`. rustopus never talks to the network directly here —
//! it only ever sees requests it built and outcomes this file hands back.

use std::future::Future;

use rustopus::{reqwest, Outcome, Transport, TransportError, TransportErrorKind};

pub struct WreqTransport {
    client: wreq::Client,
}

impl WreqTransport {
    pub fn new(client: wreq::Client) -> Self {
        Self { client }
    }
}

impl Transport for WreqTransport {
    fn send(
        &self,
        request: reqwest::Request,
    ) -> impl Future<Output = Result<Outcome, TransportError>> + Send {
        let client = self.client.clone();

        async move {
            let method = request.method().clone();
            let url = request.url().clone();
            let headers = request.headers().clone();
            // Our requests are all small/non-streaming (built fresh by
            // `ForexFactoryClient` for a POST with no body, or none at
            // all), so pulling the body out as plain bytes is safe here —
            // this would need different handling for a streaming body.
            let body: Vec<u8> = request
                .body()
                .and_then(|b| b.as_bytes())
                .map(|b| b.to_vec())
                .unwrap_or_default();

            // `http::Method`/`http::HeaderMap` are what both reqwest 0.13
            // and wreq 6.x build their own Method/HeaderMap types on top
            // of, so these conversions should be direct — but this is the
            // other spot worth a second look if it doesn't compile: it'd
            // mean the two crates have landed on different major versions
            // of the `http` crate underneath.
            let mut wreq_request = client.request(method, url.as_str());
            for (name, value) in headers.iter() {
                wreq_request = wreq_request.header(name, value);
            }
            if !body.is_empty() {
                wreq_request = wreq_request.body(body);
            }

            let response = match wreq_request.send().await {
                Ok(r) => r,
                Err(e) => return Err(wreq_failure_to_transport_error(&e)),
            };

            let status = response.status();
            let headers = response.headers().clone();
            let body = match response.bytes().await {
                Ok(b) => b,
                Err(e) => return Err(wreq_failure_to_transport_error(&e)),
            };

            Ok(Outcome {
                status,
                headers,
                body,
            })
        }
    }
}

fn wreq_failure_to_transport_error(e: &wreq::Error) -> TransportError {
    let kind = if e.is_timeout() {
        TransportErrorKind::Timeout
    } else if e.is_connect() {
        TransportErrorKind::Connect
    } else if e.is_body() || e.is_decode() {
        TransportErrorKind::Body
    } else {
        TransportErrorKind::Other
    };

    TransportError::new(kind, e.to_string())
}
