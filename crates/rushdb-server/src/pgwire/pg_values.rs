//! PostgreSQL `interval` and `point` values, in both wire formats: the
//! text PostgreSQL itself writes (`1 year 2 mons 3 days 04:05:06.5`,
//! `(1.5,2)`) and its binary layout.

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
