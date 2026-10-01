//! Parses the HTML `<table>` fragment ForexFactory embeds in the `history`
//! field of its `calendar/{ebase_event_id}/history/?i={n}` JSON response.
//!
//! Row shape, from the payload you captured:
//!
//! ```html
//! <tr>
//!   <td class="calendarhistory__row--history">
//!     <span class="icon icon--ff-impact-red"></span>
//!     <a href="/calendar?day=sep1.2026#detail=146583">Sep 1, 2026</a>
//!   </td>
//!   <td class="calendarhistory__row--actual"><span class="worse">54.6</span></td>
//!   <td class="calendarhistory__row--forecast">55.2</td>
//!   <td class="calendarhistory__row--previous nowrap">55.6</td>
//! </tr>
//! ```

use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};

/// The wrapping JSON envelope, before the HTML inside `history` gets parsed.
#[derive(Debug, Clone, Deserialize)]
pub struct HistoryPage {
    /// The `i` value that produced this page (mirrors what you sent).
    pub iterations: u32,
    /// True if the server capped how far back it would go, independent of
    /// whether more history actually exists — see [`crate::client::ForexFactoryClient::history_full`].
    pub maxed: bool,
    /// True if a larger `i` would return more (or equally capped) history.
    pub has_more: bool,
    /// Raw HTML fragment — parse with [`parse_history_html`].
    pub history: String,
}

/// How ForexFactory colors the "impact" icon for a release.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Impact {
    High,
    Medium,
    Low,
    NonEconomic,
    /// Any icon class this crate doesn't recognize yet, kept verbatim
    /// rather than dropped, in case ForexFactory adds a color this wasn't
    /// written against.
    Unknown(String),
}

impl Impact {
    fn from_icon_class(class_attr: &str) -> Self {
        // e.g. "icon icon--ff-impact-red" -> "red"
        let Some(color) = class_attr
            .split_whitespace()
            .find_map(|c| c.strip_prefix("icon--ff-impact-"))
        else {
            return Impact::Unknown(class_attr.to_string());
        };
        match color {
            "red" => Impact::High,
            "ora" => Impact::Medium,
            "yel" => Impact::Low,
            "gra" | "grey" | "gray" => Impact::NonEconomic,
            other => Impact::Unknown(other.to_string()),
        }
    }
}

/// Whether ForexFactory colored the actual value as a pleasant or
/// unpleasant surprise relative to forecast (its `better`/`worse` CSS
/// classes), or left it uncolored.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub enum ActualComparison {
    Better,
    Worse,
    #[default]
    Neutral,
}

impl ActualComparison {
    fn from_span_class(class_attr: &str) -> Self {
        if class_attr.split_whitespace().any(|c| c == "better") {
            ActualComparison::Better
        } else if class_attr.split_whitespace().any(|c| c == "worse") {
            ActualComparison::Worse
        } else {
            ActualComparison::Neutral
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryEntry {
    /// ForexFactory's own display label, e.g. `"Sep 1, 2026"`.
    pub date: String,
    /// The `day=` query value from the row's link, e.g. `"sep1.2026"` —
    /// this is ForexFactory's own slug for the calendar day, useful if you
    /// want to link back to `/calendar?day=...`.
    pub day_slug: Option<String>,
    /// The `#detail=` fragment from that same link — ForexFactory's id for
    /// *this specific release* (distinct from the series-level
    /// `ebase_event_id` used in the URL you fetch history from).
    pub release_id: Option<u64>,

    pub impact: Impact,

    pub actual_formatted: String,
    pub actual: Option<f64>,
    pub actual_comparison: ActualComparison,

    pub forecast_formatted: String,
    pub forecast: Option<f64>,

    pub previous_formatted: String,
    pub previous: Option<f64>,
}

/// Strips a trailing `%` and whitespace, returning `None` for blank cells
/// (a release that hasn't happened yet, or a series missing a previous
/// value) rather than failing to parse.
fn parse_numeric(text: &str) -> Option<f64> {
    let trimmed = text.trim().trim_end_matches('%').trim();
    if trimmed.is_empty() {
        None
    } else {
        trimmed.parse().ok()
    }
}

fn text_of(el: ElementRef) -> String {
    el.text().collect::<String>().trim().to_string()
}

/// Parses every `<tr>` in the history table into a [`HistoryEntry`].
///
/// Selectors are written against the classes actually present in your
/// sample payload; if ForexFactory changes its markup this will start
/// silently returning fewer/empty fields rather than erroring, since
/// `scraper` selectors just match nothing rather than fail — worth
/// spot-checking row counts against what you expect if results look thin.
pub fn parse_history_html(html: &str) -> Vec<HistoryEntry> {
    let document = Html::parse_document(html);

    let row_sel = Selector::parse("tbody tr").expect("static selector");
    let date_link_sel =
        Selector::parse("td.calendarhistory__row--history a").expect("static selector");
    let icon_sel =
        Selector::parse("td.calendarhistory__row--history span.icon").expect("static selector");
    let actual_span_sel =
        Selector::parse("td.calendarhistory__row--actual span").expect("static selector");
    let forecast_sel =
        Selector::parse("td.calendarhistory__row--forecast").expect("static selector");
    let previous_sel =
        Selector::parse("td.calendarhistory__row--previous").expect("static selector");

    document
        .select(&row_sel)
        .map(|row| {
            let (date, day_slug, release_id) = row
                .select(&date_link_sel)
                .next()
                .map(|a| {
                    let date = text_of(a);
                    let href = a.value().attr("href").unwrap_or_default();
                    let day_slug = href
                        .split("day=")
                        .nth(1)
                        .map(|rest| rest.split('#').next().unwrap_or(rest).to_string());
                    let release_id = href
                        .split("detail=")
                        .nth(1)
                        .and_then(|rest| rest.parse::<u64>().ok());
                    (date, day_slug, release_id)
                })
                .unwrap_or_default();

            let impact = row
                .select(&icon_sel)
                .next()
                .map(|el| Impact::from_icon_class(el.value().attr("class").unwrap_or_default()))
                .unwrap_or(Impact::Unknown(String::new()));

            let (actual_formatted, actual_comparison) = row
                .select(&actual_span_sel)
                .next()
                .map(|el| {
                    let comparison = ActualComparison::from_span_class(
                        el.value().attr("class").unwrap_or_default(),
                    );
                    (text_of(el), comparison)
                })
                .unwrap_or_default();

            let forecast_formatted = row
                .select(&forecast_sel)
                .next()
                .map(text_of)
                .unwrap_or_default();

            let previous_formatted = row
                .select(&previous_sel)
                .next()
                .map(text_of)
                .unwrap_or_default();

            HistoryEntry {
                date,
                day_slug,
                release_id,
                impact,
                actual: parse_numeric(&actual_formatted),
                actual_formatted,
                actual_comparison,
                forecast: parse_numeric(&forecast_formatted),
                forecast_formatted,
                previous: parse_numeric(&previous_formatted),
                previous_formatted,
            }
        })
        .collect()
}
