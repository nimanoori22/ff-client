//! Retry/fail classification for ForexFactory's history endpoint.
//!
//! Unchanged in spirit from before: a 403/503 is treated as "blocked, don't
//! retry" rather than "transient, retry me" — though with `WreqTransport`
//! actually presenting a real browser TLS fingerprint now, you should see
//! far fewer of these than with plain reqwest in the first place.

use std::time::Duration;

use rustopus::reqwest::StatusCode;
use rustopus::{Decision, Outcome, Policy, TransportError, TransportErrorKind};

// See the note on this in the previous version: reusing rustopus's own
// `HttpStatusError` here would be nicer for consistency across your wrapper
// crates, but I don't have its confirmed field names/visibility in front of
// me, so this is a local equivalent — swap it in if it matches.
#[derive(Debug, thiserror::Error)]
#[error("forex factory returned {status}: {body}")]
pub struct FfHttpError {
    pub status: StatusCode,
    pub body: String,
}

#[derive(Debug, thiserror::Error)]
pub enum FfPolicyError {
    #[error("forex factory returned {status} — likely blocked (bot detection / Cloudflare), not retrying: {body}")]
    Blocked { status: StatusCode, body: String },

    #[error(transparent)]
    Http(#[from] FfHttpError),
}

const MAX_RETRY_AFTER: Duration = Duration::from_secs(60);

pub struct FfPolicy {
    max_retry_after: Duration,
}

impl Default for FfPolicy {
    fn default() -> Self {
        Self {
            max_retry_after: MAX_RETRY_AFTER,
        }
    }
}

impl FfPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn max_retry_after(mut self, d: Duration) -> Self {
        self.max_retry_after = d;
        self
    }

    fn retry_after(&self, outcome: &Outcome) -> Option<Duration> {
        let raw = outcome
            .headers
            .get(rustopus::reqwest::header::RETRY_AFTER)?;
        let secs: u64 = raw.to_str().ok()?.trim().parse().ok()?;
        Some(Duration::from_secs(secs).min(self.max_retry_after))
    }
}

impl Policy for FfPolicy {
    type Error = FfPolicyError;

    fn decide(
        &self,
        _attempt: u32,
        result: &Result<Outcome, TransportError>,
    ) -> Decision<Self::Error> {
        match result {
            Err(e) => match e.kind() {
                TransportErrorKind::Timeout
                | TransportErrorKind::Connect
                | TransportErrorKind::Body => Decision::Retry { after: None },
                _ => Decision::Accept,
            },
            Ok(o) if o.status.is_success() => Decision::Accept,
            Ok(o) if o.status.as_u16() == 403 || o.status.as_u16() == 503 => {
                Decision::Fail(FfPolicyError::Blocked {
                    status: o.status,
                    body: o.text_lossy(),
                })
            }
            Ok(o) if o.status.as_u16() == 429 => Decision::Retry {
                after: self.retry_after(o),
            },
            Ok(o) if matches!(o.status.as_u16(), 500 | 502 | 504) => {
                Decision::Retry { after: None }
            }
            Ok(o) => Decision::Fail(FfPolicyError::Http(FfHttpError {
                status: o.status,
                body: o.text_lossy(),
            })),
        }
    }
}
