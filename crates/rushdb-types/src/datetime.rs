//! Canonical Date/Datetime handling backed by Chrono's ISO-8601 calendar
//! implementation. `RushDB` stores instants as UTC epoch milliseconds.

use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};

const DATE: &str = "%Y-%m-%d";
const LOCAL_DATETIME: &str = "%Y-%m-%dT%H:%M:%S%.f";
const MS_PER_DAY: i64 = 86_400_000;

/// Parse a date-only value, RFC 3339 timestamp, or timezone-less ISO local
/// datetime (defined by `RushDB` as UTC) into epoch milliseconds.
#[must_use]
pub fn parse_datetime(value: &str) -> Option<i64> {
    let value = value.trim();
    // A signed year (`-0032-01-01T08:00:00Z`, ISO 8601 astronomical year
    // numbering: year 0 is 1 BC) in the same layouts as an unsigned one.
    // RFC 3339 has no signed years, so Chrono's RFC 3339 parser rejects
    // them; without this a property of BC and AD dates split by sign into
    // text and datetimes.
    if let Some(ms) = parse_signed_year(value) {
        return Some(ms);
    }
    if let Ok(datetime) = DateTime::parse_from_rfc3339(value) {
        return Some(datetime.timestamp_millis());
    }

    // Chrono follows RFC 3339's uppercase `Z`; accepting lowercase `z`
    // preserves the database's historical input convenience at the edge.
    if let Some(prefix) = value.strip_suffix('z') {
        let normalized = format!("{prefix}Z");
        if let Ok(datetime) = DateTime::parse_from_rfc3339(&normalized) {
            return Some(datetime.timestamp_millis());
        }
    }

    if let Ok(datetime) = NaiveDateTime::parse_from_str(value, LOCAL_DATETIME) {
        return Some(datetime.and_utc().timestamp_millis());
    }
    NaiveDate::parse_from_str(value, DATE)
        .ok()?
        .and_hms_opt(0, 0, 0)
        .map(|datetime| datetime.and_utc().timestamp_millis())
}

/// [`parse_datetime`] for a `-` or `+` signed year: date-only, local
/// datetime with seconds, or with `Z` or a `±HH:MM` offset; `T` separates
/// date and time.
fn parse_signed_year(value: &str) -> Option<i64> {
    let (sign, rest) = value.split_at_checked(1)?;
    let digits = rest.split('-').next()?;
    if !matches!(sign, "-" | "+") || digits.len() < 4 || !digits.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    if let Ok(datetime) = DateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f%:z") {
        return Some(datetime.timestamp_millis());
    }
    if let Some(local) = value.strip_suffix(['Z', 'z']) {
        return NaiveDateTime::parse_from_str(local, LOCAL_DATETIME)
            .ok()
            .map(|datetime| datetime.and_utc().timestamp_millis());
    }
    if let Ok(datetime) = NaiveDateTime::parse_from_str(value, LOCAL_DATETIME) {
        return Some(datetime.and_utc().timestamp_millis());
    }
    NaiveDate::parse_from_str(value, DATE)
        .ok()?
        .and_hms_opt(0, 0, 0)
        .map(|datetime| datetime.and_utc().timestamp_millis())
}

/// Convert calendar components to epoch milliseconds, rejecting values that
/// fall outside the representable range.
///
/// Returns `None` if any component is out of range or the combination is not a
/// valid calendar instant.
#[must_use]
pub fn components_to_epoch_ms_checked(
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    millis: i64,
) -> Option<i64> {
    let year = i32::try_from(year).ok()?;
    let month = u32::try_from(month).ok()?;
    let day = u32::try_from(day).ok()?;
    let hour = u32::try_from(hour).ok()?;
    let minute = u32::try_from(minute).ok()?;
    let second = u32::try_from(second).ok()?;
    let millis = u32::try_from(millis).ok()?;
    Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
        .single()?
        .with_nanosecond(millis.checked_mul(1_000_000)?)
        .map(|datetime| datetime.timestamp_millis())
}

/// Build epoch milliseconds from validated calendar components.
///
/// Callers that accept untrusted components should use
/// [`components_to_epoch_ms_checked`].
///
/// # Panics
///
/// Panics if the components do not form a valid calendar instant.
#[must_use]
pub fn components_to_epoch_ms(
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    millis: i64,
) -> i64 {
    components_to_epoch_ms_checked(year, month, day, hour, minute, second, millis)
        .expect("valid datetime components")
}

/// Report whether a string parses as any supported datetime form.
#[must_use]
pub fn is_datetime_string(value: &str) -> bool {
    parse_datetime(value).is_some()
}

/// Extract epoch milliseconds from a JSON number or datetime string.
///
/// Returns `None` for JSON values that are neither an integer nor a parseable
/// datetime string.
#[must_use]
pub fn json_datetime_ms(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(number) => number.as_i64(),
        serde_json::Value::String(value) => parse_datetime(value),
        _ => None,
    }
}

fn utc(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).expect("epoch milliseconds in Chrono range")
}

/// Epoch milliseconds for the first instant of the calendar month containing
/// `ms`.
///
/// # Panics
///
/// Panics if `ms` is outside the range representable by Chrono.
#[must_use]
pub fn start_of_month(ms: i64) -> i64 {
    let datetime = utc(ms);
    Utc.with_ymd_and_hms(datetime.year(), datetime.month(), 1, 0, 0, 0)
        .single()
        .expect("valid month start")
        .timestamp_millis()
}

/// Epoch milliseconds for the first instant of the calendar year containing
/// `ms`.
///
/// # Panics
///
/// Panics if `ms` is outside the range representable by Chrono.
#[must_use]
pub fn start_of_year(ms: i64) -> i64 {
    let datetime = utc(ms);
    Utc.with_ymd_and_hms(datetime.year(), 1, 1, 0, 0, 0)
        .single()
        .expect("valid year start")
        .timestamp_millis()
}

/// Start of the 3-month calendar quarter containing `ms`
/// (quarters start at months 1, 4, 7, 10).
///
/// # Panics
///
/// Panics if `ms` is outside the range representable by Chrono.
#[must_use]
pub fn start_of_quarter(ms: i64) -> i64 {
    let datetime = utc(ms);
    let quarter_start_month = ((datetime.month() - 1) / 3) * 3 + 1;
    Utc.with_ymd_and_hms(datetime.year(), quarter_start_month, 1, 0, 0, 0)
        .single()
        .expect("valid quarter start")
        .timestamp_millis()
}

/// Serialize epoch milliseconds as a date at midnight UTC, otherwise as an
/// RFC-3339 UTC timestamp with millisecond precision.
#[must_use]
pub fn ms_to_datetime_string(ms: i64) -> String {
    let datetime = utc(ms);
    if ms.rem_euclid(MS_PER_DAY) == 0 {
        datetime.format(DATE).to_string()
    } else {
        // RFC 3339 layout with milliseconds and `Z`; `%Y` also signs a year
        // before 0000 or after 9999, which Chrono's RFC 3339 writer cannot.
        datetime.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
    }
}

/// Extract one calendar component from epoch milliseconds, using Chrono's
/// field order: `0` year, `1` month, `2` day, `3` hour, `4` minute,
/// `5` second, `6` millisecond, `7` microsecond, `8` nanosecond.
///
/// Returns `0` for unknown component indices.
#[must_use]
pub fn component_of(ms: i64, which: u8) -> i64 {
    let datetime = utc(ms);
    match which {
        0 => i64::from(datetime.year()),
        1 => i64::from(datetime.month()),
        2 => i64::from(datetime.day()),
        3 => i64::from(datetime.hour()),
        4 => i64::from(datetime.minute()),
        5 => i64::from(datetime.second()),
        6 => i64::from(datetime.timestamp_subsec_millis()),
        7 => i64::from(datetime.timestamp_subsec_micros()),
        8 => i64::from(datetime.timestamp_subsec_nanos()),
        _ => 0,
    }
}
