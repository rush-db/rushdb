//! The temporal family: `date`, `localtime`, `localdatetime` and `datetime`
//! (an instant with the UTC offset it was written with). Values are kept in
//! microseconds; nanosecond digits of a literal are truncated.
//!
//! Literals are ISO 8601: `2024-05-01`, `10:15:00.5`, `2024-05-01T10:15:00`,
//! `2024-05-01T10:15:00+02:00` (`Z` for UTC). A year may be signed
//! (`-0032-01-01`, astronomical numbering: year 0 is 1 BC).

use chrono::{
    DateTime as ChronoDateTime, Datelike, Duration as ChronoDuration, FixedOffset, NaiveDate,
    NaiveDateTime, NaiveTime, Timelike,
};

/// Microseconds in a second.
pub const MICROS_PER_SECOND: i64 = 1_000_000;
/// Microseconds in a day.
pub const MICROS_PER_DAY: i64 = 86_400 * MICROS_PER_SECOND;
/// The largest UTC offset a `datetime` keeps, in seconds (±18:00).
pub const MAX_OFFSET_SECONDS: i32 = 18 * 3_600;

/// An instant: UTC epoch microseconds, and the UTC offset (seconds east of
/// UTC) it was written with. Two instants are equal when their microseconds
/// are: the offset is kept for output only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Instant {
    /// Microseconds since 1970-01-01T00:00:00Z.
    pub micros: i64,
    /// Seconds east of UTC the instant was written with.
    pub offset: i32,
}

impl Instant {
    /// The instant at `micros` in UTC.
    #[must_use]
    pub const fn utc(micros: i64) -> Self {
        Self { micros, offset: 0 }
    }

    /// The instant of epoch milliseconds `ms` in UTC (the 3.0.13 storage
    /// form), `None` past the microsecond range.
    #[must_use]
    pub fn from_millis(ms: i64) -> Option<Self> {
        ms.checked_mul(1_000).map(Self::utc)
    }

    /// The instant's wall clock at its own offset, in microseconds.
    #[must_use]
    pub fn local_micros(self) -> Option<i64> {
        self.micros
            .checked_add(i64::from(self.offset) * MICROS_PER_SECOND)
    }
}

fn utc_naive(micros: i64) -> Option<NaiveDateTime> {
    ChronoDateTime::from_timestamp_micros(micros).map(|datetime| datetime.naive_utc())
}

fn naive_micros(datetime: NaiveDateTime) -> Option<i64> {
    let micros = datetime.and_utc().timestamp_micros();
    // Chrono's range is wider than i64 microseconds only in nanoseconds; a
    // round trip guards the edges.
    (utc_naive(micros)? == datetime.with_nanosecond(datetime.nanosecond() / 1_000 * 1_000)?)
        .then_some(micros)
}

/// The days since 1970-01-01 of a calendar date.
fn date_days(date: NaiveDate) -> Option<i32> {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1)?;
    i32::try_from(date.signed_duration_since(epoch).num_days()).ok()
}

/// The calendar date `days` after 1970-01-01.
#[must_use]
pub fn date_of_days(days: i32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(1970, 1, 1)?.checked_add_signed(ChronoDuration::days(days.into()))
}

/// Split a literal into its year sign and the rest: Chrono parses a year
/// with more or fewer than four digits only with an explicit sign.
fn signed_year(value: &str) -> bool {
    let Some((sign, rest)) = value.split_at_checked(1) else {
        return false;
    };
    let digits = rest.split('-').next().unwrap_or_default();
    matches!(sign, "-" | "+") && digits.len() >= 4 && digits.bytes().all(|b| b.is_ascii_digit())
}

/// Truncate fractional seconds past microseconds, so `.123456789` reads as
/// `.123456`: Chrono rounds nothing, it keeps nanoseconds, which the
/// storage form does not.
fn truncate_nanos(time: NaiveTime) -> NaiveTime {
    time.with_nanosecond(time.nanosecond() / 1_000 * 1_000)
        .unwrap_or(time)
}

/// Parse an ISO 8601 calendar date (`2024-05-01`, `-0032-01-01`) into days
/// since 1970-01-01.
#[must_use]
pub fn parse_date(value: &str) -> Option<i32> {
    let value = value.trim();
    if value.contains('T') || value.contains(':') {
        return None;
    }
    let _ = signed_year(value);
    date_days(NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()?)
}

/// Parse an ISO 8601 time of day without an offset (`10:15`, `10:15:00`,
/// `10:15:00.5`) into microseconds since midnight.
#[must_use]
pub fn parse_local_time(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.ends_with(['Z', 'z']) || value.contains(['+', 'T']) || value.matches('-').count() > 0 {
        return None;
    }
    let time = ["%H:%M:%S%.f", "%H:%M"]
        .iter()
        .find_map(|layout| NaiveTime::parse_from_str(value, layout).ok())?;
    let time = truncate_nanos(time);
    Some(
        i64::from(time.num_seconds_from_midnight()) * MICROS_PER_SECOND
            + i64::from(time.nanosecond() / 1_000),
    )
}

/// Parse an ISO 8601 date and time without an offset
/// (`2024-05-01T10:15:00`, `2024-05-01T10:15`) into wall-clock microseconds
/// since 1970-01-01T00:00.
#[must_use]
pub fn parse_local_datetime(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.ends_with(['Z', 'z']) || offset_of(value).is_some() {
        return None;
    }
    let datetime = ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M"]
        .iter()
        .find_map(|layout| NaiveDateTime::parse_from_str(value, layout).ok())?;
    naive_micros(datetime.date().and_time(truncate_nanos(datetime.time())))
}

/// The `±HH:MM` (or `±HHMM`, `±HH`) offset suffix of a date-time literal,
/// in seconds: `None` without one.
fn offset_of(value: &str) -> Option<(usize, i32)> {
    let time_start = value.find(['T', 't'])?;
    let time = &value[time_start..];
    let at = time.rfind(['+', '-'])?;
    let suffix = &time[at + 1..];
    let sign = if time.as_bytes()[at] == b'-' { -1 } else { 1 };
    let digits: String = suffix.chars().filter(char::is_ascii_digit).collect();
    if digits.len() != suffix.len() - suffix.matches(':').count() {
        return None;
    }
    let (hours, minutes) = match digits.len() {
        2 => (digits.parse::<i32>().ok()?, 0),
        4 => (
            digits[..2].parse::<i32>().ok()?,
            digits[2..].parse::<i32>().ok()?,
        ),
        _ => return None,
    };
    if minutes >= 60 {
        return None;
    }
    let seconds = sign * (hours * 3_600 + minutes * 60);
    (seconds.abs() <= MAX_OFFSET_SECONDS).then_some((time_start + at, seconds))
}

/// Parse an ISO 8601 instant: a date and time with `Z` or a UTC offset
/// (`2024-05-01T10:15:00Z`, `2024-05-01T10:15:00.5+02:00`). As RFC 3339
/// allows, the separator may also be a lowercase `t` or a space, and `Z` a
/// lowercase `z`. A zone id (`[Europe/Berlin]`) is refused: named time
/// zones are not supported.
#[must_use]
pub fn parse_instant(value: &str) -> Option<Instant> {
    let value = value.trim();
    // A date holds digits and `-` only: its first `T`, `t` or space is the
    // separator.
    let normalized;
    let value = match value.find(['t', ' ']) {
        Some(at) if value.find('T').is_none_or(|upper| at < upper) => {
            normalized = format!("{}T{}", &value[..at], &value[at + 1..]);
            normalized.as_str()
        }
        _ => value,
    };
    let (local, offset) = if let Some(local) = value.strip_suffix(['Z', 'z']) {
        (local, 0)
    } else {
        let (at, offset) = offset_of(value)?;
        (&value[..at], offset)
    };
    let local = parse_local_datetime(local)?;
    let micros = local.checked_sub(i64::from(offset) * MICROS_PER_SECOND)?;
    Some(Instant { micros, offset })
}

/// Format days since 1970-01-01 as an ISO 8601 date.
#[must_use]
pub fn format_date(days: i32) -> String {
    date_of_days(days).map_or_else(|| days.to_string(), format_naive_date)
}

fn format_naive_date(date: NaiveDate) -> String {
    // `%Y` signs a year before 0000 or after 9999.
    date.format("%Y-%m-%d").to_string()
}

/// The fraction of a second of `micros`, in its shortest digits: empty
/// for a whole second.
fn fraction(micros: i64) -> String {
    let sub = micros.rem_euclid(MICROS_PER_SECOND);
    if sub == 0 {
        return String::new();
    }
    let digits = format!("{sub:06}");
    format!(".{}", digits.trim_end_matches('0'))
}

/// Format microseconds since midnight as an ISO 8601 time of day.
#[must_use]
pub fn format_local_time(micros: i64) -> String {
    let seconds = micros.div_euclid(MICROS_PER_SECOND);
    format!(
        "{:02}:{:02}:{:02}{}",
        seconds / 3_600,
        seconds / 60 % 60,
        seconds % 60,
        fraction(micros)
    )
}

/// Format wall-clock microseconds as an ISO 8601 local date-time.
#[must_use]
pub fn format_local_datetime(micros: i64) -> String {
    let date = micros.div_euclid(MICROS_PER_DAY);
    let time = micros.rem_euclid(MICROS_PER_DAY);
    let date = i32::try_from(date).map_or_else(|_| date.to_string(), format_date);
    format!("{date}T{}", format_local_time(time))
}

/// Format an instant at its own offset: `2024-05-01T10:15:00+02:00`, `Z`
/// for UTC, with the shortest fraction of a second that keeps its
/// microseconds (none for a whole second).
#[must_use]
pub fn format_instant(instant: Instant) -> String {
    let Some(local) = instant.local_micros() else {
        return instant.micros.to_string();
    };
    let date = local.div_euclid(MICROS_PER_DAY);
    let time = local.rem_euclid(MICROS_PER_DAY);
    let seconds = time / MICROS_PER_SECOND;
    let date = i32::try_from(date).map_or_else(|_| date.to_string(), format_date);
    let offset = if instant.offset == 0 {
        "Z".to_owned()
    } else {
        let sign = if instant.offset < 0 { '-' } else { '+' };
        let minutes = instant.offset.unsigned_abs() / 60;
        format!("{sign}{:02}:{:02}", minutes / 60, minutes % 60)
    };
    format!(
        "{date}T{:02}:{:02}:{:02}{}{offset}",
        seconds / 3_600,
        seconds / 60 % 60,
        seconds % 60,
        fraction(time)
    )
}

/// A calendar component, as the `$year`…`$microsecond` predicates name
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    /// The year (astronomical numbering).
    Year,
    /// The month, 1–12.
    Month,
    /// The day of the month, 1–31.
    Day,
    /// The hour, 0–23.
    Hour,
    /// The minute, 0–59.
    Minute,
    /// The second, 0–59.
    Second,
    /// The millisecond of the second, 0–999.
    Millisecond,
    /// The microsecond of the second, 0–999,999.
    Microsecond,
}

impl Component {
    /// The component a predicate index names (`0` year … `7` microsecond).
    #[must_use]
    pub const fn of_index(which: u8) -> Option<Self> {
        Some(match which {
            0 => Self::Year,
            1 => Self::Month,
            2 => Self::Day,
            3 => Self::Hour,
            4 => Self::Minute,
            5 => Self::Second,
            6 => Self::Millisecond,
            7 => Self::Microsecond,
            _ => return None,
        })
    }

    /// Whether the component is part of a date (year, month, day).
    #[must_use]
    pub const fn of_date(self) -> bool {
        matches!(self, Self::Year | Self::Month | Self::Day)
    }
}

/// One component of a calendar date-time given as wall-clock microseconds
/// since 1970-01-01T00:00 (for an instant: its microseconds in UTC).
#[must_use]
pub fn component_of_micros(micros: i64, component: Component) -> Option<i64> {
    let datetime = utc_naive(micros)?;
    let sub = micros.rem_euclid(MICROS_PER_SECOND);
    Some(match component {
        Component::Year => i64::from(datetime.year()),
        Component::Month => i64::from(datetime.month()),
        Component::Day => i64::from(datetime.day()),
        Component::Hour => i64::from(datetime.hour()),
        Component::Minute => i64::from(datetime.minute()),
        Component::Second => i64::from(datetime.second()),
        Component::Millisecond => sub / 1_000,
        Component::Microsecond => sub,
    })
}

/// Wall-clock microseconds of calendar components, `None` for an invalid
/// date or time.
#[must_use]
pub fn micros_of_components(
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    micros: i64,
) -> Option<i64> {
    let date = NaiveDate::from_ymd_opt(
        i32::try_from(year).ok()?,
        u32::try_from(month).ok()?,
        u32::try_from(day).ok()?,
    )?;
    let time = NaiveTime::from_hms_micro_opt(
        u32::try_from(hour).ok()?,
        u32::try_from(minute).ok()?,
        u32::try_from(second).ok()?,
        u32::try_from(micros).ok()?,
    )?;
    naive_micros(date.and_time(time))
}

/// A calendar period wall-clock microseconds are truncated to
/// (`$timeBucket`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    /// A day.
    Day,
    /// A month.
    Month,
    /// A quarter (months 1, 4, 7, 10).
    Quarter,
    /// A year.
    Year,
}

/// The first microsecond of the `period` holding wall-clock `micros`.
#[must_use]
pub fn start_of(micros: i64, period: Period) -> Option<i64> {
    let datetime = utc_naive(micros)?;
    let (year, month) = (datetime.year(), datetime.month());
    let date = match period {
        Period::Day => datetime.date(),
        Period::Month => NaiveDate::from_ymd_opt(year, month, 1)?,
        Period::Quarter => NaiveDate::from_ymd_opt(year, (month - 1) / 3 * 3 + 1, 1)?,
        Period::Year => NaiveDate::from_ymd_opt(year, 1, 1)?,
    };
    naive_micros(date.and_time(NaiveTime::MIN))
}

/// A fixed offset Chrono understands, for conversions at the edges.
#[must_use]
pub fn fixed_offset(seconds: i32) -> Option<FixedOffset> {
    FixedOffset::east_opt(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instants_take_the_separators_and_zone_letter_rfc_3339_allows() {
        let expected = parse_instant("2024-05-01T10:30:00Z").unwrap();
        for text in [
            "2024-05-01t10:30:00z",
            "2024-05-01T10:30:00z",
            "2024-05-01 10:30:00Z",
            "2024-05-01t12:30:00+02:00",
        ] {
            assert_eq!(
                parse_instant(text).map(|instant| instant.micros),
                Some(expected.micros),
                "{text}"
            );
        }
        assert!(parse_instant("2024-05-01T10:30:00[Europe/Berlin]").is_none());
    }

    #[test]
    fn literals_parse_by_form() {
        assert_eq!(parse_date("1970-01-02"), Some(1));
        assert_eq!(parse_date("1969-12-31"), Some(-1));
        assert!(parse_date("2024-02-30").is_none());
        assert!(parse_date("2024-05-01T00:00").is_none());
        assert_eq!(parse_local_time("00:00:01.5"), Some(1_500_000));
        assert_eq!(parse_local_time("10:15"), Some(36_900_000_000));
        assert!(parse_local_time("10:15Z").is_none());
        assert_eq!(parse_local_datetime("1970-01-01T00:00:00.000001"), Some(1));
        assert!(parse_local_datetime("1970-01-01T00:00:00Z").is_none());
        assert!(parse_local_datetime("1970-01-01T00:00:00+01:00").is_none());
        let instant = parse_instant("1970-01-01T01:00:00+01:00").unwrap();
        assert_eq!(
            instant,
            Instant {
                micros: 0,
                offset: 3_600
            }
        );
        assert_eq!(parse_instant("1970-01-01T00:00:00z"), Some(Instant::utc(0)));
        assert_eq!(
            parse_instant("1970-01-01T00:00:00.123456789Z"),
            Some(Instant::utc(123_456))
        );
        assert!(parse_instant("1970-01-01T00:00:00").is_none());
        assert!(parse_instant("1970-01-01T00:00:00+19:00").is_none());
        assert!(parse_instant("1970-01-01T00:00:00[Europe/Berlin]").is_none());
        assert_eq!(
            parse_instant("-0032-01-01T08:00:00Z").map(format_instant),
            Some("-0032-01-01T08:00:00Z".to_owned())
        );
    }

    #[test]
    fn literals_round_trip_through_their_canonical_form() {
        for (instant, text) in [
            (Instant::utc(0), "1970-01-01T00:00:00Z"),
            (Instant::utc(1), "1970-01-01T00:00:00.000001Z"),
            (
                Instant {
                    micros: 0,
                    offset: -5_400,
                },
                "1969-12-31T22:30:00-01:30",
            ),
        ] {
            assert_eq!(format_instant(instant), text);
            assert_eq!(parse_instant(text), Some(instant));
        }
        assert_eq!(format_local_time(1_500_000), "00:00:01.5");
        assert_eq!(format_local_time(0), "00:00:00");
        assert_eq!(format_local_datetime(-1), "1969-12-31T23:59:59.999999");
        assert_eq!(format_date(-719_528), "0000-01-01");
        for days in [-1_000_000, -1, 0, 1, 20_000, 2_000_000] {
            assert_eq!(parse_date(&format_date(days)), Some(days));
        }
    }

    #[test]
    fn components_and_periods_read_the_wall_clock() {
        let micros = parse_local_datetime("2024-05-17T10:15:30.250001").unwrap();
        assert_eq!(component_of_micros(micros, Component::Year), Some(2024));
        assert_eq!(
            component_of_micros(micros, Component::Millisecond),
            Some(250)
        );
        assert_eq!(
            component_of_micros(micros, Component::Microsecond),
            Some(250_001)
        );
        assert_eq!(
            start_of(micros, Period::Quarter).map(format_local_datetime),
            Some("2024-04-01T00:00:00".to_owned())
        );
        assert_eq!(
            micros_of_components(2024, 5, 17, 10, 15, 30, 250_001),
            Some(micros)
        );
    }
}
