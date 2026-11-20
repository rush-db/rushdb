//! The value type registry: everything the engine knows about a logical
//! type, in one place. Code that needs "every type that supports ranges" or
//! "the API name of a type" asks [`LogicalType`] here instead of keeping a
//! list of its own, so a new type is added by extending this module.

use crate::model::{LogicalType, TypeState};

impl LogicalType {
    /// Every logical type, in tag order.
    pub const ALL: [LogicalType; 5] = [
        LogicalType::Boolean,
        LogicalType::Int64,
        LogicalType::Float64,
        LogicalType::Utf8,
        LogicalType::DateTime,
    ];

    /// Every logical type in the order the catalog reports a property
    /// family's typed definitions: numbers, then strings, booleans and
    /// temporal values.
    pub const CATALOG_ORDER: [LogicalType; 5] = [
        LogicalType::Int64,
        LogicalType::Float64,
        LogicalType::Utf8,
        LogicalType::Boolean,
        LogicalType::DateTime,
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
            LogicalType::DateTime => "datetime",
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

    /// Whether values of the type are numbers: integers and floats share one
    /// comparison domain.
    #[must_use]
    pub const fn numeric(self) -> bool {
        matches!(self, LogicalType::Int64 | LogicalType::Float64)
    }

    /// Whether `$gt`/`$gte`/`$lt`/`$lte` apply to values of the type.
    #[must_use]
    pub const fn rangeable(self) -> bool {
        matches!(
            self,
            LogicalType::Int64 | LogicalType::Float64 | LogicalType::DateTime
        )
    }

    /// Whether the order of the encoded payload is the value order, so an
    /// ordered index can return values sorted.
    #[must_use]
    pub const fn ordered(self) -> bool {
        true
    }

    /// Whether the type is a point in time or a calendar value.
    #[must_use]
    pub const fn temporal(self) -> bool {
        matches!(self, LogicalType::DateTime)
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
    }
}
