use crate::ids::{OwnerKind, TenantId};
use std::fmt::Write as _;

/// Hex encoding used only for the small set of legacy-style, human-readable
/// tenant metadata keys. Canonical record, vector, text, and security state
/// remains under the binary tenant keyspace (`[TENANT_NS][tenant u32]/...`).
#[must_use]
pub fn tenant_hex(tenant: TenantId) -> String {
    let mut out = String::with_capacity(8);
    for byte in tenant.to_be_bytes() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Namespace byte of system rows (not tenant-scoped), storage format v3.
pub const SYSTEM_NS: u8 = 0x00;
/// Namespace byte of tenant-owned rows, storage format v3.
pub const TENANT_NS: u8 = 0x01;
/// Length of the tenant key prefix `[TENANT_NS][tenant u32 BE]`.
pub const TENANT_PREFIX_LEN: usize = 5;

/// System family: tenant scope digest → `(ordinal u32 BE, state u8)`.
pub const SYS_TENANT_BY_SCOPE: u8 = 0x02;
/// System family: tenant ordinal → `(scope digest, state u8)`.
pub const SYS_TENANT_BY_ORD: u8 = 0x03;
/// System family: next tenant ordinal to assign (`u32 BE`).
pub const SYS_TENANT_SEQ: u8 = 0x04;

/// The 5-byte key prefix owned by `tenant`.
#[must_use]
pub fn tenant_prefix(tenant: TenantId) -> [u8; TENANT_PREFIX_LEN] {
    let [a, b, c, d] = tenant.to_be_bytes();
    [TENANT_NS, a, b, c, d]
}

/// The tenant owning a tenant-namespace key, if `key` is one.
#[must_use]
pub fn parse_tenant(key: &[u8]) -> Option<TenantId> {
    if key.first() != Some(&TENANT_NS) {
        return None;
    }
    let bytes: [u8; 4] = key.get(1..TENANT_PREFIX_LEN)?.try_into().ok()?;
    Some(TenantId(u32::from_be_bytes(bytes)))
}

/// Directory row: scope digest → ordinal.
#[must_use]
pub fn tenant_by_scope_key(scope: &crate::ids::TenantScope) -> Vec<u8> {
    let mut out = Vec::with_capacity(34);
    out.extend_from_slice(&[SYSTEM_NS, SYS_TENANT_BY_SCOPE]);
    out.extend_from_slice(&scope.0);
    out
}

/// Directory row: ordinal → scope digest.
#[must_use]
pub fn tenant_by_ord_key(tenant: TenantId) -> Vec<u8> {
    let mut out = Vec::with_capacity(6);
    out.extend_from_slice(&[SYSTEM_NS, SYS_TENANT_BY_ORD]);
    out.extend_from_slice(&tenant.to_be_bytes());
    out
}

/// Prefix of every ordinal → scope directory row (tenant enumeration).
#[must_use]
pub fn tenant_by_ord_prefix() -> [u8; 2] {
    [SYSTEM_NS, SYS_TENANT_BY_ORD]
}

/// Directory row holding the next ordinal to assign.
#[must_use]
pub fn tenant_seq_key() -> [u8; 2] {
    [SYSTEM_NS, SYS_TENANT_SEQ]
}

/// Prefix for durable vector-index definitions owned by one tenant.
#[must_use]
pub fn vector_index_meta_prefix(tenant: TenantId) -> String {
    format!("vector/index/{}/", tenant_hex(tenant))
}

/// Prefix for durable ANN manifests owned by one tenant.
#[must_use]
pub fn ann_manifest_meta_prefix(tenant: TenantId) -> String {
    format!("vector/ann-manifest/{}/", tenant_hex(tenant))
}

/// Key segment for tenant metadata rows.
pub const P_META: &[u8] = b"meta";
/// Key segment for relationship rows by public id.
pub const P_REL: &[u8] = b"relationship";
/// Key segment for relationship rows by dense ordinal.
pub const P_REL_BY_ORD: &[u8] = b"relationship-ordinal";
/// Per-relationship captured endpoint labels (cascade-safe decrement source).
pub const P_REL_GROUP: &[u8] = b"rel-group";
/// Key segment for property-family signatures.
pub const P_FAMILY_SIG: &[u8] = b"property-family-signature";
/// Key segment for property-family rows.
pub const P_FAMILY: &[u8] = b"property-family";
/// Key segment for property-definition rows.
pub const P_DEFINITION: &[u8] = b"property-definition";
/// Build a tenant keyspace key: `[TENANT_NS][tenant u32 BE]` followed by
/// `/`-separated segments.
#[must_use]
pub fn key(tenant: TenantId, segments: &[&[u8]]) -> Vec<u8> {
    let mut out =
        Vec::with_capacity(TENANT_PREFIX_LEN + segments.iter().map(|s| s.len() + 1).sum::<usize>());
    out.extend_from_slice(&tenant_prefix(tenant));
    for s in segments {
        out.push(b'/');
        out.extend_from_slice(s);
    }
    out
}

/// Inclusive start / exclusive end range covering all keys under a prefix.
///
/// # Examples
///
/// ```
/// use rushdb_types::keys::prefix_range;
///
/// let (start, end) = prefix_range(b"ab");
/// assert_eq!(start, b"ab");
/// assert_eq!(end, b"ac");
/// ```
#[must_use]
pub fn prefix_range(prefix: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut end = prefix.to_vec();
    loop {
        match end.last_mut() {
            Some(last) if *last != 0xFF => {
                *last += 1;
                return (prefix.to_vec(), end);
            }
            Some(_) => {
                end.pop();
            }
            None => return (prefix.to_vec(), vec![]),
        }
    }
}

/// Big-endian 8-byte encoding of an ordinal, used as a key segment.
#[must_use]
pub fn u64be(v: u64) -> [u8; 8] {
    v.to_be_bytes()
}

/// Key prefix for every row owned by records or relationships.
#[must_use]
pub fn owner_prefix(kind: OwnerKind) -> &'static [u8] {
    match kind {
        OwnerKind::Record => b"record",
        OwnerKind::Relationship => b"rel",
    }
}

/// Key segment for embedding-space definitions.
pub const P_EMB_SPACE: &[u8] = b"emb-space";
/// Key segment for embedding-space alias mappings.
pub const P_EMB_ALIAS: &[u8] = b"emb-alias";
/// Key segment for embedding bindings.
pub const P_EMB_BINDING: &[u8] = b"emb-binding";
/// Key segment for stored embedding vectors.
pub const P_EMB_VECTOR: &[u8] = b"vec";

/// Key for an embedding-space definition by space id.
#[must_use]
pub fn emb_space_key(tenant: TenantId, sid: u64) -> Vec<u8> {
    key(tenant, &[P_EMB_SPACE, &u64be(sid)])
}

/// Key mapping an embedding-space alias to its space.
#[must_use]
pub fn emb_alias_key(tenant: TenantId, alias: &str) -> Vec<u8> {
    key(tenant, &[P_EMB_ALIAS, alias.as_bytes()])
}

/// Key for one embedding binding of an owner property element.
#[must_use]
pub fn emb_binding_key(
    tenant: TenantId,
    space: u64,
    kind: OwnerKind,
    owner_ord: u64,
    pid: u64,
    elem: u64,
) -> Vec<u8> {
    key(
        tenant,
        &[
            P_EMB_BINDING,
            &u64be(space),
            &[kind.as_byte()],
            &u64be(owner_ord),
            &u64be(pid),
            &u64be(elem),
        ],
    )
}

/// Prefix covering every embedding binding in a space.
#[must_use]
pub fn emb_bindings_prefix(tenant: TenantId, space: u64) -> Vec<u8> {
    key(tenant, &[P_EMB_BINDING, &u64be(space)])
}

/// Key for a stored embedding vector slot within a space.
#[must_use]
pub fn emb_vector_key(tenant: TenantId, space: u64, slot: u64) -> Vec<u8> {
    key(tenant, &[P_EMB_VECTOR, &u64be(space), &u64be(slot)])
}

/// Key segment for per-space binding versions.
pub const P_EMB_VERSION: &[u8] = b"emb-version";

/// Version row of a space's binding set: `prev_ci || ci` (16 bytes), the
/// commit that last changed the space's bindings and the one before it. A
/// cached ANN generation is current when its version equals `ci`, and an
/// incremental absorb of commit `ci` chains onto it when its version equals
/// `prev_ci` — independent of any other space or tenant's commits.
#[must_use]
pub fn emb_space_version_key(tenant: TenantId, space: u64) -> Vec<u8> {
    key(tenant, &[P_EMB_VERSION, &u64be(space)])
}

/// Encode a space version row.
#[must_use]
pub fn emb_space_version_value(prev_ci: u64, ci: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(16);
    out.extend_from_slice(&u64be(prev_ci));
    out.extend_from_slice(&u64be(ci));
    out
}

/// Decode a space version row; `None` for a malformed value.
#[must_use]
pub fn parse_emb_space_version(raw: &[u8]) -> Option<(u64, u64)> {
    let prev = u64::from_be_bytes(raw.get(..8)?.try_into().ok()?);
    let ci = u64::from_be_bytes(raw.get(8..16)?.try_into().ok()?);
    (raw.len() == 16).then_some((prev, ci))
}

/// Key segment for per-space ready-vector counts.
pub const P_EMB_READY: &[u8] = b"emb-ready";

/// Number of a space's bindings in `Ready` status, maintained by every
/// commit that writes a binding (big-endian `u64`). It is the pool a vector
/// search ranks, read in one `get` to choose exact or ANN search.
#[must_use]
pub fn emb_ready_count_key(tenant: TenantId, space: u64) -> Vec<u8> {
    key(tenant, &[P_EMB_READY, &u64be(space)])
}

/// Key segment for pending embedding-slot reclamations.
pub const P_EMB_RECLAIM: &[u8] = b"emb-reclaim";

/// Pending reclamation of a vector slot superseded at commit `after_ci`. The
/// slot's bytes may still be read by a pinned view until the GC watermark
/// passes `after_ci`; the reclamation sweep deletes them only then. Ordered by
/// `after_ci` so the sweep ranges straight to the reclaimable rows.
#[must_use]
pub fn emb_reclaim_key(tenant: TenantId, after_ci: u64, space: u64, slot: u64) -> Vec<u8> {
    key(
        tenant,
        &[P_EMB_RECLAIM, &u64be(after_ci), &u64be(space), &u64be(slot)],
    )
}

/// Prefix covering every pending embedding-slot reclamation for a tenant.
#[must_use]
pub fn emb_reclaim_prefix(tenant: TenantId) -> Vec<u8> {
    key(tenant, &[P_EMB_RECLAIM])
}

/// Value of a reclamation row: `after_ci || space || slot` (24 bytes), so the
/// sweep can recover the vector key to delete without re-parsing the key.
#[must_use]
pub fn emb_reclaim_value(after_ci: u64, space: u64, slot: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(24);
    out.extend_from_slice(&u64be(after_ci));
    out.extend_from_slice(&u64be(space));
    out.extend_from_slice(&u64be(slot));
    out
}

/// Decode a reclamation row value into `(after_ci, space, slot)`.
#[must_use]
pub fn parse_emb_reclaim_value(value: &[u8]) -> Option<(u64, u64, u64)> {
    if value.len() != 24 {
        return None;
    }
    let after_ci = u64::from_be_bytes(value[0..8].try_into().ok()?);
    let space = u64::from_be_bytes(value[8..16].try_into().ok()?);
    let slot = u64::from_be_bytes(value[16..24].try_into().ok()?);
    Some((after_ci, space, slot))
}

/// Legacy raw global generation key. Retained only for old-command replay and
/// migration detection; new writes use `security_generation_key(tenant)`.
pub const SECURITY_GENERATION_KEY: &str = "security/generation";

/// Legacy raw global policy key; new writes use `security_policy_key(tenant)`.
pub const SECURITY_POLICY_ACTIVE_KEY: &str = "security/policy/active";

/// Prefix covering all security rows of a tenant.
#[must_use]
pub fn security_prefix(tenant: TenantId) -> Vec<u8> {
    key(tenant, &[b"security", b""])
}

/// Key for a tenant's security generation watermark.
#[must_use]
pub fn security_generation_key(tenant: TenantId) -> Vec<u8> {
    key(tenant, &[b"security", b"generation"])
}

/// Key for a tenant's active security policy.
#[must_use]
pub fn security_policy_key(tenant: TenantId) -> Vec<u8> {
    key(tenant, &[b"security", b"policy", b"active"])
}

// ---- storage format v3 typed families -------------------------
//
// Typed keys are `[TENANT_NS][tenant u32 BE][family u8][fields…]`. Family tags
// are below `/` (0x2F), so they never collide with the `/`-separated generic
// keys built by [`key`]. Integers are fixed-width big-endian; a variable-length
// string that is not the last field is `u16 BE length + bytes`.

/// Typed family: public record id → record ordinal.
pub const F_RECORD_ID: u8 = 0x03;
/// Typed family: record ordinal → `(public id, label)` header.
pub const F_RECORD_HDR: u8 = 0x04;
/// Typed family: `(label, record ordinal)` label membership.
pub const F_LABEL: u8 = 0x05;
/// Typed family: derived counters (see the `C_*` sub-families).
pub const F_COUNTER: u8 = 0x09;
/// Typed family: per-`(kind, label, pid)` value dictionary with refcounts.
pub const F_VALUE_DICT: u8 = 0x0A;
/// Typed family: sticky array markers.
pub const F_MARKER: u8 = 0x11;
/// Typed family: one canonical occurrence per `(kind, owner, pid, elem)`.
pub const F_OCC: u8 = 0x12;
/// Typed family: `(kind, label, pid, nullness, owner)` presence.
pub const F_PRESENCE: u8 = 0x13;
/// Typed family: index id → [`crate::index::IndexDef`] (JSON).
pub const F_INDEX_DEF: u8 = 0x14;
/// Typed family: index name → index id.
pub const F_INDEX_NAME: u8 = 0x15;
/// Typed family (derived): `(index id, tag, payload, owner)` ordered entry.
pub const F_INDEX_ENTRY: u8 = 0x16;
/// Typed family: `(text index id, owner)` → commit index of the newest
/// change the text applier has not yet indexed (base rows).
pub const F_INDEX_PENDING: u8 = 0x17;
/// Typed family: the tenant's logical stored bytes (key + value lengths of
/// every live row under the tenant prefix except this one), maintained by the
/// storage backend in the batch that changes them.
pub const F_USAGE_BYTES: u8 = 0x18;

/// Coarse family name of any store key, for accounting and diagnostics:
/// tenant families by their family byte, legacy segment keys by their first
/// segment, and system/global keys by namespace. One classifier serves the
/// layout and read-accounting tools.
#[must_use]
pub fn family_label(key: &[u8]) -> String {
    match key.first() {
        Some(&TENANT_NS) if key.len() > TENANT_PREFIX_LEN => {
            let rest = &key[TENANT_PREFIX_LEN..];
            match rest[0] {
                F_RECORD_ID => "record-id".into(),
                F_RECORD_HDR => "record-header".into(),
                F_LABEL => "label-member".into(),
                F_COUNTER => format!("counter/{:02x}", rest.get(1).copied().unwrap_or(0)),
                F_VALUE_DICT => "value-dictionary".into(),
                F_MARKER => "marker".into(),
                F_OCC => "occurrence".into(),
                F_PRESENCE => "presence".into(),
                F_INDEX_DEF => "index-def".into(),
                F_INDEX_NAME => "index-name".into(),
                F_INDEX_ENTRY => "index-entry".into(),
                F_INDEX_PENDING => "index-pending".into(),
                F_USAGE_BYTES => "usage-bytes".into(),
                b'/' => {
                    let segment = rest[1..].split(|byte| *byte == b'/').next().unwrap_or(&[]);
                    format!("legacy/{}", String::from_utf8_lossy(segment))
                }
                other => format!("tenant-family/{other:02x}"),
            }
        }
        Some(&SYSTEM_NS) => format!("system/{:02x}", key.get(1).copied().unwrap_or(0)),
        _ => {
            let segment = key.split(|byte| *byte == b'/').next().unwrap_or(&[]);
            format!("global/{}", String::from_utf8_lossy(segment))
        }
    }
}

/// Human-readable rendering of a storage key for errors and logs: the
/// tenant, the typed family name, and the remainder in hex (legacy
/// `/`-separated keys keep their printable path).
#[must_use]
pub fn describe_key(key: &[u8]) -> String {
    use std::fmt::Write as _;
    let Some(tenant) = parse_tenant(key) else {
        return key.escape_ascii().to_string();
    };
    let rest = &key[TENANT_PREFIX_LEN..];
    let family = match rest.first() {
        Some(&F_RECORD_ID) => "record-id",
        Some(&F_RECORD_HDR) => "record-header",
        Some(&F_LABEL) => "label-member",
        Some(&F_COUNTER) => "counter",
        Some(&F_VALUE_DICT) => "value-dictionary",
        Some(&F_MARKER) => "marker",
        Some(&F_OCC) => "occurrence",
        Some(&F_PRESENCE) => "presence",
        Some(&F_INDEX_DEF) => "index-def",
        Some(&F_INDEX_NAME) => "index-name",
        Some(&F_INDEX_ENTRY) => "index-entry",
        Some(&F_INDEX_PENDING) => "index-pending",
        Some(&F_USAGE_BYTES) => "usage-bytes",
        _ => return format!("tenant {tenant} {}", rest.escape_ascii()),
    };
    let mut out = format!("tenant {tenant} {family} ");
    for byte in &rest[1..] {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Counter: live records per label.
pub const C_LABEL: u8 = 0x01;
/// Counter: owners carrying a pid (all labels).
pub const C_SUPPORT_TOTAL: u8 = 0x02;
/// Counter: owners carrying a pid, by `(kind, pid, label)`.
pub const C_SUPPORT_BY_PID: u8 = 0x03;
/// Counter: owners carrying a pid, by `(kind, label, pid)`.
pub const C_SUPPORT_BY_LABEL: u8 = 0x04;
/// Counter: explicit-null occurrences of a pid.
pub const C_NULL: u8 = 0x05;
/// Counter: adjacency degree of a node in one direction.
pub const C_DEGREE: u8 = 0x06;
/// Counter: distinct dictionary values of `(kind, label, pid)`.
pub const C_DISTINCT: u8 = 0x07;
/// Counter: relationships by `(type, source label, target label)`.
pub const C_REL_CATALOG: u8 = 0x08;
/// Counter: occurrences of `(kind, label, pid)` too long for the dictionary.
pub const C_LONG_VALUES: u8 = 0x09;
/// Counter: relationships of `(type, source label, target label)` carrying a
/// pid.
pub const C_REL_SHAPE_PROP: u8 = 0x0A;
/// Counter: tenant-wide usage totals, one row per `U_*` metric.
pub const C_USAGE: u8 = 0x0B;
/// Counter: occurrences of one long value of `(kind, label, pid)`, keyed by
/// its digest (`long_value_digest`): the value dictionary's refcount for
/// values too long for it.
pub const C_LONG_VALUE: u8 = 0x0C;
/// Counter: distinct long values of `(kind, label, pid)` (non-zero
/// `C_LONG_VALUE` rows), as `C_DISTINCT` is for the dictionary.
pub const C_LONG_DISTINCT: u8 = 0x0D;

/// Usage metric: live records.
pub const U_RECORDS: u8 = 0x01;
/// Usage metric: live relationships.
pub const U_RELATIONSHIPS: u8 = 0x02;
/// Usage metric: labels with at least one live record.
pub const U_LABELS: u8 = 0x03;
/// Usage metric: property ids carried by at least one live record.
pub const U_PROPERTIES: u8 = 0x04;

/// Marker: array-holding owners of a pid under a label (a count).
pub const M_ARRAY_LABEL: u8 = 0x02;

/// Presence nullness: the owner has at least one non-null value for the pid.
pub const NULLNESS_VALUE: u8 = 0;
/// Presence nullness: every value the owner has for the pid is null.
pub const NULLNESS_NULL: u8 = 1;

/// Longest encoded `(tag + payload)` value that enters the value dictionary
/// (DC10 refinement): longer values are counted in [`C_LONG_VALUES`] instead.
pub const DICT_MAX_VALUE_BYTES: usize = 96;

fn typed(tenant: TenantId, family: u8, capacity: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(TENANT_PREFIX_LEN + 1 + capacity);
    out.extend_from_slice(&tenant_prefix(tenant));
    out.push(family);
    out
}

/// Append a `u16 BE length + bytes` string field.
///
/// # Panics
/// Panics if the string is longer than `u16::MAX` bytes (labels and
/// relationship types are bounded far below that at the protocol edge).
pub fn push_str(out: &mut Vec<u8>, value: &str) {
    let len = u16::try_from(value.len()).expect("key string field fits u16");
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(value.as_bytes());
}

/// Read a `u16 BE length + bytes` string field at `pos`; returns the string
/// and the position after it.
#[must_use]
pub fn read_str(key: &[u8], pos: usize) -> Option<(&str, usize)> {
    let len = usize::from(u16::from_be_bytes(key.get(pos..pos + 2)?.try_into().ok()?));
    let start = pos + 2;
    let text = std::str::from_utf8(key.get(start..start + len)?).ok()?;
    Some((text, start + len))
}

/// Read a big-endian `u64` at `pos`.
#[must_use]
pub fn read_u64(key: &[u8], pos: usize) -> Option<u64> {
    Some(u64::from_be_bytes(key.get(pos..pos + 8)?.try_into().ok()?))
}

/// Append `value` as an order-preserving variable-length integer: one length
/// byte (0–8), then the value's big-endian bytes without leading zeros
/// (`0` is the lone byte `0x00`). A shorter encoding is a smaller number, so
/// byte order equals numeric order, and the length byte makes the field
/// self-delimiting, so a key prefix ending in it never matches a longer value
/// (lever C: owner ordinals and property ids are mostly 1–4 bytes).
pub fn push_ord(out: &mut Vec<u8>, value: u64) {
    let bytes = value.to_be_bytes();
    let skip = usize::try_from(value.leading_zeros() / 8).unwrap_or(8);
    out.push(u8::try_from(8 - skip).unwrap_or(8));
    out.extend_from_slice(&bytes[skip..]);
}

/// Read an integer written by [`push_ord`] at `pos`; returns the value and
/// the position after it. Non-minimal encodings (a leading zero byte, or a
/// length over 8) are rejected so every value has exactly one key.
#[must_use]
pub fn read_ord(key: &[u8], pos: usize) -> Option<(u64, usize)> {
    let len = usize::from(*key.get(pos)?);
    if len > 8 {
        return None;
    }
    let digits = key.get(pos + 1..pos + 1 + len)?;
    if digits.first() == Some(&0) {
        return None;
    }
    let mut buf = [0u8; 8];
    buf[8 - len..].copy_from_slice(digits);
    Some((u64::from_be_bytes(buf), pos + 1 + len))
}

/// Read a [`push_ord`] integer that must end `key` exactly at `pos`.
#[must_use]
pub fn read_ord_tail(key: &[u8], pos: usize) -> Option<u64> {
    let (value, end) = read_ord(key, pos)?;
    (end == key.len()).then_some(value)
}

/// `RECORD_ID`: public id → ordinal.
#[must_use]
pub fn record_id_key(tenant: TenantId, id: &[u8; 16]) -> Vec<u8> {
    let mut out = record_id_prefix(tenant);
    out.extend_from_slice(id);
    out
}

/// Prefix of every record identity row of a tenant.
#[must_use]
pub fn record_id_prefix(tenant: TenantId) -> Vec<u8> {
    typed(tenant, F_RECORD_ID, 16)
}

/// `RECORD_HDR`: ordinal → header.
#[must_use]
pub fn record_hdr_key(tenant: TenantId, ordinal: u64) -> Vec<u8> {
    let mut out = typed(tenant, F_RECORD_HDR, 9);
    push_ord(&mut out, ordinal);
    out
}

/// The record ordinal of a `RECORD_HDR` key.
#[must_use]
pub fn parse_record_hdr_key(key: &[u8]) -> Option<u64> {
    if key.get(TENANT_PREFIX_LEN) != Some(&F_RECORD_HDR) {
        return None;
    }
    read_ord_tail(key, TENANT_PREFIX_LEN + 1)
}

/// Prefix of every record header of a tenant.
#[must_use]
pub fn record_hdr_prefix(tenant: TenantId) -> Vec<u8> {
    typed(tenant, F_RECORD_HDR, 0)
}

/// Encode a record header value: `id (16) || label`.
#[must_use]
pub fn encode_record_hdr(id: &[u8; 16], label: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + label.len());
    out.extend_from_slice(id);
    out.extend_from_slice(label.as_bytes());
    out
}

/// Decode a record header value into `(id, label)`.
#[must_use]
pub fn decode_record_hdr(raw: &[u8]) -> Option<([u8; 16], &str)> {
    let id: [u8; 16] = raw.get(..16)?.try_into().ok()?;
    Some((id, std::str::from_utf8(raw.get(16..)?).ok()?))
}

/// `LABEL`: membership row of a record ordinal in a label.
#[must_use]
pub fn label_member_key(tenant: TenantId, label: &str, ordinal: u64) -> Vec<u8> {
    let mut out = label_member_prefix(tenant, label);
    push_ord(&mut out, ordinal);
    out
}

/// Prefix of every member of one label.
#[must_use]
pub fn label_member_prefix(tenant: TenantId, label: &str) -> Vec<u8> {
    let mut out = typed(tenant, F_LABEL, 2 + label.len() + 9);
    push_str(&mut out, label);
    out
}

/// Prefix of every label-membership row of a tenant.
#[must_use]
pub fn label_members_prefix(tenant: TenantId) -> Vec<u8> {
    typed(tenant, F_LABEL, 0)
}

/// Parse a membership key into `(label, ordinal)`.
#[must_use]
pub fn parse_label_member_key(key: &[u8]) -> Option<(&str, u64)> {
    if key.get(TENANT_PREFIX_LEN) != Some(&F_LABEL) {
        return None;
    }
    let (label, pos) = read_str(key, TENANT_PREFIX_LEN + 1)?;
    Some((label, read_ord_tail(key, pos)?))
}

/// `OCC`: the canonical occurrence of one `(kind, owner, pid, elem)`.
#[must_use]
pub fn occ_key(tenant: TenantId, kind: OwnerKind, owner: u64, pid: u64, elem: u64) -> Vec<u8> {
    let mut out = occ_pid_prefix(tenant, kind, owner, pid);
    push_ord(&mut out, elem);
    out
}

/// Prefix of every occurrence of one owner.
#[must_use]
pub fn occ_owner_prefix(tenant: TenantId, kind: OwnerKind, owner: u64) -> Vec<u8> {
    let mut out = typed(tenant, F_OCC, 1 + 9 + 9 + 9);
    out.push(kind.as_byte());
    push_ord(&mut out, owner);
    out
}

/// Prefix of every element of one owner's pid.
#[must_use]
pub fn occ_pid_prefix(tenant: TenantId, kind: OwnerKind, owner: u64, pid: u64) -> Vec<u8> {
    let mut out = occ_owner_prefix(tenant, kind, owner);
    push_ord(&mut out, pid);
    out
}

/// Prefix of every occurrence of one owner kind in a tenant.
#[must_use]
pub fn occ_kind_prefix(tenant: TenantId, kind: OwnerKind) -> Vec<u8> {
    let mut out = typed(tenant, F_OCC, 1);
    out.push(kind.as_byte());
    out
}

/// Parse an occurrence key into `(tenant, kind, owner, pid, elem)`.
#[must_use]
pub fn parse_occ_key(key: &[u8]) -> Option<(TenantId, OwnerKind, u64, u64, u64)> {
    let tenant = parse_tenant(key)?;
    if key.get(TENANT_PREFIX_LEN) != Some(&F_OCC) {
        return None;
    }
    let kind = OwnerKind::from_byte(*key.get(TENANT_PREFIX_LEN + 1)?)?;
    let (owner, pos) = read_ord(key, TENANT_PREFIX_LEN + 2)?;
    let (pid, pos) = read_ord(key, pos)?;
    let elem = read_ord_tail(key, pos)?;
    Some((tenant, kind, owner, pid, elem))
}

/// `PRESENCE`: an owner carries `pid` (with the given nullness) under a label.
#[must_use]
pub fn presence_key(
    tenant: TenantId,
    kind: OwnerKind,
    label: &str,
    pid: u64,
    nullness: u8,
    owner: u64,
) -> Vec<u8> {
    let mut out = presence_nullness_prefix(tenant, kind, label, pid, nullness);
    push_ord(&mut out, owner);
    out
}

/// Prefix of the presence rows of `(kind, label, pid)` with one nullness.
#[must_use]
pub fn presence_nullness_prefix(
    tenant: TenantId,
    kind: OwnerKind,
    label: &str,
    pid: u64,
    nullness: u8,
) -> Vec<u8> {
    let mut out = presence_pid_prefix(tenant, kind, label, pid);
    out.push(nullness);
    out
}

/// Prefix of every presence row of `(kind, label, pid)`.
#[must_use]
pub fn presence_pid_prefix(tenant: TenantId, kind: OwnerKind, label: &str, pid: u64) -> Vec<u8> {
    let mut out = typed(tenant, F_PRESENCE, 1 + 2 + label.len() + 9 + 1 + 9);
    out.push(kind.as_byte());
    push_str(&mut out, label);
    push_ord(&mut out, pid);
    out
}

/// Parse a presence key into `(kind, label, pid, nullness, owner)`.
#[must_use]
pub fn parse_presence_key(key: &[u8]) -> Option<(OwnerKind, &str, u64, u8, u64)> {
    if key.get(TENANT_PREFIX_LEN) != Some(&F_PRESENCE) {
        return None;
    }
    let kind = OwnerKind::from_byte(*key.get(TENANT_PREFIX_LEN + 1)?)?;
    let (label, pos) = read_str(key, TENANT_PREFIX_LEN + 2)?;
    let (pid, pos) = read_ord(key, pos)?;
    let nullness = *key.get(pos)?;
    let owner = read_ord_tail(key, pos + 1)?;
    Some((kind, label, pid, nullness, owner))
}

/// `VALUE_DICT`: refcount of one `(tag || payload)` value of `(kind, label, pid)`.
#[must_use]
pub fn value_dict_key(
    tenant: TenantId,
    kind: OwnerKind,
    label: &str,
    pid: u64,
    value: &[u8],
) -> Vec<u8> {
    let mut out = value_dict_prefix(tenant, kind, label, pid);
    out.extend_from_slice(value);
    out
}

/// Prefix of the value dictionary of `(kind, label, pid)`; the remainder of a
/// key under it is the `(tag || order-preserving payload)` value.
#[must_use]
pub fn value_dict_prefix(tenant: TenantId, kind: OwnerKind, label: &str, pid: u64) -> Vec<u8> {
    let mut out = typed(
        tenant,
        F_VALUE_DICT,
        1 + 2 + label.len() + 9 + DICT_MAX_VALUE_BYTES,
    );
    out.push(kind.as_byte());
    push_str(&mut out, label);
    push_ord(&mut out, pid);
    out
}

fn counter(tenant: TenantId, sub: u8, capacity: usize) -> Vec<u8> {
    let mut out = typed(tenant, F_COUNTER, 1 + capacity);
    out.push(sub);
    out
}

/// Prefix of one counter sub-family.
#[must_use]
pub fn counter_prefix(tenant: TenantId, sub: u8) -> Vec<u8> {
    counter(tenant, sub, 0)
}

/// Counter: one tenant-wide usage metric (`U_*`).
#[must_use]
pub fn usage_count_key(tenant: TenantId, metric: u8) -> Vec<u8> {
    let mut out = counter(tenant, C_USAGE, 1);
    out.push(metric);
    out
}

/// The tenant's logical stored bytes row (see [`F_USAGE_BYTES`]).
#[must_use]
pub fn usage_bytes_key(tenant: TenantId) -> Vec<u8> {
    typed(tenant, F_USAGE_BYTES, 0)
}

/// Counter: live records of a label.
#[must_use]
pub fn label_count_key(tenant: TenantId, label: &str) -> Vec<u8> {
    let mut out = counter(tenant, C_LABEL, 2 + label.len());
    push_str(&mut out, label);
    out
}

/// Counter: owners of `kind` carrying `pid`.
#[must_use]
pub fn support_total_key(tenant: TenantId, kind: OwnerKind, pid: u64) -> Vec<u8> {
    let mut out = counter(tenant, C_SUPPORT_TOTAL, 9);
    out.push(kind.as_byte());
    out.extend_from_slice(&u64be(pid));
    out
}

/// Counter: owners of `kind` under `label` carrying `pid` (pid-major order).
#[must_use]
pub fn support_by_pid_key(tenant: TenantId, kind: OwnerKind, pid: u64, label: &str) -> Vec<u8> {
    let mut out = support_by_pid_prefix(tenant, kind, pid);
    push_str(&mut out, label);
    out
}

/// Prefix of the per-label support counters of one pid.
#[must_use]
pub fn support_by_pid_prefix(tenant: TenantId, kind: OwnerKind, pid: u64) -> Vec<u8> {
    let mut out = counter(tenant, C_SUPPORT_BY_PID, 9 + 2 + 32);
    out.push(kind.as_byte());
    out.extend_from_slice(&u64be(pid));
    out
}

/// Counter: owners of `kind` under `label` carrying `pid` (label-major order).
#[must_use]
pub fn support_by_label_key(tenant: TenantId, kind: OwnerKind, label: &str, pid: u64) -> Vec<u8> {
    let mut out = support_by_label_prefix(tenant, kind, label);
    out.extend_from_slice(&u64be(pid));
    out
}

/// Prefix of the per-pid support counters of one label.
#[must_use]
pub fn support_by_label_prefix(tenant: TenantId, kind: OwnerKind, label: &str) -> Vec<u8> {
    let mut out = counter(tenant, C_SUPPORT_BY_LABEL, 1 + 2 + label.len() + 8);
    out.push(kind.as_byte());
    push_str(&mut out, label);
    out
}

/// Counter: explicit-null occurrences of `(kind, pid)`.
#[must_use]
pub fn null_count_key(tenant: TenantId, kind: OwnerKind, pid: u64) -> Vec<u8> {
    let mut out = counter(tenant, C_NULL, 9);
    out.push(kind.as_byte());
    out.extend_from_slice(&u64be(pid));
    out
}

/// Counter: degree of `node` in one direction (`true` = outgoing).
#[must_use]
pub fn degree_key(tenant: TenantId, node: u64, outgoing: bool) -> Vec<u8> {
    let mut out = counter(tenant, C_DEGREE, 9);
    out.extend_from_slice(&u64be(node));
    out.push(u8::from(outgoing));
    out
}

/// Counter: distinct dictionary values of `(kind, label, pid)`.
#[must_use]
pub fn distinct_key(tenant: TenantId, kind: OwnerKind, label: &str, pid: u64) -> Vec<u8> {
    let mut out = counter(tenant, C_DISTINCT, 1 + 2 + label.len() + 8);
    out.push(kind.as_byte());
    push_str(&mut out, label);
    out.extend_from_slice(&u64be(pid));
    out
}

/// Counter: occurrences of `(kind, label, pid)` too long for the dictionary.
#[must_use]
pub fn long_values_key(tenant: TenantId, kind: OwnerKind, label: &str, pid: u64) -> Vec<u8> {
    let mut out = counter(tenant, C_LONG_VALUES, 1 + 2 + label.len() + 8);
    out.push(kind.as_byte());
    push_str(&mut out, label);
    out.extend_from_slice(&u64be(pid));
    out
}

/// Bytes of a long value's digest in `long_value_key`.
pub const LONG_DIGEST_BYTES: usize = 16;

/// Digest of a long dictionary entry (`tag || payload`): the first
/// `LONG_DIGEST_BYTES` of its SHA-256.
#[must_use]
pub fn long_value_digest(entry: &[u8]) -> [u8; LONG_DIGEST_BYTES] {
    use sha2::Digest as _;
    let full: [u8; 32] = sha2::Sha256::digest(entry).into();
    let mut out = [0u8; LONG_DIGEST_BYTES];
    out.copy_from_slice(&full[..LONG_DIGEST_BYTES]);
    out
}

/// Prefix of the long-value refcounts of `(kind, label, pid)`; the remainder
/// of a key under it is a `long_value_digest`.
#[must_use]
pub fn long_value_prefix(tenant: TenantId, kind: OwnerKind, label: &str, pid: u64) -> Vec<u8> {
    let mut out = counter(
        tenant,
        C_LONG_VALUE,
        1 + 2 + label.len() + 8 + LONG_DIGEST_BYTES,
    );
    out.push(kind.as_byte());
    push_str(&mut out, label);
    out.extend_from_slice(&u64be(pid));
    out
}

/// Counter: occurrences of the long value with `digest` of `(kind, label, pid)`.
#[must_use]
pub fn long_value_key(
    tenant: TenantId,
    kind: OwnerKind,
    label: &str,
    pid: u64,
    digest: &[u8; LONG_DIGEST_BYTES],
) -> Vec<u8> {
    let mut out = long_value_prefix(tenant, kind, label, pid);
    out.extend_from_slice(digest);
    out
}

/// Counter: distinct long values of `(kind, label, pid)`.
#[must_use]
pub fn long_distinct_key(tenant: TenantId, kind: OwnerKind, label: &str, pid: u64) -> Vec<u8> {
    let mut out = counter(tenant, C_LONG_DISTINCT, 1 + 2 + label.len() + 8);
    out.push(kind.as_byte());
    push_str(&mut out, label);
    out.extend_from_slice(&u64be(pid));
    out
}

/// Counter: relationships of `(type, source label, target label)`.
#[must_use]
pub fn rel_catalog_key(tenant: TenantId, rel_type: &str, source: &str, target: &str) -> Vec<u8> {
    let mut out = counter(
        tenant,
        C_REL_CATALOG,
        6 + rel_type.len() + source.len() + target.len(),
    );
    push_str(&mut out, rel_type);
    push_str(&mut out, source);
    push_str(&mut out, target);
    out
}

/// Counter: relationships of `(type, source label, target label)` that carry
/// `pid` (creation-time endpoint labels, like [`rel_catalog_key`]).
#[must_use]
pub fn rel_shape_prop_key(
    tenant: TenantId,
    (rel_type, source, target): (&str, &str, &str),
    pid: u64,
) -> Vec<u8> {
    let mut out = rel_shape_prop_prefix(tenant, rel_type, source, target);
    out.extend_from_slice(&u64be(pid));
    out
}

/// Prefix of the per-pid counters of one relationship shape.
#[must_use]
pub fn rel_shape_prop_prefix(
    tenant: TenantId,
    rel_type: &str,
    source: &str,
    target: &str,
) -> Vec<u8> {
    let mut out = counter(
        tenant,
        C_REL_SHAPE_PROP,
        6 + rel_type.len() + source.len() + target.len() + 8,
    );
    push_str(&mut out, rel_type);
    push_str(&mut out, source);
    push_str(&mut out, target);
    out
}

/// Marker: owners of `kind` under `label` holding `pid` as an array; the
/// value is their count (the row is absent at zero).
#[must_use]
pub fn array_label_marker_key(tenant: TenantId, kind: OwnerKind, pid: u64, label: &str) -> Vec<u8> {
    let mut out = array_label_marker_prefix(tenant, kind, pid);
    push_str(&mut out, label);
    out
}

/// Prefix of the per-label array markers of one pid.
#[must_use]
pub fn array_label_marker_prefix(tenant: TenantId, kind: OwnerKind, pid: u64) -> Vec<u8> {
    let mut out = typed(tenant, F_MARKER, 10 + 2 + 32);
    out.push(M_ARRAY_LABEL);
    out.push(kind.as_byte());
    out.extend_from_slice(&u64be(pid));
    out
}

/// Decode a big-endian `u64` counter value (absent → 0).
///
/// # Errors
/// Returns `None` from the outer option for a malformed value.
#[must_use]
pub fn decode_counter(raw: Option<&[u8]>) -> Option<u64> {
    match raw {
        None => Some(0),
        Some(raw) => raw.try_into().ok().map(u64::from_be_bytes),
    }
}

// ---- declared indexes ----------------------------------------------

/// Key of an index definition.
#[must_use]
pub fn index_def_key(tenant: TenantId, id: u32) -> Vec<u8> {
    let mut out = typed(tenant, F_INDEX_DEF, 4);
    out.extend_from_slice(&id.to_be_bytes());
    out
}

/// Prefix of every index definition of a tenant.
#[must_use]
pub fn index_def_prefix(tenant: TenantId) -> Vec<u8> {
    typed(tenant, F_INDEX_DEF, 0)
}

/// Key mapping an index name to its id.
#[must_use]
pub fn index_name_key(tenant: TenantId, name: &str) -> Vec<u8> {
    let mut out = typed(tenant, F_INDEX_NAME, name.len());
    out.extend_from_slice(name.as_bytes());
    out
}

/// Prefix of one index's entries.
#[must_use]
pub fn index_entry_prefix(tenant: TenantId, id: u32) -> Vec<u8> {
    let mut out = typed(tenant, F_INDEX_ENTRY, 4 + 16 + 8);
    out.extend_from_slice(&id.to_be_bytes());
    out
}

/// Key of one ordered entry: `entry` is `tag ‖ order-preserving payload`.
#[must_use]
pub fn index_entry_key(tenant: TenantId, id: u32, entry: &[u8], owner: u64) -> Vec<u8> {
    let mut out = index_entry_prefix(tenant, id);
    out.extend_from_slice(entry);
    out.extend_from_slice(&u64be(owner));
    out
}

/// `(entry, owner)` of an index entry key under `prefix`
/// ([`index_entry_prefix`]).
#[must_use]
pub fn parse_index_entry_key<'a>(key: &'a [u8], prefix: &[u8]) -> Option<(&'a [u8], u64)> {
    let rest = key.strip_prefix(prefix)?;
    let split = rest.len().checked_sub(8)?;
    let (entry, owner) = rest.split_at(split);
    Some((entry, u64::from_be_bytes(owner.try_into().ok()?)))
}

/// Prefix of one index's pending rows (owners its applier has yet to
/// process: text and structural indexes).
#[must_use]
pub fn index_pending_prefix(tenant: TenantId, index: u32) -> Vec<u8> {
    let mut out = typed(tenant, F_INDEX_PENDING, 4 + 8);
    out.extend_from_slice(&index.to_be_bytes());
    out
}

/// Pending row of one owner of an applied index.
#[must_use]
pub fn index_pending_key(tenant: TenantId, index: u32, owner: u64) -> Vec<u8> {
    let mut out = index_pending_prefix(tenant, index);
    out.extend_from_slice(&u64be(owner));
    out
}

#[cfg(test)]
mod ord_tests {
    use super::*;

    fn edge_values() -> Vec<u64> {
        let mut values = vec![0, 1, u64::MAX, u64::MAX - 1];
        for shift in (8..64).step_by(8) {
            let boundary = 1u64 << shift;
            values.extend([boundary - 1, boundary, boundary + 1]);
        }
        values.extend((0..2_000u64).map(|i| i.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> (i % 64)));
        values
    }

    fn encode(value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        push_ord(&mut out, value);
        out
    }

    #[test]
    fn ord_round_trips_with_minimal_length() {
        for value in edge_values() {
            let bytes = encode(value);
            assert_eq!(read_ord_tail(&bytes, 0), Some(value), "{value}");
            let digits = 8 - usize::try_from(value.leading_zeros() / 8).unwrap();
            assert_eq!(bytes.len(), 1 + digits, "{value}");
        }
        assert_eq!(encode(0), vec![0]);
        assert_eq!(encode(200_000), vec![3, 0x03, 0x0d, 0x40]);
    }

    #[test]
    fn ord_byte_order_is_numeric_order() {
        let mut values = edge_values();
        values.sort_unstable();
        values.dedup();
        for pair in values.windows(2) {
            assert!(
                encode(pair[0]) < encode(pair[1]),
                "{} vs {}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn non_minimal_or_oversized_encodings_are_rejected() {
        assert_eq!(read_ord(&[1, 0], 0), None);
        assert_eq!(read_ord(&[2, 0, 5], 0), None);
        assert_eq!(read_ord(&[9, 1, 2, 3, 4, 5, 6, 7, 8, 9], 0), None);
        assert_eq!(read_ord(&[3, 1, 2], 0), None);
        assert_eq!(read_ord_tail(&[1, 5, 0], 0), None);
    }

    #[test]
    fn one_owner_prefix_never_matches_another_owners_keys() {
        let tenant = TenantId(7);
        for (a, b) in [(1u64, 257u64), (0, 1), (255, 65_535), (1, 256)] {
            let prefix = occ_owner_prefix(tenant, OwnerKind::Record, a);
            let other = occ_key(tenant, OwnerKind::Record, b, 3, 0);
            assert!(!other.starts_with(&prefix), "{a} prefix matched owner {b}");
            let own = occ_key(tenant, OwnerKind::Record, a, 3, 0);
            assert_eq!(
                parse_occ_key(&own),
                Some((tenant, OwnerKind::Record, a, 3, 0))
            );
        }
        let presence = presence_key(tenant, OwnerKind::Record, "User", 42, NULLNESS_VALUE, 300);
        assert_eq!(
            parse_presence_key(&presence),
            Some((OwnerKind::Record, "User", 42, NULLNESS_VALUE, 300))
        );
        let member = label_member_key(tenant, "User", 70_000);
        assert_eq!(parse_label_member_key(&member), Some(("User", 70_000)));
        assert_eq!(parse_record_hdr_key(&record_hdr_key(tenant, 9)), Some(9));
    }
}
