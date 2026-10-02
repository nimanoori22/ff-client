use chrono::NaiveDate;
use ff_client::history::{parse_history_html, ActualComparison, Impact};

const SAMPLE: &str = r#"
<table class="calendar-event__history alternating calendarhistory">
    <thead class="subhead">
    <tr>
        <th class="calendarhistory__header calendarhistory__header--history">Expected Impact / Date</th>
        <th class="calendarhistory__header calendarhistory__header--actual">Actual</th>
        <th class="calendarhistory__header calendarhistory__header--forecast">Forecast</th>
        <th class="calendarhistory__header calendarhistory__header--previous">Previous</th>
    </tr>
    </thead>
    <tbody>
        <tr>
        <td class="calendarhistory__row nowrap calendarhistory__row--history">
            <span class="icon icon--ff-impact-red"></span>
            <a href="/calendar?day=sep1.2026#detail=146583">Sep 1, 2026</a>
        </td>
            <td class="calendarhistory__row calendarhistory__row--actual">
                <span class="worse">
                    54.6
                </span>
            </td>
            <td class="calendarhistory__row calendarhistory__row--forecast">
                55.2
            </td>
            <td class="calendarhistory__row calendarhistory__row--previous nowrap">
                55.6
            </td>
        </tr>
        <tr>
        <td class="calendarhistory__row nowrap calendarhistory__row--history">
            <span class="icon icon--ff-impact-ora"></span>
            <a href="/calendar?day=may1.2026#detail=146579">May 1, 2026</a>
        </td>
            <td class="calendarhistory__row calendarhistory__row--actual">
                <span class="">
                </span>
            </td>
            <td class="calendarhistory__row calendarhistory__row--forecast">
                53.1
            </td>
            <td class="calendarhistory__row calendarhistory__row--previous nowrap">
                52.7
            </td>
        </tr>
    </tbody>
</table>
"#;

#[test]
fn parses_real_captured_rows() {
    let entries = parse_history_html(SAMPLE);
    assert_eq!(entries.len(), 2);

    let first = &entries[0];
    assert_eq!(first.date, "Sep 1, 2026");
    assert_eq!(first.release_date, NaiveDate::from_ymd_opt(2026, 9, 1));
    assert_eq!(first.day_slug.as_deref(), Some("sep1.2026"));
    assert_eq!(first.release_id, Some(146583));
    assert_eq!(first.impact, Impact::High);
    assert_eq!(first.actual, Some(54.6));
    assert_eq!(first.actual_comparison, ActualComparison::Worse);
    assert_eq!(first.forecast, Some(55.2));
    assert_eq!(first.previous, Some(55.6));

    // Orange impact + blank actual span (release hasn't happened / no data)
    let second = &entries[1];
    assert_eq!(second.impact, Impact::Medium);
    assert_eq!(second.actual, None);
    assert_eq!(second.actual_comparison, ActualComparison::Neutral);
    assert_eq!(second.forecast, Some(53.1));
}
