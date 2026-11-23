//! PostgreSQL `interval` and `point` values, in both wire formats: the
//! text PostgreSQL itself writes (`1 year 2 mons 3 days 04:05:06.5`,
//! `(1.5,2)`) and its binary layout; and times as PostgreSQL prints them.

use std::error::Error;

use bytes::{BufMut, BytesMut};
use pgwire::api::Type;
use pgwire::types::format::FormatOptions;
use pgwire::types::ToSqlText;
use postgres_types::{to_sql_checked, IsNull, ToSql};

type BoxError = Box<dyn Error + Sync + Send>;

/// An `interval`: months, days and microseconds, kept apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PgInterval {
    pub(super) months: i32,
    pub(super) days: i32,
    pub(super) micros: i64,
}

impl PgInterval {
    /// The interval of an ISO 8601 duration.
    pub(super) fn parse(text: &str) -> Option<Self> {
        let duration = rushdb_types::types::duration::parse_duration(text)?;
        Some(Self {
            months: duration.months,
            days: duration.days,
            micros: duration.micros,
        })
    }

    /// The interval of its binary form: microseconds, days, months.
    pub(super) fn from_binary(bytes: &[u8]) -> Option<Self> {
        let micros = i64::from_be_bytes(bytes.get(0..8)?.try_into().ok()?);
        let days = i32::from_be_bytes(bytes.get(8..12)?.try_into().ok()?);
        let months = i32::from_be_bytes(bytes.get(12..16)?.try_into().ok()?);
        (bytes.len() == 16).then_some(Self {
            months,
            days,
            micros,
        })
    }

    /// The interval as an ISO 8601 duration.
    pub(super) fn iso(self) -> String {
        rushdb_types::types::duration::format_duration(rushdb_types::types::duration::Duration {
            months: self.months,
            days: self.days,
            micros: self.micros,
        })
    }

    /// PostgreSQL's default (`postgres`) interval style.
    fn text(self) -> String {
        let mut parts = Vec::new();
        let plural =
            |n: i64, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        let (years, months) = (self.months / 12, self.months % 12);
        if years != 0 {
            parts.push(plural(i64::from(years), "year", "years"));
        }
        if months != 0 {
            parts.push(plural(i64::from(months), "mon", "mons"));
        }
        if self.days != 0 {
            parts.push(plural(i64::from(self.days), "day", "days"));
        }
        if self.micros != 0 || parts.is_empty() {
            let sign = if self.micros < 0 { "-" } else { "" };
            let micros = self.micros.unsigned_abs();
            let seconds = micros / 1_000_000;
            let fraction = micros % 1_000_000;
            let mut time = format!(
                "{sign}{:02}:{:02}:{:02}",
                seconds / 3_600,
                seconds / 60 % 60,
                seconds % 60
            );
            if fraction != 0 {
                let digits = format!("{fraction:06}");
                time.push('.');
                time.push_str(digits.trim_end_matches('0'));
            }
            parts.push(time);
        }
        parts.join(" ")
    }
}

impl ToSql for PgInterval {
    fn to_sql(&self, _: &Type, out: &mut BytesMut) -> Result<IsNull, BoxError> {
        out.put_i64(self.micros);
        out.put_i32(self.days);
        out.put_i32(self.months);
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::INTERVAL
    }

    to_sql_checked!();
}

impl ToSqlText for PgInterval {
    fn to_sql_text(
        &self,
        _: &Type,
        out: &mut BytesMut,
        _: &FormatOptions,
    ) -> Result<IsNull, BoxError> {
        out.put_slice(self.text().as_bytes());
        Ok(IsNull::No)
    }
}

/// A `time`, `timestamp` or `timestamptz` printed as PostgreSQL prints it:
/// fractional seconds without trailing zeros, none when whole
/// (`19:00:00+01`, `19:00:00.5+01`). The pgwire crate's text (for every
/// `DateStyle`) always has six digits; the binary form is its own.
#[derive(Debug)]
pub(super) struct PgTime<T>(pub(super) T);

impl<T: ToSql> ToSql for PgTime<T> {
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, BoxError> {
        self.0.to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        T::accepts(ty)
    }

    to_sql_checked!();
}

impl<T: ToSqlText> ToSqlText for PgTime<T> {
    fn to_sql_text(
        &self,
        ty: &Type,
        out: &mut BytesMut,
        options: &FormatOptions,
    ) -> Result<IsNull, BoxError> {
        let mut text = BytesMut::new();
        let null = self.0.to_sql_text(ty, &mut text, options)?;
        out.put_slice(&trim_fraction(&text));
        Ok(null)
    }
}

/// `…:SS.ffffff…` with the fraction's trailing zeros dropped, and the dot
/// when nothing is left. Only a fraction right after `:SS` is touched (a
/// German date's `25.12.` is not).
fn trim_fraction(text: &[u8]) -> Vec<u8> {
    let digit = |at: usize| text.get(at).is_some_and(u8::is_ascii_digit);
    let mut out = Vec::with_capacity(text.len());
    let mut at = 0;
    while at < text.len() {
        let fraction = at >= 3
            && text[at] == b'.'
            && text[at - 3] == b':'
            && digit(at - 2)
            && digit(at - 1)
            && digit(at + 1);
        if !fraction {
            out.push(text[at]);
            at += 1;
            continue;
        }
        let end = (at + 1..=text.len())
            .find(|&end| !digit(end))
            .unwrap_or(text.len());
        let kept = text[at + 1..end]
            .iter()
            .rposition(|&d| d != b'0')
            .map_or(0, |last| last + 1);
        if kept > 0 {
            out.extend_from_slice(&text[at..=at + kept]);
        }
        at = end;
    }
    out
}

/// A `point`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PgPoint {
    pub(super) x: f64,
    pub(super) y: f64,
}

impl PgPoint {
    /// The point of PostgreSQL point text, `(x,y)`.
    pub(super) fn parse(text: &str) -> Option<Self> {
        let (x, y) = text
            .trim()
            .strip_prefix('(')?
            .strip_suffix(')')?
            .split_once(',')?;
        Some(Self {
            x: x.trim().parse().ok()?,
            y: y.trim().parse().ok()?,
        })
    }
}

impl ToSql for PgPoint {
    fn to_sql(&self, _: &Type, out: &mut BytesMut) -> Result<IsNull, BoxError> {
        out.put_f64(self.x);
        out.put_f64(self.y);
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::POINT
    }

    to_sql_checked!();
}

impl ToSqlText for PgPoint {
    fn to_sql_text(
        &self,
        _: &Type,
        out: &mut BytesMut,
        _: &FormatOptions,
    ) -> Result<IsNull, BoxError> {
        out.put_slice(format!("({},{})", self.x, self.y).as_bytes());
        Ok(IsNull::No)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_print_their_fraction_as_postgresql_does() {
        for (crate_text, postgres) in [
            ("19:00:00.000000+01", "19:00:00+01"),
            ("19:00:00.500000+01", "19:00:00.5+01"),
            ("2026-11-23 19:00:00.000120", "2026-11-23 19:00:00.00012"),
            ("23.11.2026 19:00:05.000000", "23.11.2026 19:00:05"),
            (
                "Mon Nov 23 19:00:00.250000 2026",
                "Mon Nov 23 19:00:00.25 2026",
            ),
            ("12:00:00", "12:00:00"),
        ] {
            assert_eq!(
                String::from_utf8(trim_fraction(crate_text.as_bytes())).unwrap(),
                postgres
            );
        }
    }

    #[test]
    fn intervals_read_as_postgresql_writes_them() {
        for (iso, text) in [
            ("P1Y2M3DT4H5M6.5S", "1 year 2 mons 3 days 04:05:06.5"),
            ("PT0S", "00:00:00"),
            ("P-1D", "-1 days"),
            ("PT-1M", "-00:01:00"),
            ("P14M", "1 year 2 mons"),
        ] {
            assert_eq!(PgInterval::parse(iso).unwrap().text(), text, "{iso}");
        }
        assert_eq!(PgPoint::parse("(1.5,2)"), Some(PgPoint { x: 1.5, y: 2.0 }));
        let interval = PgInterval::parse("P1M2DT3S").unwrap();
        let mut bytes = BytesMut::new();
        interval.to_sql(&Type::INTERVAL, &mut bytes).unwrap();
        assert_eq!(PgInterval::from_binary(&bytes), Some(interval));
        assert_eq!(interval.iso(), "P1M2DT3S");
    }
}
