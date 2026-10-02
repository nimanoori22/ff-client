//! `cargo run --example fetch_pmi`
use rustopus::reqwest;

use ff_client::{FfClientConfig, ForexFactoryClient};
use wreq_util::Profile;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let config = FfClientConfig {
        cookie_header: String::new(), // no cookies
        user_agent: std::env::var("FF_USER_AGENT").expect("User Agent missing in .env"),
        profile: Profile::Chrome143, // matches the UA
        ..Default::default()
    };

    // Direct connection, no proxy. with_clients() ignores config.profile,
    // so the emulation profile is set here.
    let wreq_client = wreq::Client::builder()
        .emulation(config.profile)
        .no_proxy()
        .build()?;

    // Probe: is a plain GET of the calendar page challenged too?
    let probe = wreq_client
        .get("https://www.forexfactory.com/calendar")
        .header("Cookie", &config.cookie_header)
        .send()
        .await?;
    println!("GET /calendar -> {}", probe.status());
    println!("cf-mitigated: {:?}", probe.headers().get("cf-mitigated"));

    let http = reqwest::Client::builder().build()?;
    let client = ForexFactoryClient::with_clients(http, wreq_client, config);

    let history = client.history_full(252, 20).await?;
    println!("{} points total", history.entries.len());
    if !history.is_complete() {
        eprintln!(
            "history is incomplete: iterations={}, has_more={}, maxed={}",
            history.iterations, history.has_more, history.maxed
        );
    }

    for entry in history.entries.iter().take(5) {
        println!(
            "{:<12} actual={:>7} forecast={:>7} previous={:>7} impact={:?} vs_forecast={:?}",
            entry.date,
            entry.actual_formatted,
            entry.forecast_formatted,
            entry.previous_formatted,
            entry.impact,
            entry.actual_comparison,
        );
    }

    Ok(())
}
