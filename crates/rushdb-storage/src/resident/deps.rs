//! What a resident entry reflects, and the one place that maps a written
//! key to it.
//!
//! [`Dep`] is a closed set of storage areas. [`deps_of_key`] maps every
//! written key to the areas it changes through an exhaustive match over
//! [`Family`]: adding a key family without deciding which areas it changes
//! does not compile. The publisher (and every replay or apply path) feeds
//! each op of a commit through it into the [`VersionTable`] before the
//! commit becomes visible.

use std::sync::atomic::{AtomicU64, Ordering};

use rushdb_types::ids::TenantId;
use rushdb_types::keys::{self, TENANT_PREFIX_LEN};

/// A storage area a resident entry reflects. A write to the area makes the
/// entry a miss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dep {
    /// Entries of one declared index.
    Index(TenantId, u32),
    /// Occurrences, array markers, presence and support counters of one
    /// property id (any owner, any label).
    Pid(TenantId, u64),
    /// Membership rows and the suspect marker of one label (by
    /// [`label_hash`]).
    Label(TenantId, u64),
    /// The label-scoped property rows of one label (by [`label_hash`]):
    /// presence, per-label support counters, array markers and the value
    /// dictionary with its counters, of any property id. Every such row
    /// also raises its property's [`Dep::Pid`] or [`Dep::ValueDict`].
    LabelProperties(TenantId, u64),
    /// Value dictionary and distinct counters of one property id, any label.
    ValueDict(TenantId, u64),
    /// Property families, definitions and signatures.
    Definitions(TenantId),
    /// The label catalog: per-label record counters and any label's
    /// membership (the catalog orders labels by their oldest member).
    Labels(TenantId),
    /// Adjacency and relationship rows, degree and relationship catalogs.
    Adjacency(TenantId),
    /// Bindings, vectors, version and ready count of one embedding space.
    Space(TenantId, u64),
    /// Security rows (policy and generation).
    Policy(TenantId),
    /// The tenant directory (system namespace).
    TenantDir,
    /// A tenant key no family claims. Every entry of the tenant depends on
    /// it, so an unclassified write can only cause misses.
    Tenant(TenantId),
    /// A system or global key no family claims; every entry depends on it.
    Global,
}

impl Dep {
    /// The tenant the area belongs to, if any.
    #[must_use]
    pub fn tenant(self) -> Option<TenantId> {
        match self {
            Self::Index(tenant, _)
            | Self::Pid(tenant, _)
            | Self::Label(tenant, _)
            | Self::LabelProperties(tenant, _)
            | Self::ValueDict(tenant, _)
            | Self::Definitions(tenant)
            | Self::Labels(tenant)
            | Self::Adjacency(tenant)
            | Self::Space(tenant, _)
            | Self::Policy(tenant)
            | Self::Tenant(tenant) => Some(tenant),
            Self::TenantDir | Self::Global => None,
        }
    }

    /// A well-mixed 64-bit hash, stable for the life of the process.
    pub(crate) fn mix(self) -> u64 {
        let (tag, tenant, item) = match self {
            Self::Index(t, id) => (1, t.0, u64::from(id)),
            Self::Pid(t, pid) => (2, t.0, pid),
            Self::Label(t, hash) => (3, t.0, hash),
            Self::ValueDict(t, pid) => (4, t.0, pid),
            Self::Definitions(t) => (5, t.0, 0),
            Self::Labels(t) => (6, t.0, 0),
            Self::Adjacency(t) => (7, t.0, 0),
            Self::Space(t, space) => (8, t.0, space),
            Self::Policy(t) => (9, t.0, 0),
            Self::TenantDir => (10, 0, 0),
            Self::Tenant(t) => (11, t.0, 0),
            Self::Global => (12, 0, 0),
            Self::LabelProperties(t, hash) => (13, t.0, hash),
        };
        splitmix(splitmix((tag << 32) | u64::from(tenant)) ^ item)
    }
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// The hash a [`Dep::Label`] carries for `label` (FNV-1a, stable).
#[must_use]
pub fn label_hash(label: &str) -> u64 {
    label.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Every key family the store writes. [`deps_of_key`] matches it without a
/// wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family<'a> {
    RecordId,
    RecordHeader,
    LabelMember(&'a str),
    LabelCounter,
    PidCounter(u64),
    /// A `(label, pid)` support counter.
    LabelPidCounter(&'a str, u64),
    ValueCounter(&'a str, u64),
    GraphCounter,
    UsageCounter,
    UnknownCounter,
    ValueDict(&'a str, u64),
    /// A pid's marker, with its label when the marker is label-scoped.
    Marker(u64, Option<&'a str>),
    Occurrence(u64),
    Presence(&'a str, u64),
    IndexDef(u32),
    IndexName,
    IndexEntry(u32),
    IndexPending,
    UsageBytes,
    Rederive(&'a str),
    /// `meta/…` tenant rows (id allocators).
    Meta,
    Relationship,
    PropertyDefinitions,
    Adjacency,
    EmbeddingSpace(u64),
    EmbeddingAlias,
    EmbeddingReclaim,
    Security,
    Migration,
    /// Tenant directory rows (system namespace).
    TenantDirectory,
    /// Commit receipts, the log, apply/retention/checkpoint metadata and
    /// tenant-named vector metadata: nothing resident reads them.
    Bookkeeping,
    /// A tenant key in a family not listed above.
    UnknownTenant(TenantId),
    /// A system or global key not listed above.
    UnknownGlobal,
}

/// The family of a written key.
pub(crate) fn family(key: &[u8]) -> Family<'_> {
    match key.first() {
        Some(&keys::TENANT_NS) if key.len() > TENANT_PREFIX_LEN => {
            let tenant = keys::parse_tenant(key).unwrap_or(TenantId(0));
            tenant_family(tenant, key).unwrap_or(Family::UnknownTenant(tenant))
        }
        Some(&keys::TENANT_NS) => {
            Family::UnknownTenant(keys::parse_tenant(key).unwrap_or(TenantId(0)))
        }
        Some(&keys::SYSTEM_NS) => match key.get(1) {
            Some(&(keys::SYS_TENANT_BY_SCOPE | keys::SYS_TENANT_BY_ORD | keys::SYS_TENANT_SEQ)) => {
                Family::TenantDirectory
            }
            _ => Family::UnknownGlobal,
        },
        _ => {
            const BOOKKEEPING: [&[u8]; 7] = [
                b"receipt/",
                b"receipt-exp/",
                b"__log__/",
                b"meta/",
                b"checkpoint",
                b"vector/index/",
                b"vector/ann-manifest/",
            ];
            if BOOKKEEPING.iter().any(|prefix| key.starts_with(prefix)) {
                Family::Bookkeeping
            } else {
                Family::UnknownGlobal
            }
        }
    }
}

fn tenant_family(tenant: TenantId, key: &[u8]) -> Option<Family<'_>> {
    let at = TENANT_PREFIX_LEN + 1;
    Some(match key[TENANT_PREFIX_LEN] {
        keys::F_RECORD_ID => Family::RecordId,
        keys::F_RECORD_HDR => Family::RecordHeader,
        keys::F_LABEL => Family::LabelMember(keys::read_str(key, at)?.0),
        keys::F_COUNTER => counter_family(key, at)?,
        keys::F_VALUE_DICT => {
            let (label, pos) = keys::read_str(key, at + 1)?;
            Family::ValueDict(label, keys::read_ord(key, pos)?.0)
        }
        keys::F_MARKER => {
            let label = (key.get(at) == Some(&keys::M_ARRAY_LABEL))
                .then(|| keys::read_str(key, at + 10).map(|(label, _)| label))
                .flatten();
            Family::Marker(keys::read_u64(key, at + 2)?, label)
        }
        keys::F_OCC => {
            let (_, pos) = keys::read_ord(key, at + 1)?;
            Family::Occurrence(keys::read_ord(key, pos)?.0)
        }
        keys::F_PRESENCE => {
            let (label, pos) = keys::read_str(key, at + 1)?;
            Family::Presence(label, keys::read_ord(key, pos)?.0)
        }
        keys::F_INDEX_DEF => {
            Family::IndexDef(u32::from_be_bytes(key.get(at..at + 4)?.try_into().ok()?))
        }
        keys::F_INDEX_NAME => Family::IndexName,
        keys::F_INDEX_ENTRY => Family::IndexEntry(keys::parse_index_entry_index(key)?.1),
        keys::F_INDEX_PENDING => Family::IndexPending,
        keys::F_USAGE_BYTES => Family::UsageBytes,
        keys::F_REDERIVE => Family::Rederive(keys::parse_rederive_key(key)?.1),
        b'/' => segment_family(tenant, &key[at..])?,
        _ => return None,
    })
}

fn counter_family(key: &[u8], at: usize) -> Option<Family<'_>> {
    let sub = *key.get(at)?;
    let rest = at + 1;
    // `(kind, label, pid u64)` counters.
    let labelled_pid = || -> Option<(&str, u64)> {
        let (label, pos) = keys::read_str(key, rest + 1)?;
        Some((label, keys::read_u64(key, pos)?))
    };
    Some(match sub {
        keys::C_LABEL => Family::LabelCounter,
        keys::C_SUPPORT_TOTAL | keys::C_SUPPORT_BY_PID | keys::C_NULL => {
            Family::PidCounter(keys::read_u64(key, rest + 1)?)
        }
        keys::C_SUPPORT_BY_LABEL => {
            let (label, pid) = labelled_pid()?;
            Family::LabelPidCounter(label, pid)
        }
        keys::C_DISTINCT | keys::C_LONG_VALUES | keys::C_LONG_VALUE | keys::C_LONG_DISTINCT => {
            let (label, pid) = labelled_pid()?;
            Family::ValueCounter(label, pid)
        }
        keys::C_DEGREE | keys::C_REL_CATALOG | keys::C_REL_SHAPE_PROP => Family::GraphCounter,
        keys::C_USAGE => Family::UsageCounter,
        _ => Family::UnknownCounter,
    })
}

fn segment_family(tenant: TenantId, rest: &[u8]) -> Option<Family<'_>> {
    let mut parts = rest.splitn(2, |byte| *byte == b'/');
    let segment = parts.next()?;
    let tail = parts.next().unwrap_or_default();
    let space = || keys::read_u64(tail, 0);
    Some(match segment {
        keys::P_META => Family::Meta,
        keys::P_REL | keys::P_REL_BY_ORD | keys::P_REL_GROUP => Family::Relationship,
        keys::P_FAMILY_SIG | keys::P_FAMILY | keys::P_DEFINITION | b"property-row" => {
            Family::PropertyDefinitions
        }
        b"adj" => Family::Adjacency,
        keys::P_EMB_SPACE
        | keys::P_EMB_BINDING
        | keys::P_EMB_VECTOR
        | keys::P_EMB_VERSION
        | keys::P_EMB_READY => Family::EmbeddingSpace(space()?),
        keys::P_EMB_ALIAS => Family::EmbeddingAlias,
        keys::P_EMB_RECLAIM => Family::EmbeddingReclaim,
        b"security" => Family::Security,
        b"migration" => Family::Migration,
        _ => Family::UnknownTenant(tenant),
    })
}

/// The areas a write of `key` changes, passed to `out` (none for keys no
/// resident layer reads).
pub(crate) fn deps_of_key(key: &[u8], mut out: impl FnMut(Dep)) {
    let tenant = || keys::parse_tenant(key).unwrap_or(TenantId(0));
    match family(key) {
        Family::LabelMember(label) => {
            out(Dep::Label(tenant(), label_hash(label)));
            out(Dep::Labels(tenant()));
        }
        Family::Rederive(label) => out(Dep::Label(tenant(), label_hash(label))),
        Family::LabelCounter => out(Dep::Labels(tenant())),
        Family::PidCounter(pid) | Family::Marker(pid, None) | Family::Occurrence(pid) => {
            out(Dep::Pid(tenant(), pid));
        }
        Family::LabelPidCounter(label, pid)
        | Family::Marker(pid, Some(label))
        | Family::Presence(label, pid) => {
            out(Dep::Pid(tenant(), pid));
            out(Dep::LabelProperties(tenant(), label_hash(label)));
        }
        Family::ValueDict(label, pid) | Family::ValueCounter(label, pid) => {
            out(Dep::ValueDict(tenant(), pid));
            out(Dep::LabelProperties(tenant(), label_hash(label)));
        }
        Family::GraphCounter | Family::Relationship | Family::Adjacency => {
            out(Dep::Adjacency(tenant()));
        }
        Family::IndexDef(id) | Family::IndexEntry(id) => out(Dep::Index(tenant(), id)),
        Family::PropertyDefinitions => out(Dep::Definitions(tenant())),
        Family::EmbeddingSpace(space) => out(Dep::Space(tenant(), space)),
        Family::Security => out(Dep::Policy(tenant())),
        Family::TenantDirectory => out(Dep::TenantDir),
        Family::UnknownCounter | Family::UnknownTenant(_) => out(Dep::Tenant(tenant())),
        Family::UnknownGlobal => out(Dep::Global),
        // Read by no resident layer: record identity and headers (rows are
        // answered from occurrences and membership), id allocators, usage
        // accounting, text-index work queues, embedding aliases (resolved
        // per request, then keyed by space id), slot reclamation (slots
        // are never reused), migration progress, and commit bookkeeping.
        Family::RecordId
        | Family::RecordHeader
        | Family::UsageCounter
        | Family::IndexName
        | Family::IndexPending
        | Family::UsageBytes
        | Family::Meta
        | Family::EmbeddingAlias
        | Family::EmbeddingReclaim
        | Family::Migration
        | Family::Bookkeeping => {}
    }
}

/// Last writing commit index per dep, in a fixed table of hashed slots.
///
/// A write raises its slot to the commit index (`fetch_max`, so replays at
/// any index only raise). Two deps sharing a slot can only make an entry
/// look newer than it is (a spurious miss), never older: slots never
/// decrease, and an entry hits only while the maximum over its deps' slots
/// is exactly the version it recorded.
pub(crate) struct VersionTable {
    slots: Box<[AtomicU64]>,
    mask: u64,
}

impl VersionTable {
    /// A table of `2^bits` slots.
    pub(crate) fn new(bits: u32) -> Self {
        let len = 1usize << bits;
        Self {
            slots: (0..len).map(|_| AtomicU64::new(0)).collect(),
            mask: (len as u64) - 1,
        }
    }

    fn slot(&self, dep: Dep) -> &AtomicU64 {
        let index = usize::try_from(dep.mix() & self.mask).unwrap_or_default();
        &self.slots[index]
    }

    /// Record that commit `ci` wrote `dep`.
    pub(crate) fn bump(&self, dep: Dep, ci: u64) {
        self.slot(dep).fetch_max(ci, Ordering::SeqCst);
    }

    /// The version of `deps`: the largest commit index that wrote any of
    /// them (0 when none has since the store opened).
    pub(crate) fn version(&self, deps: &[Dep]) -> u64 {
        deps.iter()
            .map(|dep| self.slot(*dep).load(Ordering::SeqCst))
            .max()
            .unwrap_or(0)
    }

    /// Bytes the table holds.
    pub(crate) fn bytes(&self) -> usize {
        self.slots.len() * std::mem::size_of::<AtomicU64>()
    }
}
