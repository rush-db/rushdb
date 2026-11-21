//! The value type registry: everything the engine knows about a logical
//! type, in one place. Code that needs "every type that supports ranges" or
//! "the API name of a type" asks [`LogicalType`] here instead of keeping a
//! list of its own, so a new type is added by extending this module.
//!
//! The temporal family, durations and points have their own modules; this
//! one also holds the literal forms every surface shares: parsing a JSON
//! literal for a type, inferring a type from a JSON value, and the tagged
//! form non-JSON values take inside documents the engine builds.

/// Durations.
pub mod duration;
/// Points: GeoJSON, distances, boxes and the Hilbert cell of the index.
pub mod point;
/// Dates, times and instants.
pub mod temporal;

use serde_json::{Map, Value as Json};

use crate::model::{LogicalType, TypeState};
use crate::value::Scalar;

impl LogicalType {
    /// Every logical type, in tag order.
    pub const ALL: [LogicalType; 10] = [
        LogicalType::Boolean,
        LogicalType::Int64,
        LogicalType::Float64,
        LogicalType::Utf8,
        LogicalType::DateTime,
        LogicalType::Date,
        LogicalType::LocalTime,
        LogicalType::LocalDateTime,
        LogicalType::Duration,
        LogicalType::Point,
    ];

    /// Every logical type in the order the catalog reports a property
    /// family's typed definitions: numbers, then strings, booleans and
    /// temporal values.
    pub const CATALOG_ORDER: [LogicalType; 10] = [
        LogicalType::Int64,
        LogicalType::Float64,
        LogicalType::Utf8,
        LogicalType::Boolean,
        LogicalType::DateTime,
        LogicalType::Date,
        LogicalType::LocalDateTime,
        LogicalType::LocalTime,
        LogicalType::Duration,
        LogicalType::Point,
    ];

    /// The stable one-byte tag of the type in encoded keys and occurrence
    /// rows.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            LogicalType::Boolean => 1,
            LogicalType::Int64 => 2,
            LogicalType::Float64 => 3,
            LogicalType::Utf8 => 4,
            LogicalType::DateTime => 5,
            LogicalType::Date => 6,
            LogicalType::LocalTime => 7,
            LogicalType::LocalDateTime => 8,
            LogicalType::Duration => 9,
            LogicalType::Point => 10,
        }
    }

    /// The type a tag names, `None` for an unknown tag.
    #[must_use]
    pub fn from_tag(tag: u8) -> Option<LogicalType> {
        LogicalType::ALL.into_iter().find(|ty| ty.tag() == tag)
    }

    /// The internal name (`int64`, `utf8`, ...): catalogs and `describe`
    /// responses that report a property's physical type.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            LogicalType::Boolean => "boolean",
            LogicalType::Int64 => "int64",
            LogicalType::Float64 => "float64",
            LogicalType::Utf8 => "utf8",
            LogicalType::DateTime => "datetime",
            LogicalType::Date => "date",
            LogicalType::LocalTime => "localtime",
            LogicalType::LocalDateTime => "localdatetime",
            LogicalType::Duration => "duration",
            LogicalType::Point => "point",
        }
    }

    /// The public name (`number`, `string`, ...): the property types of the
    /// API, `$type` and value suggestions. Integers and floats are both
    /// `number`.
    #[must_use]
    pub const fn api_name(self) -> &'static str {
        match self {
            LogicalType::Boolean => "boolean",
            LogicalType::Int64 | LogicalType::Float64 => "number",
            LogicalType::Utf8 => "string",
            other => other.name(),
        }
    }

    /// The types an API name covers (`number` is integers and floats);
    /// empty for an unknown name.
    #[must_use]
    pub fn of_api_name(name: &str) -> Vec<LogicalType> {
        LogicalType::ALL
            .into_iter()
            .filter(|ty| ty.api_name() == name)
            .collect()
    }

    /// The type a name names: an internal name (`int64`), or a public one
    /// that covers one type (`string`, `datetime`; not `number`).
    #[must_use]
    pub fn of_name(name: &str) -> Option<LogicalType> {
        let name = name.trim().to_ascii_lowercase();
        LogicalType::ALL
            .into_iter()
            .find(|ty| ty.name() == name)
            .or_else(|| match LogicalType::of_api_name(&name).as_slice() {
                [only] => Some(*only),
                _ => None,
            })
    }

    /// Whether values of the type are numbers: integers and floats share one
    /// comparison domain.
    #[must_use]
    pub const fn numeric(self) -> bool {
        matches!(self, LogicalType::Int64 | LogicalType::Float64)
    }

    /// Whether `$gt`/`$gte`/`$lt`/`$lte` apply to values of the type.
    #[must_use]
    pub const fn rangeable(self) -> bool {
        !matches!(
            self,
            LogicalType::Boolean | LogicalType::Utf8 | LogicalType::Point
        )
    }

    /// Whether the order of the encoded payload is the value order, so an
    /// ordered index can return values sorted.
    #[must_use]
    pub const fn ordered(self) -> bool {
        !matches!(self, LogicalType::Point)
    }

    /// Whether the type is a point in time or a calendar value.
    #[must_use]
    pub const fn temporal(self) -> bool {
        matches!(
            self,
            LogicalType::DateTime
                | LogicalType::Date
                | LogicalType::LocalTime
                | LogicalType::LocalDateTime
        )
    }

    /// Whether values of the type have a calendar date (`$year`, `$month`,
    /// `$day`, `$timeBucket`).
    #[must_use]
    pub const fn has_date(self) -> bool {
        matches!(
            self,
            LogicalType::DateTime | LogicalType::Date | LogicalType::LocalDateTime
        )
    }

    /// Whether values of the type have a time of day (`$hour` …
    /// `$microsecond`).
    #[must_use]
    pub const fn has_time(self) -> bool {
        matches!(
            self,
            LogicalType::DateTime | LogicalType::LocalTime | LogicalType::LocalDateTime
        )
    }

    /// Whether `$within` and `$distance` apply to values of the type.
    #[must_use]
    pub const fn spatial(self) -> bool {
        matches!(self, LogicalType::Point)
    }

    /// Whether a value of the type is a JSON value of its own (boolean,
    /// number, string), not a literal form inside one.
    #[must_use]
    pub const fn json_native(self) -> bool {
        matches!(
            self,
            LogicalType::Boolean | LogicalType::Int64 | LogicalType::Float64 | LogicalType::Utf8
        )
    }

    /// Every type state a property definition may have: unresolved, then
    /// each known type.
    #[must_use]
    pub fn all_type_states() -> Vec<TypeState> {
        std::iter::once(TypeState::Unresolved)
            .chain(LogicalType::ALL.into_iter().map(TypeState::Known))
            .collect()
    }
}

/// The canonical ISO 8601 text of a temporal or duration scalar; `None`
/// for the other types.
#[must_use]
pub fn literal_text(scalar: &Scalar) -> Option<String> {
    Some(match scalar {
        Scalar::DateTime(instant) => temporal::format_instant(*instant),
        Scalar::Date(days) => temporal::format_date(*days),
        Scalar::LocalTime(micros) => temporal::format_local_time(*micros),
        Scalar::LocalDateTime(micros) => temporal::format_local_datetime(*micros),
        Scalar::Duration(duration) => duration::format_duration(*duration),
        _ => return None,
    })
}

/// Parse a literal of `ty` written in its own form: a JSON boolean, number
/// or string for the native types, ISO 8601 text for the temporal types
/// and durations, a `GeoJSON` point. No coercion between types.
#[must_use]
pub fn parse_literal(ty: LogicalType, value: &Json) -> Option<Scalar> {
    match (ty, value) {
        (LogicalType::Point, value) => point::parse_geojson(value).map(Scalar::Point),
        (_, Json::String(text)) => parse_text(ty, text),
        (LogicalType::Boolean, Json::Bool(b)) => Some(Scalar::Boolean(*b)),
        (LogicalType::Int64, Json::Number(n)) => n.as_i64().map(Scalar::Int64),
        (LogicalType::Float64, Json::Number(n)) => n.as_f64().map(Scalar::Float64),
        _ => None,
    }
}

fn parse_text(ty: LogicalType, text: &str) -> Option<Scalar> {
    Some(match ty {
        LogicalType::Utf8 => Scalar::Utf8(text.to_owned()),
        LogicalType::DateTime => Scalar::DateTime(temporal::parse_instant(text)?),
        LogicalType::Date => Scalar::Date(temporal::parse_date(text)?),
        LogicalType::LocalTime => Scalar::LocalTime(temporal::parse_local_time(text)?),
        LogicalType::LocalDateTime => Scalar::LocalDateTime(temporal::parse_local_datetime(text)?),
        LogicalType::Duration => Scalar::Duration(duration::parse_duration(text)?),
        LogicalType::Boolean | LogicalType::Int64 | LogicalType::Float64 | LogicalType::Point => {
            return None
        }
    })
}

/// The temporal or duration value ISO 8601 text writes, by its form: a
/// date, a time of day, a date-time with `Z` or an offset (an instant), one
/// without (a local date-time), a duration. `None` for other text.
#[must_use]
pub fn infer_text(text: &str) -> Option<Scalar> {
    [
        LogicalType::DateTime,
        LogicalType::LocalDateTime,
        LogicalType::Date,
        LogicalType::LocalTime,
        LogicalType::Duration,
    ]
    .into_iter()
    .find_map(|ty| parse_text(ty, text))
}

/// The scalar of a JSON value with types suggested from its form: native
/// values as [`crate::value::infer_scalar`] reads them, ISO 8601 text as
/// [`infer_text`] does, and an object that is exactly a `GeoJSON` point as a
/// point. `None` for text of no temporal form is not returned: it is a
/// string.
#[must_use]
pub fn infer_typed(value: &Json) -> Option<Scalar> {
    match value {
        Json::String(text) => Some(infer_text(text).unwrap_or_else(|| Scalar::Utf8(text.clone()))),
        Json::Object(_) => point::parse_geojson(value).map(Scalar::Point),
        other => crate::value::infer_scalar(other).map(|(_, scalar)| scalar),
    }
}

/// A filter literal read as a value of `ty`, the type of a property it is
/// compared with: its own form ([`parse_literal`]), or one of the
/// conversions every filter has made:
/// - a datetime compares with a date (midnight UTC), a local date-time
///   (read in UTC) and an integer (epoch milliseconds);
/// - a local date-time compares with a date (midnight);
/// - integers and floats compare with each other (the numeric domain).
///
/// `None` when the literal cannot be a value of `ty`: the property's values
/// of that type never match it.
#[must_use]
pub fn coerce_literal(ty: LogicalType, value: &Json) -> Option<Scalar> {
    if let Some(scalar) = parse_literal(ty, value) {
        return Some(scalar);
    }
    match (ty, value) {
        (LogicalType::DateTime, Json::String(text)) => {
            if let Some(micros) = temporal::parse_local_datetime(text) {
                return Some(Scalar::DateTime(temporal::Instant::utc(micros)));
            }
            let days = temporal::parse_date(text)?;
            Some(Scalar::DateTime(temporal::Instant::utc(
                i64::from(days).checked_mul(temporal::MICROS_PER_DAY)?,
            )))
        }
        (LogicalType::DateTime, Json::Number(n)) => {
            temporal::Instant::from_millis(n.as_i64()?).map(Scalar::DateTime)
        }
        (LogicalType::LocalDateTime, Json::String(text)) => Some(Scalar::LocalDateTime(
            i64::from(temporal::parse_date(text)?).checked_mul(temporal::MICROS_PER_DAY)?,
        )),
        _ => None,
    }
}

/// Whether a document value equals a filter literal: a typed value when
/// the literal coerces to its type ([`coerce_literal`]) and the values are
/// equal; JSON's own values as JSON compares them (integers and floats by
/// their numeric value).
#[must_use]
pub fn doc_equals_literal(actual: &Json, literal: &Json) -> bool {
    if let Some(scalar) = tagged_scalar(actual) {
        let expected =
            tagged_scalar(literal).or_else(|| coerce_literal(scalar.logical_type(), literal));
        return expected.is_some_and(|expected| crate::value::scalar_equal(&scalar, &expected));
    }
    match (actual, literal) {
        (Json::String(x), Json::String(y)) => x == y,
        (Json::Bool(x), Json::Bool(y)) => x == y,
        (Json::Number(_), Json::Number(_)) => {
            match (
                crate::value::infer_scalar(actual),
                crate::value::infer_scalar(literal),
            ) {
                (Some((_, left)), Some((_, right))) => crate::value::scalar_equal(&left, &right),
                _ => false,
            }
        }
        (Json::Null, Json::Null) => true,
        _ => false,
    }
}

/// How a document value compares with a range bound (`$gt` … `$lte`):
/// numbers with numbers, a typed value with a bound that coerces to its
/// type. `None` when they do not compare (text, booleans, points, or a
/// bound of another type): the predicate does not hold.
#[must_use]
pub fn doc_cmp_literal(actual: &Json, bound: &Json) -> Option<std::cmp::Ordering> {
    if let Some(scalar) = tagged_scalar(actual) {
        if !scalar.logical_type().rangeable() {
            return None;
        }
        let bound =
            tagged_scalar(bound).or_else(|| coerce_literal(scalar.logical_type(), bound))?;
        return crate::value::scalar_cmp(&scalar, &bound);
    }
    let numeric = |value: &Json| match crate::value::infer_scalar(value)? {
        (_, scalar @ (Scalar::Int64(_) | Scalar::Float64(_))) => Some(scalar),
        _ => None,
    };
    let bound = numeric(bound).or_else(|| epoch_millis_bound(bound).map(Scalar::Int64))?;
    crate::value::scalar_cmp(&numeric(actual)?, &bound)
}

/// A temporal bound as the epoch milliseconds it names, for a number
/// compared with it: integer timestamps have always compared with ISO 8601
/// text this way.
#[must_use]
pub fn epoch_millis_bound(bound: &Json) -> Option<i64> {
    if matches!(bound, Json::Number(_)) {
        return None;
    }
    let instant = match tagged_scalar(bound) {
        Some(Scalar::DateTime(instant)) => instant,
        Some(_) => return None,
        None => match coerce_literal(LogicalType::DateTime, bound)? {
            Scalar::DateTime(instant) => instant,
            _ => return None,
        },
    };
    Some(instant.micros.div_euclid(1_000))
}

/// One calendar component of a temporal scalar: of its wall clock for the
/// local types, of its UTC instant for a datetime (and for an integer,
/// read as epoch milliseconds). `None` for a component
/// the type has no part for (the hour of a date) and for other types.
#[must_use]
pub fn component_of(scalar: &Scalar, component: temporal::Component) -> Option<i64> {
    let micros = match scalar {
        Scalar::DateTime(instant) => instant.micros,
        // An integer is a timestamp in epoch milliseconds, as it always was
        // to these predicates.
        Scalar::Int64(ms) => ms.checked_mul(1_000)?,
        Scalar::LocalDateTime(micros) => *micros,
        Scalar::Date(days) if component.of_date() => {
            i64::from(*days).checked_mul(temporal::MICROS_PER_DAY)?
        }
        Scalar::LocalTime(micros) if !component.of_date() => *micros,
        _ => return None,
    };
    temporal::component_of_micros(micros, component)
}

/// The key of the one-member object a non-JSON value takes inside a
/// document the engine builds: `__` and the type's name
/// (`{"__datetime": "2024-05-01T10:15:00+02:00"}`). Property names
/// starting with `__` are reserved, so the form cannot be a user's object.
fn tag_key(ty: LogicalType) -> String {
    format!("__{}", ty.name())
}

/// A scalar as a document value: JSON's own value for the native types,
/// the tagged form (`{"__date": "2024-05-01"}`) for the others.
#[must_use]
pub fn to_doc_json(scalar: &Scalar) -> Json {
    let ty = scalar.logical_type();
    if ty.json_native() {
        return crate::value::scalar_to_json(scalar);
    }
    let mut object = Map::with_capacity(1);
    object.insert(tag_key(ty), crate::value::scalar_to_json(scalar));
    Json::Object(object)
}

/// The scalar of a tagged document value; `None` for any other value.
#[must_use]
pub fn tagged_scalar(value: &Json) -> Option<Scalar> {
    let object = value.as_object()?;
    if object.len() != 1 {
        return None;
    }
    let (key, literal) = object.iter().next()?;
    let ty = LogicalType::ALL
        .into_iter()
        .find(|ty| !ty.json_native() && key.strip_prefix("__") == Some(ty.name()))?;
    parse_literal(ty, literal)
}

/// The scalar of a document value: a tagged value, or a JSON boolean,
/// number or string (never re-typed from its text).
#[must_use]
pub fn doc_scalar(value: &Json) -> Option<Scalar> {
    tagged_scalar(value).or_else(|| crate::value::infer_scalar(value).map(|(_, s)| s))
}

/// A document value in its public form: tagged values become their
/// literals (ISO 8601 text, `GeoJSON`), in arrays and objects too.
#[must_use]
pub fn doc_to_public(mut value: Json) -> Json {
    publish_in_place(&mut value);
    value
}

/// [`doc_to_public`] in place: only tagged values are replaced, so a
/// document without one is walked and left as it is (every response row
/// passes here; rebuilding each object cost more than the query on small
/// answers).
pub fn publish_in_place(value: &mut Json) {
    if let Some(scalar) = tagged_scalar(value) {
        *value = crate::value::scalar_to_json(&scalar);
        return;
    }
    match value {
        Json::Array(items) => items.iter_mut().for_each(publish_in_place),
        Json::Object(object) => object.values_mut().for_each(publish_in_place),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_type_has_one_tag_and_one_name() {
        let mut tags = std::collections::BTreeSet::new();
        let mut names = std::collections::BTreeSet::new();
        for ty in LogicalType::ALL {
            assert!(tags.insert(ty.tag()), "{ty:?}");
            assert!(names.insert(ty.name()), "{ty:?}");
            assert_eq!(LogicalType::from_tag(ty.tag()), Some(ty));
            assert!(LogicalType::of_api_name(ty.api_name()).contains(&ty));
        }
        assert_eq!(LogicalType::from_tag(0), None);
        assert_eq!(
            LogicalType::of_api_name("number"),
            [LogicalType::Int64, LogicalType::Float64]
        );
        assert_eq!(LogicalType::of_name("STRING"), Some(LogicalType::Utf8));
        assert_eq!(
            LogicalType::of_name("localtime"),
            Some(LogicalType::LocalTime)
        );
        assert_eq!(LogicalType::of_name("number"), None);
    }

    #[test]
    fn literals_infer_their_type_from_their_form() {
        use serde_json::json;
        let infer = |value: Json| infer_typed(&value).map(|s| s.logical_type());
        assert_eq!(infer(json!("2024-05-01")), Some(LogicalType::Date));
        assert_eq!(infer(json!("10:15:00")), Some(LogicalType::LocalTime));
        assert_eq!(
            infer(json!("2024-05-01T10:15")),
            Some(LogicalType::LocalDateTime)
        );
        assert_eq!(
            infer(json!("2024-05-01T10:15Z")),
            Some(LogicalType::DateTime)
        );
        assert_eq!(infer(json!("P1D")), Some(LogicalType::Duration));
        assert_eq!(infer(json!("PDF")), Some(LogicalType::Utf8));
        assert_eq!(infer(json!(1)), Some(LogicalType::Int64));
        assert_eq!(
            infer(json!({"type": "Point", "coordinates": [1.0, 2.0]})),
            Some(LogicalType::Point)
        );
        assert_eq!(infer(json!({"type": "Point"})), None);
    }

    #[test]
    fn document_values_round_trip_through_their_tagged_form() {
        use serde_json::json;
        for value in [
            json!("2024-05-01"),
            json!("10:15:00.5"),
            json!("2024-05-01T10:15:00"),
            json!("2024-05-01T10:15:00+02:00"),
            json!("P1Y2M"),
            json!({"type": "Point", "coordinates": [1.0, 2.0]}),
        ] {
            let scalar = infer_typed(&value).unwrap();
            let doc = to_doc_json(&scalar);
            assert_eq!(doc_scalar(&doc), Some(scalar), "{value}");
            assert_eq!(doc_to_public(json!({"a": [doc]})), json!({"a": [value]}));
        }
        // Text is never re-typed inside a document.
        assert_eq!(
            doc_scalar(&json!("2024-05-01")),
            Some(Scalar::Utf8("2024-05-01".into()))
        );
    }

    #[test]
    fn document_values_compare_with_literals_of_their_type() {
        use serde_json::json;
        use std::cmp::Ordering;
        let date = to_doc_json(&Scalar::Date(temporal::parse_date("2024-05-01").unwrap()));
        assert!(doc_equals_literal(&date, &json!("2024-05-01")));
        assert!(!doc_equals_literal(&date, &json!("2024-05-02")));
        assert!(!doc_equals_literal(&json!("2024-05-01"), &json!(1)));
        assert_eq!(
            doc_cmp_literal(&date, &json!("2024-04-30")),
            Some(Ordering::Greater)
        );
        assert_eq!(doc_cmp_literal(&date, &json!(5)), None);
        assert_eq!(
            doc_cmp_literal(&json!(5), &json!(4.5)),
            Some(Ordering::Greater)
        );
        assert_eq!(doc_cmp_literal(&json!("b"), &json!("a")), None);
        let instant = to_doc_json(&Scalar::DateTime(
            temporal::parse_instant("2024-05-01T10:00:00+02:00").unwrap(),
        ));
        assert!(doc_equals_literal(&instant, &json!("2024-05-01T08:00:00Z")));
        assert_eq!(
            doc_cmp_literal(&instant, &json!("2024-05-01")),
            Some(Ordering::Greater)
        );
        let hour =
            |value: &Json| component_of(&doc_scalar(value).unwrap(), temporal::Component::Hour);
        assert_eq!(hour(&instant), Some(8));
        assert_eq!(hour(&date), None);
    }

    #[test]
    fn filter_literals_coerce_as_filters_always_have() {
        use serde_json::json;
        let instant = |text: &str| Scalar::DateTime(temporal::parse_instant(text).unwrap());
        assert_eq!(
            coerce_literal(LogicalType::DateTime, &json!("2024-05-01")),
            Some(instant("2024-05-01T00:00:00Z"))
        );
        assert_eq!(
            coerce_literal(LogicalType::DateTime, &json!("2024-05-01T10:00")),
            Some(instant("2024-05-01T10:00:00Z"))
        );
        assert_eq!(
            coerce_literal(LogicalType::DateTime, &json!(1_000)),
            Some(instant("1970-01-01T00:00:01Z"))
        );
        assert_eq!(
            coerce_literal(LogicalType::Date, &json!("2024-05-01T00:00Z")),
            None
        );
        assert_eq!(
            coerce_literal(LogicalType::LocalDateTime, &json!("1970-01-02")),
            Some(Scalar::LocalDateTime(temporal::MICROS_PER_DAY))
        );
    }
}
