//! Canonical Date/Datetime parse + serialize round-trips (the engine's typed
//! datetime value layer).

use rushdb_types::datetime::{ms_to_datetime_string, parse_datetime};

#[test]
fn date_only_parses_to_midnight_utc_and_roundtrips() {
    let ms = parse_datetime("2026-07-23").expect("date-only");
    assert_eq!(ms % 86_400_000, 0, "midnight UTC");
    assert_eq!(ms_to_datetime_string(ms), "2026-07-23");
}

#[test]
fn full_iso_with_time_roundtrips() {
    let ms = parse_datetime("2026-07-23T10:30:00Z").expect("iso");
    assert_eq!(ms_to_datetime_string(ms), "2026-07-23T10:30:00.000Z");

    let ms = parse_datetime("2026-07-23T10:30:00.123Z").expect("iso ms");
    assert_eq!(ms_to_datetime_string(ms), "2026-07-23T10:30:00.123Z");
}

#[test]
fn timezone_offsets_apply() {
    // 10:30+02:00 == 08:30Z
    let utc = parse_datetime("2026-07-23T08:30:00Z").unwrap();
    let offset = parse_datetime("2026-07-23T10:30:00+02:00").unwrap();
    assert_eq!(utc, offset);

    let neg = parse_datetime("2026-07-23T06:30:00-02:00").unwrap();
    assert_eq!(utc, neg);
}

#[test]
fn invalid_dates_rejected() {
    assert!(parse_datetime("2026-02-30").is_none(), "Feb 30");
    assert!(parse_datetime("2026-13-01").is_none(), "month 13");
    assert!(parse_datetime("2026-00-10").is_none(), "month 0");
    assert!(parse_datetime("2026-07-32").is_none(), "day 32");
    assert!(parse_datetime("2026-07-23T25:00:00Z").is_none(), "hour 25");
    assert!(parse_datetime("not-a-date").is_none());
}

#[test]
fn leap_years_accepted_and_rejected() {
    assert!(parse_datetime("2024-02-29").is_some(), "leap year");
    assert!(parse_datetime("2026-02-29").is_none(), "non-leap Feb 29");
    assert!(parse_datetime("2000-02-29").is_some(), "century leap year");
    assert!(parse_datetime("1900-02-29").is_none(), "century non-leap");
}

#[test]
fn signed_years_parse_in_order_and_render_signed() {
    // BBY/ABY dates in the LMPG JSON audit: `-0032-…` stayed text while
    // `0000-…` parsed, splitting one property by sign.
    let bc = parse_datetime("-0032-01-01T08:00:00Z").expect("signed year");
    let year_zero = parse_datetime("0000-01-01T05:00:00Z").unwrap();
    let ad = parse_datetime("0034-01-01T18:00:00Z").unwrap();
    assert!(bc < year_zero && year_zero < ad);
    assert_eq!(ms_to_datetime_string(bc), "-0032-01-01T08:00:00.000Z");
    assert_eq!(
        parse_datetime("-0001-01-01T18:00:00Z").map(ms_to_datetime_string),
        Some("-0001-01-01T18:00:00.000Z".to_string())
    );
    assert_eq!(
        parse_datetime("-0600-01-01").map(ms_to_datetime_string),
        Some("-0600-01-01".to_string())
    );
    assert_eq!(
        parse_datetime("-0019-01-01T11:45:00+01:00"),
        parse_datetime("-0019-01-01T10:45:00Z")
    );
    assert!(parse_datetime("-0019-01-01T11:45:00").is_some());
    assert!(parse_datetime("-0032-02-30").is_none(), "Feb 30");
    assert!(parse_datetime("-0032").is_none(), "year only");
    assert!(parse_datetime("--0032-01-01").is_none());
    // Unsigned rendering is unchanged.
    assert_eq!(
        parse_datetime("2026-07-23T10:30:00.123Z").map(ms_to_datetime_string),
        Some("2026-07-23T10:30:00.123Z".to_string())
    );
}
