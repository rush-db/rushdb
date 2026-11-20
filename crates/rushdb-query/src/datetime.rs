//! Query-level temporal operators over the type implementation in
//! `rushdb-types` (`types::temporal`).

use rushdb_types::types::temporal::{micros_of_components, Component};

/// Parse a `{$year, $month, ...}` component object into wall-clock
/// microseconds, or `None` when the year is missing or a component is out
/// of range. Missing components are the start of their unit.
#[must_use]
pub fn parse_components(map: &serde_json::Map<String, serde_json::Value>) -> Option<i64> {
    let year = get(map, "$year")?;
    let millis = get(map, "$millisecond").unwrap_or(0);
    let micros = get(map, "$microsecond").unwrap_or(0);
    micros_of_components(
        year,
        get(map, "$month").unwrap_or(1),
        get(map, "$day").unwrap_or(1),
        get(map, "$hour").unwrap_or(0),
        get(map, "$minute").unwrap_or(0),
        get(map, "$second").unwrap_or(0),
        millis.checked_mul(1_000)?.checked_add(micros)?,
    )
}

fn get(map: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<i64> {
    map.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|v| v.try_into().ok()))
    })
}

/// The value of component predicate `which` (`0` year … `6` millisecond,
/// `7` microsecond, `8` nanosecond) of a temporal scalar; `None` when the
/// value has no such part.
#[must_use]
pub fn component_value(scalar: &rushdb_types::value::Scalar, which: u8) -> Option<i64> {
    if which == 8 {
        let micros = rushdb_types::types::component_of(scalar, Component::Microsecond)?;
        return micros.checked_mul(1_000);
    }
    rushdb_types::types::component_of(scalar, Component::of_index(which)?)
}
