//! Declared secondary indexes: the client chooses what is fast.
//!
//! An index belongs to one record label and one property (by normalized
//! family name, so it covers every typed definition of that property). Its
//! entries are derived rows maintained by the apply-time deriver; the state
//! decides which records they cover, so entries always equal a from-scratch
//! derivation of the base rows, including while a build or drop is running.

use serde::{Deserialize, Serialize};

/// One declared index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexDef {
    /// Tenant-local index id (allocated at definition, never reused).
    pub id: u32,
    /// Tenant-unique name.
    pub name: String,
    /// Record label the index covers.
    pub label: String,
    /// Normalized property (family) name.
    pub property: String,
    /// Index kind.
    pub kind: IndexKind,
    /// Build/drop state.
    pub state: IndexState,
}

/// What an index answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IndexKind {
    /// Ordered entries `(value, owner)`: equality, `$in`, ranges, prefix.
    Ordered,
    /// Full-text (BM25) over the property's string values — every string
    /// property of the record when the property is [`TEXT_ALL_PROPERTIES`].
    /// Maintained by the text applier from pending rows.
    Text {
        /// When a search sees a write.
        consistency: TextConsistency,
    },
    /// Structural embeddings: one vector per record of the label, derived
    /// from the graph around it (`FastRP` over every relationship, ignoring
    /// direction), bound into the vector space named after the index.
    /// Maintained by the structural applier from pending rows: a
    /// relationship or record change marks the records within two hops.
    /// Its property is [`TEXT_ALL_PROPERTIES`]: it reads topology, not a
    /// property.
    Structural {
        /// Vector dimensions.
        dimensions: u32,
    },
}

impl IndexKind {
    /// Whether an applier outside the store maintains the index from pending
    /// rows (text and structural), rather than apply-time entries.
    #[must_use]
    pub fn applied(self) -> bool {
        matches!(self, Self::Text { .. } | Self::Structural { .. })
    }
}

/// Property name of a text index covering every string property (and of a
/// structural index, which covers no single property).
pub const TEXT_ALL_PROPERTIES: &str = "*";

/// When a text search sees committed writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextConsistency {
    /// A search first applies every pending change up to its snapshot
    /// (read-your-writes).
    Immediate,
    /// A search reads what the background applier has applied and reports
    /// the backlog.
    Eventual,
}

/// Lifecycle of an index. Records covered by the entries:
/// `Building{cursor}` covers ordinals `< cursor`, `Ready` covers all, and
/// `Dropping{cursor}` covers ordinals `≥ cursor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum IndexState {
    /// Backfill has covered records below `cursor`.
    Building {
        /// First record ordinal not yet backfilled.
        cursor: u64,
    },
    /// Every record is covered; the planner may use the index.
    Ready,
    /// Entries of records below `cursor` have been removed.
    Dropping {
        /// First record ordinal whose entries remain.
        cursor: u64,
    },
}

impl IndexState {
    /// Half-open ordinal interval `[start, end)` of covered records.
    #[must_use]
    pub fn covered(self) -> (u64, u64) {
        match self {
            Self::Building { cursor } => (0, cursor),
            Self::Ready => (0, u64::MAX),
            Self::Dropping { cursor } => (cursor, u64::MAX),
        }
    }

    /// Whether record ordinal `ord` is covered.
    #[must_use]
    pub fn covers(self, ord: u64) -> bool {
        let (start, end) = self.covered();
        start <= ord && ord < end
    }

    /// Wire label (`building`, `ready`, `dropping`).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Building { .. } => "building",
            Self::Ready => "ready",
            Self::Dropping { .. } => "dropping",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_follows_the_cursor() {
        assert!(!IndexState::Building { cursor: 0 }.covers(0));
        let building = IndexState::Building { cursor: 5 };
        assert!(building.covers(0) && building.covers(4) && !building.covers(5));
        assert!(IndexState::Ready.covers(u64::MAX - 1));
        let dropping = IndexState::Dropping { cursor: 5 };
        assert!(!dropping.covers(4) && dropping.covers(5));
    }
}
