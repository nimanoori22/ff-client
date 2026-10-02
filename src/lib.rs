//! Rust client for ForexFactory's `calendar/{ebase_event_id}/history/`
//! endpoint — the AJAX call behind a calendar event's "History" panel
//! (PMI, CPI m/m, etc), paged via a `?i=1,2,3,...` counter whose responses
//! are cumulative (see [`ForexFactoryClient::history_full`]).
//!
//! Built on [`rustopus`] for rate limiting, bounded concurrency and
//! retries, and on [`wreq`] for TLS/JA3/JA4 browser fingerprint
//! impersonation — plain `reqwest`'s TLS handshake doesn't look like a
//! browser's, which is a fingerprint bot-detection can key on regardless
//! of what `User-Agent` header you send. See `transport.rs` for how the
//! two are bridged.
//!
//! ```no_run
//! # async fn go() -> Result<(), ff_client::FfClientError> {
//! let client = ff_client::ForexFactoryClient::new(ff_client::FfClientConfig::default())?;
//! let pmi = client.history_full(252, 20).await?;
//! println!("{} points, most recent: {}", pmi.entries.len(), pmi.entries[0].date);
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod error;
pub mod history;
pub mod policy;
pub mod transport;

pub use client::{FfClientConfig, ForexFactoryClient};
pub use error::{ArmError, FfClientError};
pub use history::{ActualComparison, HistoryEntry, HistoryFetch, HistoryPage, Impact};
pub use policy::{FfHttpError, FfPolicy, FfPolicyError};
pub use transport::WreqTransport;

/// Re-exported so callers building requests for [`ForexFactoryClient`]'s
/// escape hatches use the same `reqwest` version this crate does.
pub use rustopus::reqwest;
/// Re-exported for the same reason, for `wreq`.
pub use wreq;
pub use wreq_util;
