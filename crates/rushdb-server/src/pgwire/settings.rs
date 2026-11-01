//! Session settings that bound a client's own statements.
//!
//! The names are PostgreSQL's and `CockroachDB`'s, so drivers and tools that
//! already set them work unchanged: `statement_timeout`, `work_mem` (here
//! the working-memory cap of the whole statement), and `CockroachDB`'s
//! `disallow_full_table_scans` with `large_full_scan_rows`. Each may only
//! tighten the server's ceilings. A value is set with `SET`, cleared with
//! `RESET` or `SET … = DEFAULT`, read with `SHOW`, or passed as a startup
//! parameter; it lives in the connection's metadata.
//!
//! Drivers and tools also set client parameters on every connection
//! (`application_name`, `extra_float_digits`, `DateStyle`, `TimeZone`, …).
//! Those are recorded so `SHOW` answers them, and change nothing the server
//! does; a value the server cannot honour (an encoding other than UTF8, a
//! date style other than ISO) is refused. `SET` of any other parameter is
//! refused as unsupported.

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
    // Client parameters: recorded, not acted on.
    ApplicationName,
    ClientEncoding,
    DateStyle,
    TimeZone,
    ExtraFloatDigits,
    SearchPath,
    IntervalStyle,
    StandardConformingStrings,
    ClientMinMessages,
}

const SETTINGS: [Setting; 13] = [
    Setting::StatementTimeout,
    Setting::WorkMem,
    Setting::DisallowFullTableScans,
    Setting::LargeFullScanRows,
    Setting::ApplicationName,
    Setting::ClientEncoding,
    Setting::DateStyle,
    Setting::TimeZone,
    Setting::ExtraFloatDigits,
    Setting::SearchPath,
    Setting::IntervalStyle,
    Setting::StandardConformingStrings,
    Setting::ClientMinMessages,
];

impl Setting {
    pub(super) fn name(self) -> &'static str {
        match self {
            Setting::StatementTimeout => "statement_timeout",
            Setting::WorkMem => "work_mem",
            Setting::DisallowFullTableScans => "disallow_full_table_scans",
            Setting::LargeFullScanRows => "large_full_scan_rows",
            Setting::ApplicationName => "application_name",
            Setting::ClientEncoding => "client_encoding",
            Setting::DateStyle => "DateStyle",
            Setting::TimeZone => "TimeZone",
            Setting::ExtraFloatDigits => "extra_float_digits",
            Setting::SearchPath => "search_path",
            Setting::IntervalStyle => "IntervalStyle",
            Setting::StandardConformingStrings => "standard_conforming_strings",
            Setting::ClientMinMessages => "client_min_messages",
        }
    }

    fn of(name: &str) -> Option<Self> {
        SETTINGS
            .into_iter()
            .find(|setting| setting.name().eq_ignore_ascii_case(name))
    }

    /// What `SHOW` answers when the session has not set it: no bound of
    /// its own (the server's ceilings still apply), or PostgreSQL's default
    /// for a client parameter.
    fn unset(self) -> &'static str {
        match self {
            Setting::StatementTimeout | Setting::LargeFullScanRows => "0",
            Setting::WorkMem => "server",
            Setting::DisallowFullTableScans => "off",
            Setting::ApplicationName => "",
            Setting::ClientEncoding => "UTF8",
            Setting::DateStyle => "ISO, MDY",
            Setting::TimeZone => "UTC",
            Setting::ExtraFloatDigits => "1",
            Setting::SearchPath => "\"$user\", public",
            Setting::IntervalStyle => "postgres",
            Setting::StandardConformingStrings => "on",
            Setting::ClientMinMessages => "notice",
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
            Setting::ApplicationName
            | Setting::TimeZone
            | Setting::IntervalStyle
            | Setting::ClientMinMessages => Ok(value.to_owned()),
            Setting::ClientEncoding => matches!(
                value.to_ascii_uppercase().as_str(),
                "UTF8" | "UTF-8" | "UNICODE"
            )
            .then(|| "UTF8".to_owned())
            .ok_or_else(|| "only UTF8 is offered".to_owned()),
            // Dates and timestamps are always written in ISO form.
            Setting::DateStyle => value
                .to_ascii_uppercase()
                .contains("ISO")
                .then(|| value.to_owned())
                .ok_or_else(|| "only ISO output is offered".to_owned()),
            Setting::ExtraFloatDigits => value
                .parse::<i8>()
                .ok()
                .filter(|digits| (-15..=3).contains(digits))
                .map(|digits| digits.to_string())
                .ok_or_else(|| "expected a whole number from -15 to 3".to_owned()),
            // Record tables are in `public`; a path without it would name
            // tables the session cannot see unqualified.
            Setting::SearchPath => value
                .split(',')
                .any(|schema| {
                    schema
                        .trim()
                        .trim_matches('"')
                        .eq_ignore_ascii_case("public")
                })
                .then(|| value.to_owned())
                .ok_or_else(|| {
                    "the path must include public, where the record tables are".to_owned()
                }),
            Setting::StandardConformingStrings => boolean(value)
                .ok()
                .filter(|on| *on)
                .map(|_| "on".to_owned())
                .ok_or_else(|| "only on is offered".to_owned()),
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
    /// `SET`/`RESET` of a parameter this module does not offer, named as
    /// written.
    Unsupported(String),
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
        "RESET" => Some(Setting::of(rest).map_or_else(
            || Command::Unsupported(rest.to_owned()),
            |setting| Command::Set(setting, None),
        )),
        "SET" => {
            let rest = strip_word(rest, "SESSION").unwrap_or(rest);
            // `SET TIME ZONE x` and `SET NAMES x` are PostgreSQL's spellings
            // of `TimeZone` and `client_encoding`.
            let (setting, value) = if let Some(value) =
                strip_word(rest, "TIME").and_then(|rest| strip_word(rest, "ZONE"))
            {
                (Setting::TimeZone, value)
            } else if let Some(value) = strip_word(rest, "NAMES") {
                (Setting::ClientEncoding, value)
            } else {
                let Some((name, value)) = rest
                    .split_once('=')
                    .or_else(|| split_word(rest, "TO"))
                    .map(|(name, value)| (name.trim(), value.trim()))
                else {
                    return Some(Command::Unsupported(rest.to_owned()));
                };
                let Some(setting) = Setting::of(name) else {
                    return Some(Command::Unsupported(name.to_owned()));
                };
                (setting, value)
            };
            let value = unquote(value.trim());
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
        Command::Unsupported(name) => return Err(unsupported(&name)),
        Command::Show(setting) => {
            // A startup parameter is keyed as the client spelled it.
            let value = metadata
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(setting.name()))
                .map_or_else(|| setting.unset().to_owned(), |(_, value)| value.clone());
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

/// `SET` of a parameter the listener does not offer: refused, not ignored,
/// so a client relying on it learns it has no effect.
pub(super) fn unsupported(name: &str) -> pgwire::error::PgWireError {
    let offered = SETTINGS.map(Setting::name).join(", ");
    err(
        "0A000",
        format!(
            "parameter \"{name}\" is not supported over the PostgreSQL wire; \
             the parameters SET accepts are {offered}"
        ),
    )
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
        // SHOW of other settings is not this module's.
        assert_eq!(parse("SHOW server_version"), None);
        assert_eq!(parse("SELECT 1"), None);
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

    #[test]
    fn client_parameters_are_recorded_and_shown() {
        // PGW-1: Metabase (JDBC), psycopg and node-postgres set these on
        // every connection; refusing them refused the connection.
        assert_eq!(
            parse("SET application_name = 'Metabase v0.57.4 [abc]'"),
            Some(Command::Set(
                Setting::ApplicationName,
                Some("Metabase v0.57.4 [abc]".into())
            ))
        );
        assert_eq!(
            parse("SET TIME ZONE 'UTC'"),
            Some(Command::Set(Setting::TimeZone, Some("UTC".into())))
        );
        assert_eq!(
            parse("SET NAMES 'utf8'"),
            Some(Command::Set(Setting::ClientEncoding, Some("utf8".into())))
        );
        let mut metadata = session(&[
            "SET application_name = 'Metabase v0.57.4 [abc]'",
            "SET extra_float_digits = 3",
            "SET DateStyle = 'ISO'",
            "SET client_encoding TO 'UTF-8'",
            "SET search_path = \"$user\", public",
            "SET TIME ZONE 'Europe/Berlin'",
        ]);
        let mut shown = |sql: &str| {
            let Some(Command::Show(setting)) = parse(sql) else {
                panic!("{sql}");
            };
            apply(&mut metadata, Command::Show(setting))
                .unwrap()
                .unwrap()[0][setting.name()]
            .clone()
        };
        assert_eq!(shown("SHOW application_name"), "Metabase v0.57.4 [abc]");
        assert_eq!(shown("SHOW extra_float_digits"), "3");
        assert_eq!(shown("show datestyle"), "ISO");
        assert_eq!(shown("SHOW client_encoding"), "UTF8");
        assert_eq!(shown("SHOW TimeZone"), "Europe/Berlin");
        assert_eq!(shown("SHOW IntervalStyle"), "postgres");
        // They bound nothing.
        assert_eq!(limits(&metadata).unwrap(), SessionLimits::default());
        // A startup parameter is shown as the client sent it.
        let mut startup = HashMap::from([("DateStyle".to_owned(), "ISO, DMY".to_owned())]);
        let row = apply(&mut startup, Command::Show(Setting::DateStyle))
            .unwrap()
            .unwrap();
        assert_eq!(row[0]["DateStyle"], "ISO, DMY");
        // A value the server cannot honour is refused.
        for sql in [
            "SET client_encoding = 'LATIN1'",
            "SET DateStyle = 'German'",
            "SET search_path = rushdb_edges",
            "SET extra_float_digits = 9",
            "SET standard_conforming_strings = off",
        ] {
            let error = apply(&mut startup, parse(sql).unwrap()).unwrap_err();
            assert!(error.to_string().contains("22023"), "{sql}: {error}");
        }
    }

    #[test]
    fn other_parameters_are_refused_as_unsupported() {
        // PGW-1: not "send writes as mutate.statement".
        for sql in [
            "SET enable_seqscan = off",
            "SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY",
            "RESET enable_seqscan",
        ] {
            let command = parse(sql).expect(sql);
            assert!(matches!(command, Command::Unsupported(_)), "{sql}");
            let error = apply(&mut HashMap::new(), command).unwrap_err().to_string();
            assert!(error.contains("0A000"), "{sql}: {error}");
            assert!(error.contains("application_name"), "{error}");
        }
    }
}
