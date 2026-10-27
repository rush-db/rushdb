use serde::{Deserialize, Serialize};
use std::fmt;

/// The canonical scalar domain stored in postings and occurrences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LogicalType {
    /// Boolean value.
    Boolean,
    /// Signed 64-bit integer.
    Int64,
    /// IEEE-754 double-precision float.
    Float64,
    /// UTF-8 string.
    Utf8,
    /// UTC epoch milliseconds.
    DateTime,
}

impl fmt::Display for LogicalType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            LogicalType::Boolean => "boolean",
            LogicalType::Int64 => "int64",
            LogicalType::Float64 => "float64",
            LogicalType::Utf8 => "utf8",
            LogicalType::DateTime => "datetime",
        };
        f.write_str(name)
    }
}

/// Catalog state: definitions are Unresolved until type evidence exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeState {
    /// No value has been observed yet, so no type is committed.
    Unresolved,
    /// A concrete logical type has been observed.
    Known(LogicalType),
}

/// How many values a property may hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cardinality {
    /// Exactly one value per owner.
    Single,
    /// Multiple values whose order is significant.
    OrderedMany,
    /// Multiple values treated as an unordered set.
    UnorderedMany,
}

/// Whether a property may be absent or explicitly null.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Nullability {
    /// No nullability evidence has been recorded.
    Unspecified,
    /// Explicit nulls are permitted.
    Nullable,
    /// Nulls are rejected.
    NotNull,
}
