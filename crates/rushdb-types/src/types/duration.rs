//! The `duration` type: months, days and microseconds, kept apart as ISO
//! 8601 and Neo4j keep them (`P1M` is not `P30D`). Literals are ISO 8601
//! durations: `P1Y2M3DT4H5M6.5S`, `P2W`, `PT0S`, with an optional leading
//! `-` and signed components (`PT-1.5S`).

use super::temporal::{MICROS_PER_DAY, MICROS_PER_SECOND};

/// A duration. Equality compares the components; order is by
/// [`Duration::normalized`] (a month counts as 30 days), components breaking
/// ties, so a sort is total.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Duration {
    /// Months (a year is twelve).
    pub months: i32,
    /// Days (a week is seven).
    pub days: i32,
    /// Microseconds (hours, minutes and seconds).
    pub micros: i64,
}

impl Duration {
    /// The zero duration.
    pub const ZERO: Self = Self {
        months: 0,
        days: 0,
        micros: 0,
    };

    /// The duration's length in microseconds with a month as 30 days: the
    /// order of durations.
    #[must_use]
    pub fn normalized(self) -> i128 {
        (i128::from(self.months) * 30 + i128::from(self.days)) * i128::from(MICROS_PER_DAY)
            + i128::from(self.micros)
    }

    /// The same components negated, `None` on overflow.
    #[must_use]
    pub fn checked_neg(self) -> Option<Self> {
        Some(Self {
            months: self.months.checked_neg()?,
            days: self.days.checked_neg()?,
            micros: self.micros.checked_neg()?,
        })
    }
}

impl PartialOrd for Duration {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Duration {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.normalized(), self.months, self.days, self.micros).cmp(&(
            other.normalized(),
            other.months,
            other.days,
            other.micros,
        ))
    }
}

/// Parse a signed decimal number of `unit` microseconds (`1.5` hours):
/// fractions finer than a microsecond are truncated.
fn decimal_micros(text: &str, unit: i64) -> Option<i64> {
    let (negative, text) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (whole, fraction) = text.split_once(['.', ',']).unwrap_or((text, ""));
    if whole.is_empty() && fraction.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let mut micros = if whole.is_empty() {
        0
    } else {
        whole.parse::<i64>().ok()?.checked_mul(unit)?
    };
    // The fraction of a unit, digit by digit, to the microsecond.
    let mut scale = unit;
    for digit in fraction.bytes() {
        scale /= 10;
        if scale == 0 {
            break;
        }
        micros = micros.checked_add(i64::from(digit - b'0') * scale)?;
    }
    Some(if negative { -micros } else { micros })
}

/// A signed integer component (`-3`); `None` for a fraction.
fn whole(text: &str) -> Option<i32> {
    let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

/// Parse an ISO 8601 duration.
#[must_use]
pub fn parse_duration(value: &str) -> Option<Duration> {
    let value = value.trim();
    let (negative, value) = match value.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, value),
    };
    let body = value.strip_prefix(['P', 'p'])?;
    let (date, time) = match body.split_once(['T', 't']) {
        Some((date, time)) if !time.is_empty() => (date, Some(time)),
        Some(_) => return None,
        None => (body, None),
    };
    if date.is_empty() && time.is_none() {
        return None;
    }
    let mut out = Duration::ZERO;
    let mut number = String::new();
    for c in date.chars() {
        if c.is_ascii_digit() || matches!(c, '-' | '+') {
            number.push(c);
            continue;
        }
        let n = whole(&number)?;
        number.clear();
        match c.to_ascii_uppercase() {
            'Y' => out.months = out.months.checked_add(n.checked_mul(12)?)?,
            'M' => out.months = out.months.checked_add(n)?,
            'W' => out.days = out.days.checked_add(n.checked_mul(7)?)?,
            'D' => out.days = out.days.checked_add(n)?,
            _ => return None,
        }
    }
    if !number.is_empty() {
        return None;
    }
    for c in time.unwrap_or_default().chars() {
        if c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | ',') {
            number.push(c);
            continue;
        }
        let unit = match c.to_ascii_uppercase() {
            'H' => 3_600 * MICROS_PER_SECOND,
            'M' => 60 * MICROS_PER_SECOND,
            'S' => MICROS_PER_SECOND,
            _ => return None,
        };
        out.micros = out.micros.checked_add(decimal_micros(&number, unit)?)?;
        number.clear();
    }
    if !number.is_empty() {
        return None;
    }
    if negative {
        out = out.checked_neg()?;
    }
    Some(out)
}

/// Format a duration as ISO 8601 with signed components, as Neo4j does:
/// years and months, days, then hours, minutes and seconds; `PT0S` for
/// zero.
#[must_use]
pub fn format_duration(duration: Duration) -> String {
    use std::fmt::Write as _;
    if duration == Duration::ZERO {
        return "PT0S".to_owned();
    }
    let mut out = String::from("P");
    let (years, months) = (duration.months / 12, duration.months % 12);
    for (value, unit) in [(years, 'Y'), (months, 'M'), (duration.days, 'D')] {
        if value != 0 {
            let _ = write!(out, "{value}{unit}");
        }
    }
    if duration.micros != 0 {
        out.push('T');
        let micros = duration.micros;
        let hours = micros / (3_600 * MICROS_PER_SECOND);
        let minutes = micros / (60 * MICROS_PER_SECOND) % 60;
        let seconds = micros % (60 * MICROS_PER_SECOND);
        if hours != 0 {
            let _ = write!(out, "{hours}H");
        }
        if minutes != 0 {
            let _ = write!(out, "{minutes}M");
        }
        if seconds != 0 {
            let sign = if seconds < 0 { "-" } else { "" };
            let seconds = seconds.unsigned_abs();
            let whole = seconds / MICROS_PER_SECOND.unsigned_abs();
            let sub = seconds % MICROS_PER_SECOND.unsigned_abs();
            if sub == 0 {
                let _ = write!(out, "{sign}{whole}S");
            } else {
                let digits = format!("{sub:06}");
                let _ = write!(out, "{sign}{whole}.{}S", digits.trim_end_matches('0'));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_durations_parse_into_components() {
        let parsed = parse_duration("P1Y2M3W4DT5H6M7.5S").unwrap();
        assert_eq!(
            parsed,
            Duration {
                months: 14,
                days: 25,
                micros: (5 * 3_600 + 6 * 60 + 7) * MICROS_PER_SECOND + 500_000,
            }
        );
        assert_eq!(
            parse_duration("-P1D"),
            Some(Duration {
                days: -1,
                ..Duration::ZERO
            })
        );
        assert_eq!(
            parse_duration("PT-1.5S"),
            Some(Duration {
                micros: -1_500_000,
                ..Duration::ZERO
            })
        );
        assert_eq!(
            parse_duration("PT0.0000015S"),
            Some(Duration {
                micros: 1,
                ..Duration::ZERO
            })
        );
        for bad in ["P", "PT", "1D", "P1.5D", "P1DT", "PXD", "P1H"] {
            assert!(parse_duration(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn durations_round_trip_and_order_by_length() {
        for text in [
            "PT0S",
            "P1Y2M3DT4H5M6.5S",
            "P-1D",
            "PT-1.000001S",
            "P14DT16H12M",
        ] {
            let parsed = parse_duration(text).unwrap();
            assert_eq!(
                parse_duration(&format_duration(parsed)),
                Some(parsed),
                "{text}"
            );
        }
        let month = parse_duration("P1M").unwrap();
        let thirty_days = parse_duration("P30D").unwrap();
        assert_ne!(month, thirty_days);
        assert_eq!(month.normalized(), thirty_days.normalized());
        assert!(thirty_days < month, "ties break on the components");
        assert!(parse_duration("PT1H").unwrap() < parse_duration("P1D").unwrap());
    }
}
