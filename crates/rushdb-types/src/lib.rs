//! Canonical value model, identifiers, key layout, encodings, and the
//! maintained-key constants shared by storage, LMPG, and the query engine.
//!
//! This crate defines the vocabulary every other crate agrees on: tenant and
//! record/relationship identities, the order-preserving scalar encoding used
//! by occurrence values and range scans, and the byte-exact key constructors
//! (for example [`keys::occ_key`] and [`keys::presence_key`]). Keeping the
//! key layout here is what lets a persisted database remain readable across
//! engine versions.
/// Replicated command batches, mutations and preconditions.
pub mod command;
/// Binary codec of [`command::CommitBatch`] for the WAL, outcome payloads
/// and replication.
pub mod command_codec;
/// Canonical date/datetime parsing and formatting helpers.
pub mod datetime;
/// Order-preserving byte encodings for the scalar value model.
pub mod encoding;
/// Error type shared by the canonical type layer.
pub mod error;
/// Stable hashing helpers and hash domains.
pub mod hash;
/// Tenant, record, relationship and dense-ordinal identifiers.
pub mod ids;
/// Declared secondary indexes.
pub mod index;
/// Byte-exact key constructors for the tenant keyspace.
pub mod keys;
/// Logical types, catalog state, cardinality and nullability.
pub mod model;
/// Name tokens and spelling similarity for labels and properties.
pub mod names;
/// Occurrence rows, headers and property/embedding metadata.
pub mod occurrence;
/// Canonical tenant policy payloads for authorization.
pub mod security;
/// Planner value-distribution summaries (estimates, never results).
pub mod stats;
/// Scalar value model and JSON conversion helpers.
pub mod value;
/// Matching a search text against stored values.
pub mod value_match;

pub use error::TypesError;

/// Convenience alias for a [`std::result::Result`] over [`TypesError`].
pub type Result<T> = std::result::Result<T, TypesError>;
