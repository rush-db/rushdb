//! `RushDB`'s typed values and Grafeo's, both ways: a date is a `Date`, a
//! time of day a `Time` without an offset, a local date-time a
//! `Timestamp`, a datetime a `ZonedDatetime` (its offset kept), a duration
//! a `Duration`. A point, which Grafeo does not have, is its `GeoJSON` map.

use grafeo_common::types::{
    Date as GDate, Duration as GDuration, Time as GTime, Timestamp, Value, ZonedDatetime,
};
use rushdb_types::types::duration::Duration;
use rushdb_types::types::temporal::Instant;
use rushdb_types::value::Scalar;

/// The Grafeo value of a typed scalar; `None` for the JSON-native types
/// and points, which the caller converts as JSON.
pub(crate) fn to_grafeo(scalar: &Scalar) -> Option<Value> {
    Some(match scalar {
        Scalar::Date(days) => Value::Date(GDate::from_days(*days)),
        Scalar::LocalTime(micros) => Value::Time(GTime::from_nanos(
            u64::try_from(*micros).ok()?.checked_mul(1_000)?,
        )?),
        Scalar::LocalDateTime(micros) => Value::Timestamp(Timestamp::from_micros(*micros)),
        Scalar::DateTime(instant) => Value::ZonedDatetime(ZonedDatetime::from_timestamp_offset(
            Timestamp::from_micros(instant.micros),
            instant.offset,
        )),
        Scalar::Duration(duration) => Value::Duration(GDuration::new(
            i64::from(duration.months),
            i64::from(duration.days),
            duration.micros.checked_mul(1_000)?,
        )),
        _ => return None,
    })
}

/// The typed scalar of a Grafeo temporal value; `None` for other values
/// and for what `RushDB` does not store as a typed value (a time of day with
/// an offset, a duration beyond its range): those are written as text.
/// Nanoseconds are truncated to microseconds.
pub(crate) fn from_grafeo(value: &Value) -> Option<Scalar> {
    Some(match value {
        Value::Date(date) => Scalar::Date(date.as_days()),
        Value::Time(time) if time.offset_seconds().is_none() => {
            Scalar::LocalTime(i64::try_from(time.as_nanos() / 1_000).ok()?)
        }
        Value::Timestamp(timestamp) => Scalar::LocalDateTime(timestamp.as_micros()),
        Value::ZonedDatetime(datetime) => Scalar::DateTime(Instant {
            micros: datetime.as_timestamp().as_micros(),
            offset: datetime.offset_seconds(),
        }),
        Value::Duration(duration) => Scalar::Duration(Duration {
            months: i32::try_from(duration.months()).ok()?,
            days: i32::try_from(duration.days()).ok()?,
            micros: duration.nanos() / 1_000,
        }),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rushdb_types::types::infer_text;

    #[test]
    fn typed_values_round_trip_through_grafeo() {
        for text in [
            "2024-05-01",
            "10:15:00.25",
            "2024-05-01T10:15:00.000001",
            "2024-05-01T10:15:00.000-05:30",
            "P1Y2M3DT4H5M6.5S",
        ] {
            let scalar = infer_text(text).unwrap();
            let value = to_grafeo(&scalar).unwrap();
            assert_eq!(from_grafeo(&value), Some(scalar), "{text}");
        }
        assert_eq!(
            from_grafeo(&Value::Date(GDate::from_days(-1))),
            Some(Scalar::Date(-1))
        );
    }
}
