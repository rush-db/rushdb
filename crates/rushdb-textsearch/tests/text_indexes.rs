//! Text indexes: declared on the index catalog, maintained by the
//! applier from pending rows, `eventual` (reports lag) or `immediate`
//! (read-your-writes), never pinning the WAL.

use std::collections::HashSet;
use std::sync::Arc;

use rushdb_lmpg::index::{pending, IndexStep};
use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::Lmpg;
use rushdb_storage::Store;
use rushdb_textsearch::{LexicalOptions, TextIndexes};
use rushdb_types::ids::TenantId;
use rushdb_types::index::{IndexKind, TextConsistency};
use serde_json::json;

const TENANT: TenantId = TenantId::from_ordinal(21);

fn setup() -> (Lmpg, TextIndexes) {
    let store = Arc::new(Store::new_in_memory());
    (
        Lmpg::new(Arc::clone(&store), TENANT),
        TextIndexes::new(store),
    )
}

fn define(
    lmpg: &Lmpg,
    name: &str,
    label: &str,
    property: &str,
    consistency: TextConsistency,
) -> u32 {
    let def = lmpg
        .define_index(name, label, property, IndexKind::Text { consistency })
        .unwrap();
    while lmpg.index_step(def.id, 1_000).unwrap() != IndexStep::Finished {}
    def.id
}

fn search(lmpg: &Lmpg, text: &TextIndexes, name: &str, query: &str) -> (Vec<u64>, u64) {
    search_opts(lmpg, text, name, query, LexicalOptions::default(), None)
}

fn search_opts(
    lmpg: &Lmpg,
    text: &TextIndexes,
    name: &str,
    query: &str,
    opts: LexicalOptions,
    candidates: Option<Arc<HashSet<u64>>>,
) -> (Vec<u64>, u64) {
    let view = lmpg.store.read_view().unwrap();
    let hits = text
        .search(
            lmpg.tenant,
            &view,
            rushdb_textsearch::TextQuery {
                index: name,
                query,
                opts,
                top_k: 10,
            },
            candidates,
        )
        .unwrap();
    (
        hits.hits.iter().map(|h| h.owner_ord).collect(),
        hits.pending,
    )
}

fn article(lmpg: &Lmpg, title: &str, body: &str) -> rushdb_lmpg::RecordRef {
    lmpg.create_record(
        "Article",
        None,
        vec![
            ("title".into(), FieldChange::Set(json!(title))),
            ("body".into(), FieldChange::Set(json!(body))),
        ],
    )
    .unwrap()
}

#[test]
fn bm25_ranks_relevant_documents_across_all_string_properties() {
    let (lmpg, text) = setup();
    let graph = article(
        &lmpg,
        "Graph database fundamentals",
        "A property graph database stores nodes",
    );
    article(&lmpg, "Cooking pasta", "Boil water and add salt");
    let engine = article(
        &lmpg,
        "Database internals",
        "Storage engines in a database engine",
    );
    define(
        &lmpg,
        "articles",
        "Article",
        "*",
        TextConsistency::Immediate,
    );
    let (hits, pending) = search(&lmpg, &text, "articles", "database");
    assert_eq!(pending, 0);
    assert_eq!(hits.len(), 2);
    assert!(hits.contains(&graph.ordinal) && hits.contains(&engine.ordinal));
    let view = lmpg.store.read_view().unwrap();
    let all = text
        .search(
            lmpg.tenant,
            &view,
            rushdb_textsearch::TextQuery {
                index: "articles",
                query: "database engine",
                opts: LexicalOptions {
                    all_terms: true,
                    ..LexicalOptions::default()
                },
                top_k: 10,
            },
            None,
        )
        .unwrap();
    assert_eq!(
        all.hits.iter().map(|h| h.owner_ord).collect::<Vec<_>>(),
        vec![engine.ordinal]
    );
    // Candidates restrict ranking (hybrid search's filter-before-rank).
    let (scoped, _) = search_opts(
        &lmpg,
        &text,
        "articles",
        "database",
        LexicalOptions::default(),
        Some(Arc::new(HashSet::from([engine.ordinal]))),
    );
    assert_eq!(scoped, vec![engine.ordinal]);
}

#[test]
fn a_property_index_sees_only_that_property() {
    let (lmpg, text) = setup();
    let a = article(&lmpg, "graph", "pasta");
    article(&lmpg, "pasta", "graph");
    define(
        &lmpg,
        "titles",
        "Article",
        "title",
        TextConsistency::Immediate,
    );
    assert_eq!(search(&lmpg, &text, "titles", "graph").0, vec![a.ordinal]);
}

#[test]
fn incremental_index_tracks_insert_update_delete_and_retype() {
    let (lmpg, text) = setup();
    define(
        &lmpg,
        "articles",
        "Article",
        "*",
        TextConsistency::Immediate,
    );
    let a = article(&lmpg, "alpha", "one");
    let b = article(&lmpg, "beta", "two");
    assert_eq!(search(&lmpg, &text, "articles", "alpha").0, vec![a.ordinal]);
    lmpg.update_fields(
        b,
        vec![("title".into(), FieldChange::Set(json!("alpha again")))],
    )
    .unwrap();
    let (mut both, _) = search(&lmpg, &text, "articles", "alpha");
    both.sort_unstable();
    assert_eq!(both, vec![a.ordinal, b.ordinal]);
    lmpg.delete_record(a).unwrap();
    assert_eq!(search(&lmpg, &text, "articles", "alpha").0, vec![b.ordinal]);
    // Leaving the label leaves the index.
    lmpg.retype_record(b, "Article", "Draft").unwrap();
    assert!(search(&lmpg, &text, "articles", "alpha").0.is_empty());
}

#[test]
fn text_eventual_reports_lag_and_converges() {
    let (lmpg, text) = setup();
    define(&lmpg, "articles", "Article", "*", TextConsistency::Eventual);
    for i in 0..5 {
        article(&lmpg, &format!("story {i}"), "news");
    }
    let (hits, pending) = search(&lmpg, &text, "articles", "news");
    assert!(hits.is_empty(), "nothing applied yet");
    assert_eq!(pending, 5, "the backlog is reported");
    assert_eq!(
        text.apply_tenant(&lmpg, std::time::Duration::from_secs(5))
            .unwrap(),
        5
    );
    let (hits, pending) = search(&lmpg, &text, "articles", "news");
    assert_eq!((hits.len(), pending), (5, 0));
}

#[test]
fn text_immediate_is_read_your_writes() {
    let (lmpg, text) = setup();
    define(
        &lmpg,
        "articles",
        "Article",
        "*",
        TextConsistency::Immediate,
    );
    for i in 0..20 {
        let r = article(&lmpg, &format!("item {i}"), "fresh");
        let (hits, pending) = search(&lmpg, &text, "articles", &format!("\"item {i}\""));
        assert_eq!(pending, 0);
        assert!(
            hits.contains(&r.ordinal),
            "write {i} is visible to the next search"
        );
    }
}

#[test]
fn text_never_pins_wal_and_idle_tenant_never_commits() {
    let (lmpg, text) = setup();
    define(&lmpg, "articles", "Article", "*", TextConsistency::Eventual);
    article(&lmpg, "once", "only");
    text.apply_tenant(&lmpg, std::time::Duration::from_secs(5))
        .unwrap();
    let settled = lmpg.store.committed_version();
    for _ in 0..10 {
        assert_eq!(
            text.apply_tenant(&lmpg, std::time::Duration::from_secs(5))
                .unwrap(),
            0
        );
        search(&lmpg, &text, "articles", "once");
    }
    assert_eq!(
        lmpg.store.committed_version(),
        settled,
        "an idle text index never commits"
    );
}

#[test]
fn cross_tenant_commits_do_not_touch_other_tenants_text() {
    let store = Arc::new(Store::new_in_memory());
    let a = Lmpg::new(Arc::clone(&store), TENANT);
    let b = Lmpg::new(Arc::clone(&store), TenantId::from_ordinal(22));
    let text = TextIndexes::new(Arc::clone(&store));
    let id = define(&a, "articles", "Article", "*", TextConsistency::Eventual);
    article(&a, "mine", "a");
    text.apply_tenant(&a, std::time::Duration::from_secs(5))
        .unwrap();
    for i in 0..10 {
        b.create_record(
            "Article",
            None,
            vec![("title".into(), FieldChange::Set(json!(i)))],
        )
        .unwrap();
    }
    let view = store.read_view().unwrap();
    assert!(pending(&view, TENANT, id, 100).unwrap().is_empty());
    assert_eq!(
        text.apply_tenant(&a, std::time::Duration::from_secs(5))
            .unwrap(),
        0
    );
}

#[test]
fn text_index_restart_resumes_from_pending_rows() {
    let dir = rushdb_storage::TempPath::dir("text-restart");
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rushdb.redb");
    let (applied, pending) = {
        let store = Arc::new(rushdb_lmpg::open_store(&db).unwrap());
        let lmpg = Lmpg::new(Arc::clone(&store), TENANT);
        let text = TextIndexes::new(store);
        define(&lmpg, "articles", "Article", "*", TextConsistency::Eventual);
        let applied = article(&lmpg, "applied before restart", "x");
        text.apply_tenant(&lmpg, std::time::Duration::from_secs(5))
            .unwrap();
        let pending = article(&lmpg, "pending at restart", "y");
        (applied, pending)
    };
    let store = Arc::new(rushdb_lmpg::open_store(&db).unwrap());
    let lmpg = Lmpg::new(Arc::clone(&store), TENANT);
    let text = TextIndexes::new(store);
    // The tantivy directory persisted the applied document.
    assert_eq!(
        search(&lmpg, &text, "articles", "applied").0,
        vec![applied.ordinal]
    );
    assert_eq!(search(&lmpg, &text, "articles", "pending").1, 1);
    text.apply_tenant(&lmpg, std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(
        search(&lmpg, &text, "articles", "pending").0,
        vec![pending.ordinal]
    );
    assert!(
        dir.join("text").exists(),
        "text indexes live under the data directory"
    );
    drop(text);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn text_applier_crash_between_tantivy_commit_and_marker_is_idempotent() {
    let (lmpg, text) = setup();
    let id = define(&lmpg, "articles", "Article", "*", TextConsistency::Eventual);
    let r = article(&lmpg, "twice applied", "z");
    text.apply_tenant(&lmpg, std::time::Duration::from_secs(5))
        .unwrap();
    // Simulate the crash: the tantivy commit happened but the marker did
    // not, so the owner is pending again; applying it again must not
    // duplicate the document.
    lmpg.update_fields(r, vec![("body".into(), FieldChange::Set(json!("z")))])
        .unwrap();
    let view = lmpg.store.read_view().unwrap();
    assert_eq!(pending(&view, TENANT, id, 10).unwrap(), vec![r.ordinal]);
    text.apply_tenant(&lmpg, std::time::Duration::from_secs(5))
        .unwrap();
    text.apply_tenant(&lmpg, std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(search(&lmpg, &text, "articles", "twice").0, vec![r.ordinal]);
}

#[test]
fn a_change_racing_the_applier_stays_pending() {
    let (lmpg, text) = setup();
    let id = define(&lmpg, "articles", "Article", "*", TextConsistency::Eventual);
    let r = article(&lmpg, "first", "v");
    let upto = lmpg.store.committed_version();
    lmpg.update_fields(r, vec![("title".into(), FieldChange::Set(json!("second")))])
        .unwrap();
    // An applier that read the snapshot before the edit consumes nothing
    // newer than that snapshot.
    lmpg.index_applied(id, upto, vec![r.ordinal]).unwrap();
    let view = lmpg.store.read_view().unwrap();
    assert_eq!(pending(&view, TENANT, id, 10).unwrap(), vec![r.ordinal]);
    drop(view);
    text.apply_tenant(&lmpg, std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(
        search(&lmpg, &text, "articles", "second").0,
        vec![r.ordinal]
    );
}

#[test]
fn phrase_adjacency_verification() {
    let (lmpg, text) = setup();
    let a = article(&lmpg, "Storage engine internals", "database systems class");
    article(&lmpg, "x", "engine of the database stored");
    define(
        &lmpg,
        "articles",
        "Article",
        "*",
        TextConsistency::Immediate,
    );
    let phrase = LexicalOptions {
        phrase: true,
        ..LexicalOptions::default()
    };
    assert_eq!(
        search_opts(&lmpg, &text, "articles", "storage engine", phrase, None).0,
        vec![a.ordinal]
    );
    assert_eq!(
        search(&lmpg, &text, "articles", "storage engine").0.len(),
        2
    );
}

#[test]
fn building_and_dropping_text_indexes() {
    let (lmpg, text) = setup();
    for i in 0..30 {
        article(&lmpg, &format!("doc {i}"), "corpus");
    }
    let def = lmpg
        .define_index(
            "articles",
            "Article",
            "*",
            IndexKind::Text {
                consistency: TextConsistency::Eventual,
            },
        )
        .unwrap();
    let view = lmpg.store.read_view().unwrap();
    assert!(
        text.search(
            lmpg.tenant,
            &view,
            rushdb_textsearch::TextQuery {
                index: "articles",
                query: "corpus",
                opts: LexicalOptions::default(),
                top_k: 10
            },
            None
        )
        .is_err(),
        "a building index is not searchable"
    );
    drop(view);
    while lmpg.index_step(def.id, 7).unwrap() != IndexStep::Finished {}
    text.apply_tenant(&lmpg, std::time::Duration::from_secs(5))
        .unwrap();
    let view = lmpg.store.read_view().unwrap();
    let all = text
        .search(
            lmpg.tenant,
            &view,
            rushdb_textsearch::TextQuery {
                index: "articles",
                query: "corpus",
                opts: LexicalOptions::default(),
                top_k: 100,
            },
            None,
        )
        .unwrap();
    assert_eq!(
        all.hits.len(),
        30,
        "the backfill covered every existing record"
    );
    drop(view);
    article(&lmpg, "late", "corpus");
    lmpg.drop_index("articles").unwrap();
    while lmpg.index_step(def.id, 7).unwrap() != IndexStep::Finished {}
    let view = lmpg.store.read_view().unwrap();
    assert!(pending(&view, TENANT, def.id, 10).unwrap().is_empty());
    assert!(rushdb_lmpg::index::by_name(&view, TENANT, "articles")
        .unwrap()
        .is_none());
    text.remove(TENANT, def.id).unwrap();
}

#[test]
fn a_search_counts_every_match_and_can_return_them_all() {
    let (lmpg, text) = setup();
    for i in 0..30 {
        article(&lmpg, &format!("Database note {i}"), "graph storage");
    }
    let other = article(&lmpg, "Database outlier", "kept apart");
    article(&lmpg, "Cooking pasta", "Boil water");
    define(
        &lmpg,
        "articles",
        "Article",
        "*",
        TextConsistency::Immediate,
    );
    let view = lmpg.store.read_view().unwrap();
    let run = |top_k, candidates| {
        text.search(
            lmpg.tenant,
            &view,
            rushdb_textsearch::TextQuery {
                index: "articles",
                query: "database",
                opts: LexicalOptions::default(),
                top_k,
            },
            candidates,
        )
        .unwrap()
    };
    let top = run(5, None);
    assert_eq!((top.hits.len(), top.matches), (5, 31));
    let all = run(rushdb_textsearch::ALL_HITS, None);
    assert_eq!((all.hits.len(), all.matches), (31, 31));
    // Candidates bound the count too.
    let scoped = run(5, Some(Arc::new(HashSet::from([other.ordinal]))));
    assert_eq!((scoped.hits.len(), scoped.matches), (1, 1));
}

#[test]
fn index_for_names_the_ready_index_of_a_label_and_property() {
    let (lmpg, _text) = setup();
    article(&lmpg, "Graph", "body");
    define(
        &lmpg,
        "titles",
        "Article",
        "title",
        TextConsistency::Eventual,
    );
    define(
        &lmpg,
        "everything",
        "Article",
        "*",
        TextConsistency::Eventual,
    );
    let view = lmpg.store.read_view().unwrap();
    let find = |label, property| TextIndexes::index_for(&view, TENANT, label, property).unwrap();
    assert_eq!(find("Article", "title").as_deref(), Some("titles"));
    assert_eq!(find("Article", "*").as_deref(), Some("everything"));
    assert_eq!(find("Article", "body"), None);
    assert_eq!(find("Note", "title"), None);
}

/// More text indexes than the open-index cache holds: every one still
/// answers, in memory (which never evicts: the open index is the data) and
/// on disk (bounded; an evicted index reopens from its files).
#[test]
fn more_indexes_than_the_open_cache_holds_all_keep_answering() {
    let dir = rushdb_storage::TempPath::dir("text-open-cache");
    std::fs::create_dir_all(&dir).unwrap();
    let on_disk = Arc::new(rushdb_lmpg::open_store(&dir.join("rushdb.redb")).unwrap());
    for store in [Arc::new(Store::new_in_memory()), on_disk] {
        let store_is_persistent = store.data_dir().is_some();
        let lmpg = Lmpg::new(Arc::clone(&store), TENANT);
        let text = TextIndexes::new(store);
        let labels: Vec<String> = (0..300).map(|i| format!("L{i}")).collect();
        for label in &labels {
            lmpg.create_record(
                label,
                None,
                vec![("body".into(), FieldChange::Set(json!("needle")))],
            )
            .unwrap();
            define(&lmpg, label, label, "body", TextConsistency::Immediate);
        }
        for label in &labels {
            let (hits, _) = search(&lmpg, &text, label, "needle");
            assert_eq!(hits.len(), 1, "{label}");
            if store_is_persistent {
                assert!(text.open_indexes() <= 256, "open cache is bounded");
            }
        }
        // The first indexes were evicted along the way; they still answer.
        for label in &labels[..5] {
            let (hits, _) = search(&lmpg, &text, label, "needle");
            assert_eq!(hits.len(), 1, "{label} after eviction");
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}
