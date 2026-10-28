//! Session settings that bound a client's own statements.
//!
//! The names are PostgreSQL's and `CockroachDB`'s, so drivers and tools that
//! already set them work unchanged: `statement_timeout`, `work_mem` (here
//! the working-memory cap of the whole statement), and `CockroachDB`'s
//! `disallow_full_table_scans` with `large_full_scan_rows`. Each may only
//! tighten the server's ceilings. A value is set with `SET`, cleared with
//! `RESET` or `SET … = DEFAULT`, read with `SHOW`, or passed as a startup
//! parameter; it lives in the connection's metadata.

use std::collections::HashMap;

use pgwire::error::PgWireResult;
use serde_json::{Map, Value};

use super::err;

/// One setting this module answers for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Setting {
    StatementTimeout,
    WorkMem,
    DisallowFullTableScans,
    LargeFullScanRows,
}

const SETTINGS: [Setting; 4] = [
    Setting::StatementTimeout,
    Setting::WorkMem,
    Setting::DisallowFullTableScans,
    Setting::LargeFullScanRows,
];

impl Setting {
    pub(super) fn name(self) -> &'static str {
        match self {
            Setting::StatementTimeout => "statement_timeout",
            Setting::WorkMem => "work_mem",
            Setting::DisallowFullTableScans => "disallow_full_table_scans",
            Setting::LargeFullScanRows => "large_full_scan_rows",
        }
    }

    fn of(name: &str) -> Option<Self> {
        SETTINGS
            .into_iter()
            .find(|setting| setting.name().eq_ignore_ascii_case(name))
    }

    /// What `SHOW` answers when the session has not set it: no bound of
    /// its own (the server's ceilings still apply).
    fn unset(self) -> &'static str {
        match self {
            Setting::StatementTimeout | Setting::LargeFullScanRows => "0",
            Setting::WorkMem => "server",
            Setting::DisallowFullTableScans => "off",
        }
    }

    /// The canonical form of `value`, or why it is not one.
    fn normalize(self, value: &str) -> Result<String, String> {
        match self {
            Setting::StatementTimeout => duration_ms(value).map(|ms| format!("{ms}ms")),
            Setting::WorkMem => bytes(value).map(|bytes| format!("{bytes}B")),
            Setting::DisallowFullTableScans => {
                boolean(value).map(|on| if on { "on" } else { "off" }.to_owned())
            }
            Setting::LargeFullScanRows => value
                .parse::<u64>()
                .map(|rows| rows.to_string())
                .map_err(|_| "expected a whole number of rows".to_owned()),
        }
    }
}

/// A settings statement.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Command {
    /// `SET name = value`, or `None` for `RESET name` / `SET name = DEFAULT`.
    Set(Setting, Option<String>),
    /// `RESET ALL`.
    ResetAll,
    Show(Setting),
}

/// The settings statement `sql` is, if it names one of these settings.
pub(super) fn parse(sql: &str) -> Option<Command> {
    let sql = sql.trim().trim_end_matches(';').trim();
    let mut words = sql.splitn(2, char::is_whitespace);
    let verb = words.next()?.to_ascii_uppercase();
    let rest = words.next().unwrap_or_default().trim();
    match verb.as_str() {
        "SHOW" => Setting::of(rest).map(Command::Show),
        "RESET" if rest.eq_ignore_ascii_case("all") => Some(Command::ResetAll),
        "RESET" => Setting::of(rest).map(|setting| Command::Set(setting, None)),
        "SET" => {
            let rest = strip_word(rest, "SESSION").unwrap_or(rest);
            let (name, value) = rest
                .split_once('=')
                .or_else(|| split_word(rest, "TO"))
                .map(|(name, value)| (name.trim(), value.trim()))?;
            let setting = Setting::of(name)?;
            let value = unquote(value);
            Some(Command::Set(
                setting,
                (!value.eq_ignore_ascii_case("default")).then(|| value.to_owned()),
            ))
        }
        _ => None,
    }
}

/// Apply `command` to the session; `SHOW` answers its one row.
pub(super) fn apply(
    metadata: &mut HashMap<String, String>,
    command: Command,
) -> PgWireResult<Option<Vec<Map<String, Value>>>> {
    match command {
        Command::Set(setting, Some(value)) => {
            let value = setting
                .normalize(&value)
                .map_err(|reason| invalid(setting, &value, &reason))?;
            metadata.insert(setting.name().to_owned(), value);
        }
        Command::Set(setting, None) => {
            metadata.remove(setting.name());
        }
        Command::ResetAll => {
            for setting in SETTINGS {
                metadata.remove(setting.name());
            }
        }
        Command::Show(setting) => {
            let value = metadata
                .get(setting.name())
                .map_or_else(|| setting.unset().to_owned(), Clone::clone);
            let mut row = Map::new();
            row.insert(setting.name().to_owned(), Value::String(value));
            return Ok(Some(vec![row]));
        }
    }
    Ok(None)
}

/// The bounds a session's settings put on its statements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SessionLimits {
    /// `statement_timeout` in milliseconds (`None` = unset or `0`).
    pub(super) timeout_ms: Option<u64>,
    pub(super) query_memory_max_bytes: Option<u64>,
    pub(super) unindexed_walk_max_owners: Option<u64>,
}

/// The session's bounds. A startup parameter is read here, so a malformed
/// one fails each statement with the reason.
pub(super) fn limits(metadata: &HashMap<String, String>) -> PgWireResult<SessionLimits> {
    let read = |setting: Setting| -> PgWireResult<Option<String>> {
        metadata
            .get(setting.name())
            .map(|value| {
                setting
                    .normalize(value)
                    .map_err(|reason| invalid(setting, value, &reason))
            })
            .transpose()
    };
    let timeout_ms = read(Setting::StatementTimeout)?
        .and_then(|value| duration_ms(&value).ok())
        .filter(|ms| *ms != 0);
    let query_memory_max_bytes = read(Setting::WorkMem)?.and_then(|value| bytes(&value).ok());
    let disallow = read(Setting::DisallowFullTableScans)?.is_some_and(|value| value == "on");
    let rows = read(Setting::LargeFullScanRows)?
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    Ok(SessionLimits {
        timeout_ms,
        query_memory_max_bytes,
        unindexed_walk_max_owners: disallow.then_some(rows),
    })
}

fn invalid(setting: Setting, value: &str, reason: &str) -> pgwire::error::PgWireError {
    err(
        "22023",
        format!(
            "invalid value for parameter \"{}\": \"{value}\" ({reason})",
            setting.name()
        ),
    )
}

/// Milliseconds: a bare number is milliseconds, as in PostgreSQL; units
/// `us`, `ms`, `s`, `min`, `h`, `d`.
fn duration_ms(value: &str) -> Result<u64, String> {
    let (number, unit) = number_and_unit(value)?;
    let scale = match unit.to_ascii_lowercase().as_str() {
        "" | "ms" => 1.0,
        "us" => 0.001,
        "s" => 1_000.0,
        "min" => 60_000.0,
        "h" => 3_600_000.0,
        "d" => 86_400_000.0,
        other => return Err(format!("unknown time unit `{other}`")),
    };
    whole(number * scale)
}

/// Bytes: a bare number is kilobytes, as in PostgreSQL; units `B`, `kB`,
/// `MB`, `GB`, `TB`. Must be positive.
fn bytes(value: &str) -> Result<u64, String> {
    let (number, unit) = number_and_unit(value)?;
    let scale = match unit.to_ascii_lowercase().as_str() {
        "b" => 1.0,
        "" | "kb" => 1024.0,
        "mb" => 1024.0 * 1024.0,
        "gb" => 1024.0 * 1024.0 * 1024.0,
        "tb" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        other => return Err(format!("unknown memory unit `{other}`")),
    };
    let bytes = whole(number * scale)?;
    if bytes == 0 {
        return Err("must be positive".to_owned());
    }
    Ok(bytes)
}

fn boolean(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" => Ok(true),
        "off" | "false" | "no" | "0" => Ok(false),
        _ => Err("expected on or off".to_owned()),
    }
}

fn number_and_unit(value: &str) -> Result<(f64, &str), String> {
    let value = value.trim();
    let split = value
        .find(|character: char| !(character.is_ascii_digit() || character == '.'))
        .unwrap_or(value.len());
    let number = value[..split]
        .parse::<f64>()
        .map_err(|_| "expected a number".to_owned())?;
    Ok((number, value[split..].trim()))
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "checked finite, non-negative and under 2^64 before the cast"
)]
fn whole(value: f64) -> Result<u64, String> {
    /// 2^64, the first value a `u64` cannot hold.
    const TWO_TO_THE_64: f64 = 18_446_744_073_709_551_616.0;
    if !value.is_finite() || value < 0.0 || value >= TWO_TO_THE_64 {
        return Err("out of range".to_owned());
    }
    Ok(value.round() as u64)
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
        .unwrap_or(value)
}

/// `rest` without a leading `word` (any case) and the space after it.
fn strip_word<'a>(rest: &'a str, word: &str) -> Option<&'a str> {
    let (first, tail) = rest.split_once(char::is_whitespace)?;
    first.eq_ignore_ascii_case(word).then(|| tail.trim_start())
}

/// `rest` split around a standalone `word` (any case).
fn split_word<'a>(rest: &'a str, word: &str) -> Option<(&'a str, &'a str)> {
    let mut offset = 0;
    for part in rest.split_inclusive(char::is_whitespace) {
        if part.trim().eq_ignore_ascii_case(word) && offset > 0 {
            return Some((&rest[..offset], &rest[offset + part.len()..]));
        }
        offset += part.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(statements: &[&str]) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        for sql in statements {
            apply(&mut metadata, parse(sql).expect(sql)).unwrap();
        }
        metadata
    }

    #[test]
    fn the_postgres_and_cockroach_forms_parse() {
        assert_eq!(
            parse("SET statement_timeout = '5s'"),
            Some(Command::Set(Setting::StatementTimeout, Some("5s".into())))
        );
        assert_eq!(
            parse("set session work_mem to 64MB;"),
            Some(Command::Set(Setting::WorkMem, Some("64MB".into())))
        );
        assert_eq!(
            parse("SET disallow_full_table_scans = DEFAULT"),
            Some(Command::Set(Setting::DisallowFullTableScans, None))
        );
        assert_eq!(
            parse("RESET large_full_scan_rows"),
            Some(Command::Set(Setting::LargeFullScanRows, None))
        );
        assert_eq!(parse("RESET ALL"), Some(Command::ResetAll));
        assert_eq!(
            parse("SHOW statement_timeout"),
            Some(Command::Show(Setting::StatementTimeout))
        );
        // Other settings are not this module's.
        assert_eq!(parse("SET search_path = public"), None);
        assert_eq!(parse("SHOW server_version"), None);
    }

    #[test]
    fn settings_become_the_session_bounds() {
        let metadata = session(&[
            "SET statement_timeout = '2s'",
            "SET work_mem = '64MB'",
            "SET disallow_full_table_scans = on",
            "SET large_full_scan_rows = 1000",
        ]);
        assert_eq!(
            limits(&metadata).unwrap(),
            SessionLimits {
                timeout_ms: Some(2_000),
                query_memory_max_bytes: Some(64 << 20),
                unindexed_walk_max_owners: Some(1_000),
            }
        );
        // `disallow_full_table_scans` alone refuses every walk.
        let metadata = session(&["SET disallow_full_table_scans = true"]);
        assert_eq!(
            limits(&metadata).unwrap().unindexed_walk_max_owners,
            Some(0)
        );
        // A bare timeout is milliseconds, a bare memory size kilobytes, as
        // in PostgreSQL; `0` is no timeout of the session's own.
        let metadata = session(&["SET statement_timeout = 1500", "SET work_mem = 4096"]);
        let bounds = limits(&metadata).unwrap();
        assert_eq!(
            (bounds.timeout_ms, bounds.query_memory_max_bytes),
            (Some(1_500), Some(4 << 20))
        );
        let metadata = session(&["SET statement_timeout = 0"]);
        assert_eq!(limits(&metadata).unwrap().timeout_ms, None);
        // RESET clears.
        let metadata = session(&["SET work_mem = '1GB'", "RESET ALL"]);
        assert_eq!(limits(&metadata).unwrap(), SessionLimits::default());
    }

    #[test]
    fn show_answers_the_set_value_or_the_unset_one() {
        let mut metadata = session(&["SET statement_timeout = '1min'"]);
        let shown = |metadata: &mut HashMap<String, String>, setting| {
            apply(metadata, Command::Show(setting)).unwrap().unwrap()[0][setting.name()].clone()
        };
        assert_eq!(shown(&mut metadata, Setting::StatementTimeout), "60000ms");
        assert_eq!(shown(&mut metadata, Setting::WorkMem), "server");
        assert_eq!(shown(&mut metadata, Setting::DisallowFullTableScans), "off");
    }

    #[test]
    fn a_malformed_value_is_refused_and_leaves_the_session_as_it_was() {
        let mut metadata = session(&["SET work_mem = '8MB'"]);
        for sql in [
            "SET work_mem = 0",
            "SET work_mem = 'lots'",
            "SET statement_timeout = '5 parsecs'",
            "SET disallow_full_table_scans = maybe",
            "SET large_full_scan_rows = -1",
        ] {
            assert!(apply(&mut metadata, parse(sql).unwrap()).is_err(), "{sql}");
        }
        assert_eq!(
            limits(&metadata).unwrap().query_memory_max_bytes,
            Some(8 << 20)
        );
        // A malformed startup parameter fails the statement, not silently.
        let startup = HashMap::from([("statement_timeout".to_owned(), "soon".to_owned())]);
        assert!(limits(&startup).is_err());
    }
}
