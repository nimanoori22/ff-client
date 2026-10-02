//! Fetch the ForexFactory NMI / ISM Services PMI history series.
//!
//! Set `FF_USER_AGENT` in a local, ignored `.env` file to a current browser
//! user-agent string before running this example.

use ff_client::{FfClientConfig, ForexFactoryClient};
use wreq_util::Profile;

const NMI_SERVICES_PMI_EVENT_ID: u64 = 253;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let config = FfClientConfig {
        user_agent: std::env::var("FF_USER_AGENT").expect("FF_USER_AGENT missing in .env"),
        profile: Profile::Chrome143,
        ..Default::default()
    };
    let client = ForexFactoryClient::new(config)?;

    let history = client.history_full(NMI_SERVICES_PMI_EVENT_ID, 20).await?;
    println!("{} NMI / ISM Services PMI releases", history.entries.len());
    if !history.is_complete() {
        eprintln!(
            "history is incomplete: iterations={}, has_more={}, maxed={}",
            history.iterations, history.has_more, history.maxed
        );
    }

    for entry in history.entries.iter().take(5) {
        println!(
            "{:<12} actual={:>7} forecast={:>7} previous={:>7} impact={:?}",
            entry.date,
            entry.actual_formatted,
            entry.forecast_formatted,
            entry.previous_formatted,
            entry.impact,
        );
    }

    Ok(())
}
