use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Condvar, Mutex};

use rushdb_types::command::{CommitBatch, IsolationMode, COMMAND_VERSION};
use rushdb_types::ids::{OwnerKind, TenantId};
use rushdb_types::keys;
use uuid::Uuid;

use super::deps::{deps_of_key, family, Family};
use super::*;
use crate::{CommitOutcome, Store};

/// Per-test build behaviour, keyed by the probe's row (tests run in
/// parallel, so nothing global is shared between them).
#[derive(Default)]
struct Hook {
    /// Builds wait here until the test opens the gate.
    gate: Option<Arc<(Mutex<bool>, Condvar)>>,
    /// Refuse the build part-way through.
    refuse: bool,
    builds: Arc<AtomicU64>,
}

static HOOKS: Mutex<Option<HashMap<Vec<u8>, Hook>>> = Mutex::new(None);

fn hook(row: &[u8], make: impl FnOnce(&mut Hook)) -> Arc<AtomicU64> {
    let mut hooks = HOOKS.lock().unwrap();
    let hook = hooks
        .get_or_insert_with(HashMap::new)
        .entry(row.to_vec())
        .or_default();
    make(hook);
    Arc::clone(&hook.builds)
}

/// A layer that keeps one stored row, with the deps its key names.
#[derive(Default)]
struct Probe;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    row: Vec<u8>,
    deps: Vec<Dep>,
}

impl Resident for Probe {
    type Key = Key;
    type Value = Option<Vec<u8>>;
    const NAME: &'static str = "probe";
    const BUDGET: Budget = Budget::Columns;

    fn deps(&self, key: &Key, _view: &ReadView) -> Result<Vec<Dep>, StorageError> {
        Ok(key.deps.clone())
    }

    fn build(
        &self,
        key: &Key,
        view: &ReadView,
        cx: &BuildCx,
    ) -> Result<Option<Self::Value>, BuildError> {
        let (gate, refuse, builds) = {
            let hooks = HOOKS.lock().unwrap();
            hooks.as_ref().and_then(|hooks| hooks.get(&key.row)).map_or(
                (None, false, None),
                |hook| {
                    (
                        hook.gate.clone(),
                        hook.refuse,
                        Some(Arc::clone(&hook.builds)),
                    )
                },
            )
        };
        if let Some(builds) = builds {
            builds.fetch_add(1, Ordering::SeqCst);
        }
        if let Some(gate) = gate {
            let (open, cv) = &*gate;
            let mut open = open.lock().unwrap();
            while !*open {
                open = cv.wait(open).unwrap();
            }
        }
        if refuse {
            cx.admit(usize::MAX)?;
        }
        Ok(Some(view.get(&key.row)?))
    }

    fn bytes(value: &Self::Value) -> usize {
        8 + value.as_ref().map_or(0, Vec::len)
    }
}

const T1: TenantId = TenantId(1);
const T2: TenantId = TenantId(2);

fn row(tenant: TenantId, pid: u64) -> Vec<u8> {
    keys::occ_key(tenant, OwnerKind::Record, 1, pid, 0)
}

fn probe(tenant: TenantId, pid: u64) -> Key {
    Key {
        row: row(tenant, pid),
        deps: vec![Dep::Pid(tenant, pid)],
    }
}

type Rows = Arc<Mutex<Vec<(Vec<u8>, Vec<u8>)>>>;

/// Rows the next commit of each store writes (its expander drains them),
/// keyed by the store's address.
static QUEUES: Mutex<Option<HashMap<usize, Rows>>> = Mutex::new(None);

fn queue(store: &Store) -> Rows {
    let mut queues = QUEUES.lock().unwrap();
    Arc::clone(
        queues
            .get_or_insert_with(HashMap::new)
            .entry(std::ptr::from_ref(store) as usize)
            .or_default(),
    )
}

/// Make `store`'s commits write the rows [`write`] queues.
fn install_writer(store: &Store) {
    let rows = queue(store);
    store.set_expander(Arc::new(Box::new(move |_, _, txn| {
        for (key, value) in rows.lock().unwrap().drain(..) {
            txn.put(key, value);
        }
        Ok(())
    })));
}

fn store_with(config: StorageResources) -> Arc<Store> {
    let store = Arc::new(Store::new_with_resources(config).unwrap());
    install_writer(&store);
    store.attach_residents();
    // These tests are about the framework itself: on in both suite modes.
    store.residents().set_enabled(true);
    store
}

fn store() -> Arc<Store> {
    store_with(StorageResources::default())
}

fn batch(tenant: TenantId) -> CommitBatch {
    CommitBatch {
        command_version: COMMAND_VERSION,
        tenant,
        transaction_id: "resident-test".into(),
        commit_key: Uuid::now_v7(),
        start_index: 0,
        isolation: IsolationMode::ReadCommitted,
        preconditions: vec![],
        mutations: vec![],
    }
}

/// Commit `rows` and return the commit index.
fn write(store: &Store, rows: &[(Vec<u8>, &[u8])]) -> u64 {
    let batch = batch(T1);
    queue(store).lock().unwrap().extend(
        rows.iter()
            .map(|(key, value)| (key.clone(), value.to_vec())),
    );
    match store
        .commit(batch.commit_key, |ci, _| Ok((batch.clone(), ci)))
        .unwrap()
    {
        CommitOutcome::Committed { payload, .. } => payload,
        CommitOutcome::Conflicted(reason) => panic!("conflict: {reason}"),
    }
}

/// Offer what storage says for `key` at a fresh view; return that view.
fn offer(store: &Store, key: &Key) -> ReadView {
    let view = store.read_view().unwrap();
    let value = view.get(&key.row).unwrap();
    store
        .residents()
        .layer::<Probe>()
        .offer(key.clone(), &view, Arc::new(value));
    view
}

/// What the layer serves for `key` at `view`: `None` = a miss, else the
/// stored row it holds (itself optional).
type Served = Option<Option<Vec<u8>>>;

fn get(store: &Store, key: &Key, view: &ReadView) -> Served {
    store
        .residents()
        .layer::<Probe>()
        .get(key, view)
        .map(|value| (*value).clone())
}

fn snapshot(store: &Store) -> LayerSnapshot {
    store
        .residents()
        .snapshots()
        .into_iter()
        .find(|layer| layer.layer == "probe")
        .unwrap_or_default()
}

#[test]
fn an_entry_survives_writes_to_other_deps_and_other_tenants() {
    let store = store();
    let mine = probe(T1, 1010);
    write(&store, &[(mine.row.clone(), b"a")]);
    let at = offer(&store, &mine);
    write(&store, &[(row(T1, 8), b"other pid")]);
    write(&store, &[(row(T2, 7), b"other tenant")]);
    let now = store.read_view().unwrap();
    assert_eq!(get(&store, &mine, &now), Some(Some(b"a".to_vec())));
    // A reader still pinned at the entry's snapshot hits too.
    assert_eq!(get(&store, &mine, &at), Some(Some(b"a".to_vec())));
}

#[test]
fn a_write_to_a_dep_makes_the_entry_a_miss_also_for_older_readers() {
    for dep in [
        Dep::Index(T1, 3),
        Dep::Pid(T1, 7),
        Dep::Label(T1, label_hash("Trip")),
        Dep::ValueDict(T1, 7),
        Dep::Definitions(T1),
        Dep::Labels(T1),
        Dep::Adjacency(T1),
        Dep::Space(T1, 4),
        Dep::Policy(T1),
        Dep::TenantDir,
    ] {
        let store = store();
        let key = Key {
            row: row(T1, 1020),
            deps: vec![dep],
        };
        let before = offer(&store, &key);
        assert!(get(&store, &key, &before).is_some(), "{dep:?}: kept");
        store
            .residents()
            .note_writes(store.committed_version() + 1, [sample_key(dep).as_slice()]);
        assert!(
            get(&store, &key, &before).is_none(),
            "{dep:?}: older reader misses"
        );
        assert_eq!(snapshot(&store).invalidations, 1, "{dep:?}");
    }
}

/// A key whose write changes `dep`.
fn sample_key(dep: Dep) -> Vec<u8> {
    match dep {
        Dep::Index(t, id) => keys::index_entry_key(t, id, b"x", 1),
        Dep::Pid(t, pid) => keys::occ_key(t, OwnerKind::Record, 9, pid, 0),
        Dep::Label(t, _) => keys::rederive_key(t, "Trip"),
        Dep::ValueDict(t, pid) => keys::value_dict_key(t, OwnerKind::Record, "Trip", pid, b"v"),
        Dep::LabelProperties(t, _) => keys::presence_key(t, OwnerKind::Record, "Trip", 7, 0, 9),
        Dep::Definitions(t) => keys::key(t, &[keys::P_FAMILY, b"f"]),
        Dep::Labels(t) => keys::label_count_key(t, "Trip"),
        Dep::Adjacency(t) => keys::key(t, &[b"adj", b"out", &keys::u64be(1)]),
        Dep::Space(t, space) => keys::emb_vector_key(t, space, 1),
        Dep::Policy(t) => keys::security_policy_key(t),
        Dep::TenantDir => keys::tenant_by_ord_key(TenantId(9)),
        Dep::Tenant(t) => keys::key(t, &[b"no-such-family"]),
        Dep::Global => b"no-such-global".to_vec(),
    }
}

/// The rows a wide commit writes: `records` records of `tenant`, each with
/// an occurrence of every pid in `pids`, a label row, a label counter, an
/// index entry and a value-dictionary row per record.
fn wide_commit(tenant: TenantId, records: u64, pids: std::ops::Range<u64>) -> Vec<Vec<u8>> {
    let mut rows = Vec::new();
    for ord in 0..records {
        for pid in pids.clone() {
            rows.push(keys::occ_key(tenant, OwnerKind::Record, ord, pid, 0));
        }
        let label = format!("L{}", ord % 7);
        rows.push(keys::rederive_key(tenant, &label));
        rows.push(keys::label_count_key(tenant, &label));
        rows.push(keys::index_entry_key(
            tenant,
            u32::try_from(ord % 3).unwrap(),
            b"x",
            ord,
        ));
        rows.push(keys::value_dict_key(
            tenant,
            OwnerKind::Record,
            &label,
            ord % 11,
            b"v",
        ));
    }
    rows
}

#[test]
fn noting_a_commit_raises_every_area_it_writes_like_one_bump_per_row() {
    let store = store();
    let base = store.committed_version();
    // More distinct areas per record than the repeat filter holds; the
    // second commit overlaps the first and lands at a lower index (a replay
    // order), so only `fetch_max` may decide.
    let commits = [
        (base + 20, wide_commit(T1, 30, 0..1_500)),
        (base + 10, wide_commit(T1, 20, 1_000..2_600)),
        (base + 30, wide_commit(TenantId(2), 5, 0..50)),
    ];
    let reference = super::deps::VersionTable::new(TABLE_BITS);
    let mut written = std::collections::BTreeSet::new();
    for (ci, rows) in &commits {
        store
            .residents()
            .note_writes(*ci, rows.iter().map(Vec::as_slice));
        for row in rows {
            deps_of_key(row, |dep| {
                reference.bump(dep, *ci);
                written.insert(dep);
            });
        }
    }
    assert!(written.len() > 2 * NOTE_FILTER_SLOTS);
    for dep in written {
        assert_eq!(
            store.residents().version(&[dep]),
            reference.version(&[dep]),
            "{dep:?}"
        );
    }
}

#[test]
fn every_dep_kind_is_reached_by_its_sample_key() {
    let trip = Dep::LabelProperties(T1, label_hash("Trip"));
    for (dep, also) in [
        (Dep::Index(T1, 3), None),
        (Dep::Pid(T1, 7), None),
        (Dep::Label(T1, label_hash("Trip")), None),
        // A label's value dictionary is one of its label-scoped rows.
        (Dep::ValueDict(T1, 7), Some(trip)),
        (trip, Some(Dep::Pid(T1, 7))),
        (Dep::Definitions(T1), None),
        (Dep::Labels(T1), None),
        (Dep::Adjacency(T1), None),
        (Dep::Space(T1, 4), None),
        (Dep::Policy(T1), None),
        (Dep::TenantDir, None),
        (Dep::Tenant(T1), None),
        (Dep::Global, None),
    ] {
        let mut got = Vec::new();
        deps_of_key(&sample_key(dep), |d| got.push(d));
        let mut want: Vec<Dep> = std::iter::once(dep).chain(also).collect();
        got.sort();
        want.sort();
        assert_eq!(got, want);
    }
}

/// Every label-scoped property row raises its label's `LabelProperties`
/// and its property's own dep; a pid-wide row raises only the latter.
#[test]
fn label_scoped_property_rows_raise_their_label() {
    let trip = Dep::LabelProperties(T1, label_hash("Trip"));
    let rec = OwnerKind::Record;
    for (name, key, want) in [
        (
            "presence",
            keys::presence_key(T1, rec, "Trip", 7, 0, 9),
            vec![Dep::Pid(T1, 7), trip],
        ),
        (
            "value dictionary",
            keys::value_dict_key(T1, rec, "Trip", 7, b"v"),
            vec![Dep::ValueDict(T1, 7), trip],
        ),
        (
            "array marker",
            keys::array_label_marker_key(T1, rec, 7, "Trip"),
            vec![Dep::Pid(T1, 7), trip],
        ),
        (
            "occurrence",
            keys::occ_key(T1, rec, 9, 7, 0),
            vec![Dep::Pid(T1, 7)],
        ),
    ] {
        let mut got = Vec::new();
        deps_of_key(&key, |d| got.push(d));
        let mut want = want;
        got.sort();
        want.sort();
        assert_eq!(got, want, "{name}");
    }
    // The label-scoped counters, from the counter keys a commit writes.
    let mut counters = 0;
    for (name, key) in family_samples() {
        let mut got = Vec::new();
        deps_of_key(&key, |d| got.push(d));
        let scoped = matches!(
            name,
            "C_SUPPORT_BY_LABEL"
                | "C_DISTINCT"
                | "C_LONG_VALUES"
                | "C_LONG_VALUE"
                | "C_LONG_DISTINCT"
        );
        if scoped {
            counters += 1;
            assert!(
                got.iter()
                    .any(|dep| matches!(dep, Dep::LabelProperties(..))),
                "{name}: {got:?}"
            );
        }
    }
    assert_eq!(counters, 5);
}

#[test]
fn an_entry_offered_at_a_snapshot_older_than_its_version_is_not_kept() {
    let store = store();
    let key = probe(T1, 1040);
    let old = store.read_view().unwrap();
    write(&store, &[(key.row.clone(), b"new")]);
    store
        .residents()
        .layer::<Probe>()
        .offer(key.clone(), &old, Arc::new(None));
    let now = store.read_view().unwrap();
    assert!(get(&store, &key, &now).is_none());
    assert_eq!(snapshot(&store).entries, 0);
}

#[test]
fn hashed_slot_collisions_only_cause_misses() {
    // Four slots for many deps: almost every dep shares a slot.
    let table = super::deps::VersionTable::new(2);
    let mut seed = 0x9E37_79B9u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    // Truth: the last write per dep; entries: (deps, version recorded).
    let mut truth: HashMap<Dep, u64> = HashMap::new();
    let mut entries: Vec<(Vec<Dep>, u64, u64)> = Vec::new();
    for ci in 1..5_000u64 {
        let dep = Dep::Pid(TenantId(u32::try_from(next() % 3).unwrap()), next() % 40);
        table.bump(dep, ci);
        truth.insert(dep, ci);
        if next() % 3 == 0 {
            let deps: Vec<Dep> = (0..=next() % 3)
                .map(|_| Dep::Pid(TenantId(u32::try_from(next() % 3).unwrap()), next() % 40))
                .collect();
            let real = deps
                .iter()
                .filter_map(|d| truth.get(d))
                .max()
                .copied()
                .unwrap_or(0);
            entries.push((deps.clone(), table.version(&deps), real));
        }
        for (deps, version, real_then) in &entries {
            let real_now = deps
                .iter()
                .filter_map(|d| truth.get(d))
                .max()
                .copied()
                .unwrap_or(0);
            if table.version(deps) == *version {
                assert_eq!(real_now, *real_then, "a hit on a changed dep");
            }
        }
    }
}

/// One written key of every family, named by its constant.
fn family_samples() -> Vec<(&'static str, Vec<u8>)> {
    let t = T1;
    let rec = OwnerKind::Record;
    vec![
        ("F_RECORD_ID", keys::record_id_key(t, &[1; 16])),
        ("F_RECORD_HDR", keys::record_hdr_key(t, 5)),
        ("F_LABEL", keys::label_member_key(t, "Trip", 5)),
        ("F_COUNTER", keys::label_count_key(t, "Trip")),
        ("C_LABEL", keys::label_count_key(t, "Trip")),
        ("C_SUPPORT_TOTAL", keys::support_total_key(t, rec, 3)),
        (
            "C_SUPPORT_BY_PID",
            keys::support_by_pid_key(t, rec, 3, "Trip"),
        ),
        (
            "C_SUPPORT_BY_LABEL",
            keys::support_by_label_key(t, rec, "Trip", 3),
        ),
        ("C_NULL", keys::null_count_key(t, rec, 3)),
        ("C_DEGREE", keys::degree_key(t, 5, true)),
        ("C_DISTINCT", keys::distinct_key(t, rec, "Trip", 3)),
        ("C_REL_CATALOG", keys::rel_catalog_key(t, "R", "A", "B")),
        ("C_LONG_VALUES", keys::long_values_key(t, rec, "Trip", 3)),
        (
            "C_REL_SHAPE_PROP",
            keys::rel_shape_prop_key(t, ("R", "A", "B"), 3),
        ),
        ("C_USAGE", keys::usage_count_key(t, keys::U_RECORDS)),
        (
            "C_LONG_VALUE",
            keys::long_value_key(t, rec, "Trip", 3, &[0; 16]),
        ),
        (
            "C_LONG_DISTINCT",
            keys::long_distinct_key(t, rec, "Trip", 3),
        ),
        (
            "F_VALUE_DICT",
            keys::value_dict_key(t, rec, "Trip", 3, b"\x01v"),
        ),
        ("F_MARKER", keys::array_label_marker_key(t, rec, 3, "Trip")),
        ("F_OCC", keys::occ_key(t, rec, 5, 3, 0)),
        ("F_PRESENCE", keys::presence_key(t, rec, "Trip", 3, 0, 5)),
        ("F_INDEX_DEF", keys::index_def_key(t, 2)),
        ("F_INDEX_NAME", keys::index_name_key(t, "by_fare")),
        ("F_INDEX_ENTRY", keys::index_entry_key(t, 2, b"e", 5)),
        ("F_INDEX_PENDING", keys::index_pending_key(t, 2, 5)),
        ("F_UNIQUE_CLAIM", keys::unique_claim_key(t, 2, b"\x05value")),
        ("F_USAGE_BYTES", keys::usage_bytes_key(t)),
        ("F_REDERIVE", keys::rederive_key(t, "Trip")),
        ("P_META", keys::key(t, &[keys::P_META, b"alloc", b"x"])),
        ("P_REL", keys::key(t, &[keys::P_REL, b"id"])),
        (
            "P_REL_BY_ORD",
            keys::key(t, &[keys::P_REL_BY_ORD, &keys::u64be(1)]),
        ),
        (
            "P_REL_GROUP",
            keys::key(t, &[keys::P_REL_GROUP, &keys::u64be(1)]),
        ),
        ("P_FAMILY_SIG", keys::key(t, &[keys::P_FAMILY_SIG, b"s"])),
        ("P_FAMILY", keys::key(t, &[keys::P_FAMILY, b"f"])),
        ("P_DEFINITION", keys::key(t, &[keys::P_DEFINITION, b"d"])),
        (
            "property-row",
            keys::key(t, &[b"property-row", &keys::u64be(3)]),
        ),
        ("adj", keys::key(t, &[b"adj", b"out", &keys::u64be(1)])),
        ("P_EMB_SPACE", keys::emb_space_key(t, 4)),
        ("P_EMB_ALIAS", keys::emb_alias_key(t, "docs")),
        ("P_EMB_BINDING", keys::emb_binding_key(t, 4, rec, 5, 3, 0)),
        ("P_EMB_VECTOR", keys::emb_vector_key(t, 4, 1)),
        ("P_EMB_VERSION", keys::emb_space_version_key(t, 4)),
        ("P_EMB_READY", keys::emb_ready_count_key(t, 4)),
        ("P_EMB_RECLAIM", keys::emb_reclaim_key(t, 9, 4, 1)),
        ("security", keys::security_policy_key(t)),
        ("migration", keys::key(t, &[b"migration", b"0001"])),
        (
            "SYS_TENANT_BY_SCOPE",
            keys::tenant_by_scope_key(&rushdb_types::ids::TenantScope([7; 32])),
        ),
        ("SYS_TENANT_BY_ORD", keys::tenant_by_ord_key(t)),
        ("SYS_TENANT_SEQ", keys::tenant_seq_key().to_vec()),
        ("receipt", b"receipt/abc".to_vec()),
        ("log", b"__log__/1".to_vec()),
        ("meta", b"meta/last-applied".to_vec()),
    ]
}

#[test]
fn every_key_family_maps_to_deps_or_is_listed_as_unread() {
    let samples = family_samples();
    for (name, key) in &samples {
        assert!(
            !matches!(
                family(key),
                Family::UnknownTenant(_) | Family::UnknownGlobal | Family::UnknownCounter
            ),
            "{name} is not classified"
        );
    }
    // Every family and counter constant of the key module has a sample, so
    // a new one cannot be added without deciding what it changes.
    let source = include_str!("../../../rushdb-types/src/keys.rs");
    for line in source.lines() {
        let Some(rest) = line.trim().strip_prefix("pub const ") else {
            continue;
        };
        let name = rest.split(':').next().unwrap_or_default();
        let is_family = ["F_", "C_", "P_", "SYS_TENANT_"]
            .iter()
            .any(|prefix| name.starts_with(prefix));
        if is_family {
            assert!(
                samples.iter().any(|(sample, _)| *sample == name),
                "key family constant {name} has no sample in this test"
            );
        }
    }
}

#[test]
fn a_follower_apply_raises_versions_like_a_local_publish() {
    let store = store();
    let key = probe(T1, 1070);
    let before = offer(&store, &key);
    let row = key.row.clone();
    store.set_expander(Arc::new(Box::new(move |_, _, txn| {
        txn.put(row.clone(), b"replicated".to_vec());
        Ok(())
    })));
    let ci = store.apply_replicated_batch(&batch(T1)).unwrap();
    assert_eq!(store.residents().version(&[Dep::Pid(T1, 1070)]), ci);
    assert!(get(&store, &key, &before).is_none());
    let now = store.read_view().unwrap();
    assert!(get(&store, &key, &now).is_none());
    offer(&store, &key);
    assert_eq!(get(&store, &key, &now), Some(Some(b"replicated".to_vec())));
}

/// A restore replays WAL records into a running store; an entry made before
/// the restore must not survive it. (Before R0 the replay path raised no
/// version, so the owner-set cache kept serving its pre-restore set.)
#[test]
fn a_wal_replay_into_a_running_store_invalidates_entries() {
    let dir = std::env::temp_dir().join(format!("rushdb-resident-replay-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = Store::open(&dir.join("source.redb")).unwrap();
    install_writer(&source);
    let key = probe(T1, 1080);
    write(&source, &[(key.row.clone(), b"restored")]);
    let mut stream = Vec::new();
    source.export_wal_segments(&mut stream, 1, 1).unwrap();

    let target = store();
    let before = offer(&target, &key);
    assert_eq!(get(&target, &key, &before), Some(None));
    target.replay_wal_segments(&mut &stream[..]).unwrap();
    let after = target.read_view().unwrap();
    assert_eq!(after.get(&key.row).unwrap(), Some(b"restored".to_vec()));
    assert!(
        get(&target, &key, &after).is_none(),
        "a pre-restore entry served"
    );
    drop(source);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn get_results_are_shared_and_immutable() {
    let store = store();
    let key = probe(T1, 1090);
    let view = offer(&store, &key);
    let layer = store.residents().layer::<Probe>();
    let a = layer.get(&key, &view).unwrap();
    let b = layer.get(&key, &view).unwrap();
    assert!(Arc::ptr_eq(&a, &b));
}

#[test]
fn a_value_over_the_budget_is_never_kept_and_bytes_are_released() {
    let store = store_with(StorageResources {
        column_cache_bytes: 512,
        ..StorageResources::default()
    });
    let key = probe(T1, 1100);
    write(&store, &[(key.row.clone(), &[0u8; 4096])]);
    let view = offer(&store, &key);
    assert!(get(&store, &key, &view).is_none());
    let stats = snapshot(&store);
    assert_eq!((stats.entries, stats.bytes, stats.build_refused), (0, 0, 1));

    // Small entries fit; LRU makes room; invalidation releases bytes.
    for pid in 0..64 {
        let small = probe(T1, 100 + pid);
        offer(&store, &small);
    }
    let stats = snapshot(&store);
    assert!(stats.bytes <= 512 && stats.evictions > 0, "{stats:?}");
    let resident = stats.entries;
    let last = probe(T1, 163);
    let ci = write(&store, &[(last.row.clone(), b"x")]);
    assert!(ci > 0);
    let now = store.read_view().unwrap();
    assert!(get(&store, &last, &now).is_none());
    let after = snapshot(&store);
    assert_eq!(after.entries, resident - 1);
    assert!(after.bytes < stats.bytes);
}

#[test]
fn budget_zero_and_off_leave_no_entry() {
    let zero = store_with(StorageResources {
        column_cache_bytes: 0,
        ..StorageResources::default()
    });
    let key = probe(T1, 1110);
    let view = offer(&zero, &key);
    assert!(get(&zero, &key, &view).is_none());
    assert_eq!(snapshot(&zero).entries, 0);

    let off = store();
    off.residents().set_enabled(false);
    let view = offer(&off, &key);
    assert!(get(&off, &key, &view).is_none());
    assert_eq!(snapshot(&off).entries, 0);
    // Rent never builds while off.
    off.residents().layer::<Probe>().rent(&key, &view, 10, 1);
    off.residents().wait_idle();
    assert_eq!(snapshot(&off).builds, 0);
}

#[test]
fn rent_builds_in_the_background_once_paid() {
    let store = store();
    let key = probe(T1, 1120);
    let builds = hook(&key.row, |_| {});
    write(&store, &[(key.row.clone(), b"v")]);
    let view = store.read_view().unwrap();
    let layer = store.residents().layer::<Probe>();
    layer.rent(&key, &view, 40, 100);
    store.residents().wait_idle();
    assert!(layer.get(&key, &view).is_none(), "not paid yet");
    layer.rent(&key, &view, 60, 100);
    store.residents().wait_idle();
    assert_eq!(builds.load(Ordering::SeqCst), 1);
    assert_eq!(*layer.get(&key, &view).unwrap(), Some(b"v".to_vec()));
    // Rent at another version starts over.
    write(&store, &[(key.row.clone(), b"w")]);
    let now = store.read_view().unwrap();
    layer.rent(&key, &now, 60, 100);
    store.residents().wait_idle();
    assert_eq!(builds.load(Ordering::SeqCst), 1);
}

#[test]
fn rent_of_a_resident_key_builds_nothing() {
    let store = store();
    let key = probe(T1, 1125);
    let builds = hook(&key.row, |_| {});
    write(&store, &[(key.row.clone(), b"v")]);
    let view = store.read_view().unwrap();
    let layer = store.residents().layer::<Probe>();
    // Each reader pays a whole build, as a search that reads every row does.
    for _ in 0..20 {
        layer.rent(&key, &view, 100, 100);
        store.residents().wait_idle();
    }
    assert_eq!(builds.load(Ordering::SeqCst), 1, "built once, then hit");
    assert_eq!(*layer.get(&key, &view).unwrap(), Some(b"v".to_vec()));
    // A write to a dep: the next paid rent builds again, once.
    write(&store, &[(key.row.clone(), b"w")]);
    let now = store.read_view().unwrap();
    for _ in 0..20 {
        layer.rent(&key, &now, 100, 100);
        store.residents().wait_idle();
    }
    assert_eq!(builds.load(Ordering::SeqCst), 2);
}

#[test]
fn a_build_refused_part_way_leaves_no_entry_until_a_dep_changes() {
    let store = store();
    let key = probe(T1, 1130);
    let builds = hook(&key.row, |hook| hook.refuse = true);
    let view = store.read_view().unwrap();
    let layer = store.residents().layer::<Probe>();
    for _ in 0..3 {
        layer.rent(&key, &view, 100, 100);
        store.residents().wait_idle();
    }
    assert_eq!(builds.load(Ordering::SeqCst), 1, "declined at this version");
    assert_eq!(snapshot(&store).build_refused, 1);
    assert!(layer.get(&key, &view).is_none());
    write(&store, &[(key.row.clone(), b"v")]);
    let now = store.read_view().unwrap();
    layer.rent(&key, &now, 100, 100);
    store.residents().wait_idle();
    assert_eq!(builds.load(Ordering::SeqCst), 2, "a dep change retries");
}

#[test]
fn concurrent_rent_of_one_key_runs_one_build() {
    let store = store();
    let key = probe(T1, 1140);
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let builds = hook(&key.row, |hook| hook.gate = Some(Arc::clone(&gate)));
    let layer = store.residents().layer::<Probe>();
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                let view = store.read_view().unwrap();
                layer.rent(&key, &view, 100, 100);
            });
        }
    });
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    store.residents().wait_idle();
    assert_eq!(builds.load(Ordering::SeqCst), 1);
}

#[test]
fn a_build_racing_a_write_to_its_dep_is_never_served() {
    let store = store();
    let key = probe(T1, 1150);
    write(&store, &[(key.row.clone(), b"old")]);
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let builds = hook(&key.row, |hook| hook.gate = Some(Arc::clone(&gate)));
    let layer = store.residents().layer::<Probe>();
    let view = store.read_view().unwrap();
    layer.rent(&key, &view, 100, 100);
    while builds.load(Ordering::SeqCst) == 0 {
        std::thread::yield_now();
    }
    // The build has pinned its view; a write to its dep publishes now.
    write(&store, &[(key.row.clone(), b"new")]);
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    store.residents().wait_idle();
    assert_eq!(snapshot(&store).entries, 0, "a stale build was inserted");
    let now = store.read_view().unwrap();
    assert!(layer.get(&key, &now).is_none());
}

#[test]
fn verification_catches_a_corrupted_entry() {
    let store = store_with(StorageResources {
        resident_verify_rate: 1.0,
        ..StorageResources::default()
    });
    let key = probe(T1, 1160);
    write(&store, &[(key.row.clone(), b"true")]);
    let view = offer(&store, &key);
    let layer = store.residents().layer::<Probe>();
    layer.corrupt_for_test(&key, Some(b"false".to_vec()));
    // The hit already returned keeps its answer; verification runs after.
    assert_eq!(*layer.get(&key, &view).unwrap(), Some(b"false".to_vec()));
    store.residents().wait_idle();
    let stats = snapshot(&store);
    assert_eq!((stats.verify, stats.verify_mismatch), (1, 1));
    assert!(layer.get(&key, &view).is_none(), "the entry was dropped");
}

#[test]
fn counters_move_for_every_event() {
    let store = store_with(StorageResources {
        resident_verify_rate: 1.0,
        ..StorageResources::default()
    });
    let key = probe(T1, 1170);
    hook(&key.row, |_| {});
    let layer = store.residents().layer::<Probe>();
    let view = store.read_view().unwrap();
    assert!(layer.get(&key, &view).is_none());
    layer.rent(&key, &view, 1, 1);
    store.residents().wait_idle();
    assert!(layer.get(&key, &view).is_some());
    store.residents().wait_idle();
    write(&store, &[(key.row.clone(), b"x")]);
    let now = store.read_view().unwrap();
    assert!(layer.get(&key, &now).is_none());
    let stats = snapshot(&store);
    assert_eq!(stats.misses, 2);
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.builds, 1);
    assert_eq!(stats.invalidations, 1);
    assert_eq!((stats.verify, stats.verify_mismatch), (1, 0));
    assert!(stats.build_seconds >= 0.0);
}
