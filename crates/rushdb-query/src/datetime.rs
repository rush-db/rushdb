//! Query-level datetime operators over the canonical Chrono-backed type
//! implementation in `rushdb-types`.

/// Build an epoch-millisecond timestamp from calendar components.
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
    rushdb_types::datetime::components_to_epoch_ms(year, month, day, hour, minute, second, millis)
}

/// Parse an ISO-8601 datetime string into epoch milliseconds, or `None` when
/// the input is not a valid datetime.
#[must_use]
pub fn parse_iso_8601(value: &str) -> Option<i64> {
    rushdb_types::datetime::parse_datetime(value)
}

/// Parse a `{$year, $month, ...}` component object into epoch milliseconds,
/// or `None` when required components are missing or out of range.
#[must_use]
pub fn parse_components(map: &serde_json::Map<String, serde_json::Value>) -> Option<i64> {
    let year = get(map, "$year")?;
    let month = get(map, "$month").unwrap_or(1);
    let day = get(map, "$day").unwrap_or(1);
    let hour = get(map, "$hour").unwrap_or(0);
    let minute = get(map, "$minute").unwrap_or(0);
    let second = get(map, "$second").unwrap_or(0);
    let millis = get(map, "$millisecond").unwrap_or(0);
    rushdb_types::datetime::components_to_epoch_ms_checked(
        year, month, day, hour, minute, second, millis,
    )
}

/// Epoch milliseconds at the start of `year` (January 1st, 00:00:00.000).
#[must_use]
pub fn year_start(year: i64) -> i64 {
    rushdb_types::datetime::components_to_epoch_ms(year, 1, 1, 0, 0, 0, 0)
}

/// Epoch milliseconds at the start of the year after `year` (exclusive end).
#[must_use]
pub fn year_end(year: i64) -> i64 {
    rushdb_types::datetime::components_to_epoch_ms(year + 1, 1, 1, 0, 0, 0, 0)
}

fn get(map: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<i64> {
    map.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|v| v.try_into().ok()))
    })
}

/// Extract one calendar component (`which`: 0=year .. 6=millisecond) from an
/// epoch-millisecond timestamp.
#[must_use]
pub fn component_of(ms: i64, which: u8) -> i64 {
    rushdb_types::datetime::component_of(ms, which)
}
