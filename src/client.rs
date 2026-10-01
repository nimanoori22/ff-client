use std::num::NonZeroU32;
use std::time::Duration;

use rustopus::{reqwest, Arm, Backoff, Quota};
use url::Url;

use wreq_util::Profile;

use crate::error::FfClientError;
use crate::history::{parse_history_html, HistoryEntry, HistoryPage};
use crate::policy::FfPolicy;
use crate::transport::WreqTransport;

const BASE_URL: &str = "https://www.forexfactory.com/calendar/";

/// Knobs for [`ForexFactoryClient`].
#[derive(Debug, Clone)]
pub struct FfClientConfig {
    pub max_concurrent_requests: usize,
    pub requests_per_second: Option<u32>,
    pub max_retries: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub cookie_header: String,
    pub user_agent: String,

    /// Which browser wreq impersonates at the TLS/JA3/JA4 level. Defaults
    /// to a recent Chrome — worth updating occasionally as
    /// bot-detection services expand their fingerprint databases and older
    /// emulation profiles become distinguishable again.
    pub profile: Profile,

    /// Sent as `Referer` on every request. The capture you gave me doesn't
    /// show the full request headers Chrome actually sent (only status/
    /// method/remote address), so this is an assumption about what a real
    /// browser would send hitting this endpoint from the calendar page —
    /// worth confirming against your browser's network tab if you hit
    /// unexpected failures, and adjusting per event if the real Referer
    /// includes the specific event's calendar day.
    pub referer: String,

    /// Sent as `X-Requested-With: XMLHttpRequest` when true (default) —
    /// same caveat as `referer`: an assumption based on this being an
    /// AJAX-style endpoint, not confirmed against the real request headers.
    pub send_ajax_header: bool,
}

impl Default for FfClientConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 1,
            requests_per_second: Some(1),
            max_retries: 3,
            initial_backoff: Duration::from_millis(750),
            max_backoff: Duration::from_secs(20),
            profile: Profile::Chrome131,
            referer: "https://www.forexfactory.com/calendar".to_string(),
            send_ajax_header: true,
            cookie_header: String::new(),
            user_agent: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct ForexFactoryClient {
    /// Used only to build `reqwest::Request` objects for rustopus's arm to
    /// clone on retries — never actually executed. See `transport.rs`.
    http: reqwest::Client,
    arm: Arm<FfPolicy, WreqTransport>,
    referer: String,
    send_ajax_header: bool,
    cookie_header: String,
    user_agent: String,
}

impl ForexFactoryClient {
    pub fn new(config: FfClientConfig) -> Result<Self, FfClientError> {
        let http = reqwest::Client::builder().build()?;
        let wreq_client = wreq::Client::builder()
            .emulation(config.profile)
            .build()
            .map_err(FfClientError::WreqClientBuild)?;
        Ok(Self::with_clients(http, wreq_client, config))
    }

    /// Escape hatch for bringing your own pre-built clients (a shared wreq
    /// client with a specific proxy/cookie-store setup, for instance).
    pub fn with_clients(
        http: reqwest::Client,
        wreq_client: wreq::Client,
        config: FfClientConfig,
    ) -> Self {
        let transport = WreqTransport::new(wreq_client);

        let mut builder = Arm::builder()
            .name("forexfactory-history")
            .suckers(config.max_concurrent_requests)
            .max_attempts(config.max_retries + 1)
            .backoff(Backoff::exponential(config.initial_backoff).cap(config.max_backoff))
            .policy(FfPolicy::new());

        if let Some(rps) = config.requests_per_second.and_then(NonZeroU32::new) {
            builder = builder.rate(Quota::per_second(rps));
        }

        Self {
            arm: builder.build_with(transport),
            http,
            referer: config.referer,
            send_ajax_header: config.send_ajax_header,
            cookie_header: config.cookie_header,
            user_agent: config.user_agent,
        }
    }

    /// Fetch one cumulative history page for `ebase_event_id`. `i` starts at
    /// one; larger values contain the earlier pages plus additional history.
    pub async fn history_page(
        &self,
        ebase_event_id: u64,
        i: u32,
    ) -> Result<HistoryPage, FfClientError> {
        let mut url = Url::parse(BASE_URL)?.join(&format!("{ebase_event_id}/history/"))?;
        url.query_pairs_mut().append_pair("i", &i.to_string());

        let mut builder = self
            .http
            .post(url)
            .header(reqwest::header::REFERER, &self.referer)
            .header("Origin", "https://www.forexfactory.com")
            .header(
                reqwest::header::ACCEPT,
                "application/json, text/javascript, */*; q=0.01",
            )
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded; charset=UTF-8",
            )
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            // Empty body so the POST is well-formed
            .body("");

        if !self.cookie_header.is_empty() {
            builder = builder.header(reqwest::header::COOKIE, &self.cookie_header);
        }

        if self.send_ajax_header {
            builder = builder.header("X-Requested-With", "XMLHttpRequest");
        }

        let request = builder.build()?;
        let outcome = self.arm.send(request).await?;

        Ok(serde_json::from_slice(&outcome.body)?)
    }

    /// Pages through `i = 1, 2, 3, ...` until ForexFactory reports
    /// `has_more: false` or `maxed: true` (or `max_iterations` is hit as a
    /// hard ceiling against an unexpected infinite loop), then parses the
    /// final — and by then complete — page's HTML table.
    ///
    /// `max_iterations` must be at least one.
    pub async fn history_full(
        &self,
        ebase_event_id: u64,
        max_iterations: u32,
    ) -> Result<Vec<HistoryEntry>, FfClientError> {
        if max_iterations == 0 {
            return Err(FfClientError::InvalidMaxIterations);
        }

        let mut i = 1;
        let mut page = self.history_page(ebase_event_id, i).await?;

        while page.has_more && !page.maxed && i < max_iterations {
            i += 1;
            page = self.history_page(ebase_event_id, i).await?;
        }

        Ok(parse_history_html(&page.history))
    }
}

#[cfg(test)]
mod tests {
    use super::{FfClientConfig, ForexFactoryClient};
    use crate::error::FfClientError;

    #[tokio::test]
    async fn history_full_rejects_zero_max_iterations_without_a_request() {
        let client = ForexFactoryClient::new(FfClientConfig::default()).unwrap();
        let error = client.history_full(252, 0).await.unwrap_err();

        assert!(matches!(error, FfClientError::InvalidMaxIterations));
    }
}
