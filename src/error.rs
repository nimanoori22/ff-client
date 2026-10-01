use rustopus::reqwest;

use crate::policy::FfPolicyError;

/// What `rustopus::Arm<FfPolicy, WreqTransport>::send` returns on failure.
pub type ArmError = rustopus::Error<FfPolicyError>;

#[derive(Debug, thiserror::Error)]
pub enum FfClientError {
    #[error("max_iterations must be at least 1")]
    InvalidMaxIterations,

    #[error("building the request URL failed: {0}")]
    Url(#[from] url::ParseError),

    #[error("building the HTTP request failed: {0}")]
    RequestBuild(#[from] reqwest::Error),

    #[error("building the wreq client failed: {0}")]
    WreqClientBuild(wreq::Error),

    #[error("request to forex factory failed: {0}")]
    Request(#[from] ArmError),

    #[error("decoding the response body as JSON failed: {0}")]
    Decode(#[from] serde_json::Error),
}
