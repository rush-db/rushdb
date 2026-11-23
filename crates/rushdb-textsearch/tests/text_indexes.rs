//! Text indexes: declared on the index catalog, maintained by the
//! applier from pending rows, `eventual` (reports lag) or `immediate`
//! (read-your-writes), never pinning the WAL.

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
    candidates: Option<Arc<roaring::RoaringTreemap>>,
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
                count: false,
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
                count: false,
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
        Some(Arc::new(roaring::RoaringTreemap::from([engine.ordinal]))),
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
                top_k: 10,
                count: false,
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
                count: false,
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
fn an_applier_holding_the_definition_from_before_the_build_indexes_its_records() {
    // The applier reads the definitions, then a snapshot. A build step
    // committing in between marks its records pending and widens the
    // covered range in one commit: judged by the older (building) range,
    // those records were skipped, then cleared as applied, and the index
    // never held them. Writes waking the applier made this the usual
    // interleaving after "ingest, then create the index". The applier now
    // takes the index id and reads the definition at its own snapshot, so
    // it cannot be handed the older one; the sequence stays as a test.
    for property in ["body", "*"] {
        let (lmpg, text) = setup();
        article(&lmpg, "quantum world", "machines learn quantum mechanics");
        article(&lmpg, "dragons", "magic dragon call");
        let building = lmpg
            .define_index(
                "articles",
                "Article",
                property,
                IndexKind::Text {
                    consistency: TextConsistency::Eventual,
                },
            )
            .unwrap();
        while lmpg.index_step(building.id, 1_000).unwrap() != IndexStep::Finished {}
        while text.apply(&lmpg, building.id, 100).unwrap() > 0 {}
        let (hits, pending) = search(&lmpg, &text, "articles", "quantum");
        assert_eq!(
            (hits.len(), pending),
            (1, 0),
            "{property}: the record is indexed, not cleared as applied"
        );
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one seeded interleaving of every kind of step, read as a whole"
)]
fn any_interleaving_of_writes_builds_applies_and_redefinitions_settles_to_the_store() {
    // Background index work in every order the workers can take: writes,
    // build steps, applier batches, and the index dropped and declared
    // again. Whenever the index is ready with nothing pending, a search
    // returns exactly the records that hold the term.
    struct Rng(u64);
    impl Rng {
        fn below(&mut self, n: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % n
        }
    }
    let define = |lmpg: &Lmpg| {
        lmpg.define_index(
            "articles",
            "Article",
            "body",
            IndexKind::Text {
                consistency: TextConsistency::Eventual,
            },
        )
        .unwrap()
        .id
    };
    // Until the build or drop is done (`Idle`: nothing left, or ready).
    let settle = |lmpg: &Lmpg, id: u32| {
        while matches!(lmpg.index_step(id, 7).unwrap(), IndexStep::Advanced(_)) {}
    };
    // Every hit, not the first ten.
    let all_hits = |lmpg: &Lmpg, text: &TextIndexes| {
        let view = lmpg.store.read_view().unwrap();
        let hits = text
            .search(
                lmpg.tenant,
                &view,
                rushdb_textsearch::TextQuery {
                    index: "articles",
                    query: "quantum",
                    opts: LexicalOptions::default(),
                    top_k: 1_000,
                    count: false,
                },
                None,
            )
            .unwrap();
        let mut owners: Vec<u64> = hits.hits.iter().map(|h| h.owner_ord).collect();
        owners.sort_unstable();
        (owners, hits.pending)
    };
    for seed in 1..=12u64 {
        let (lmpg, text) = setup();
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut holders: std::collections::BTreeMap<u64, rushdb_lmpg::RecordRef> =
            std::collections::BTreeMap::new();
        let mut id = None;
        for step in 0..120 {
            match rng.below(8) {
                0..=2 => {
                    let quantum = rng.below(2) == 0;
                    let record = article(
                        &lmpg,
                        &format!("doc {step}"),
                        if quantum {
                            "quantum leap"
                        } else {
                            "plain words"
                        },
                    );
                    if quantum {
                        holders.insert(record.ordinal, record);
                    }
                }
                3 => {
                    if let Some((&ordinal, _)) = holders.iter().next() {
                        let record = holders.remove(&ordinal).unwrap();
                        lmpg.delete_record(record).unwrap();
                    }
                }
                4 => {
                    if let Some(current) = id {
                        let batch = usize::try_from(1 + rng.below(4)).unwrap();
                        let _ = lmpg.index_step(current, batch).unwrap();
                    }
                }
                5 | 6 => {
                    if let Some(current) = id {
                        let batch = usize::try_from(1 + rng.below(3)).unwrap();
                        text.apply(&lmpg, current, batch).unwrap();
                    }
                }
                _ => match id {
                    None => id = Some(define(&lmpg)),
                    Some(current) if rng.below(3) == 0 => {
                        lmpg.drop_index("articles").unwrap();
                        settle(&lmpg, current);
                        text.remove(TENANT, current).unwrap();
                        id = Some(define(&lmpg));
                    }
                    Some(_) => {}
                },
            }
            let Some(current) = id else { continue };
            let view = lmpg.store.read_view().unwrap();
            let ready = rushdb_lmpg::index::by_id(&view, TENANT, current)
                .unwrap()
                .is_some_and(|def| def.state == rushdb_types::index::IndexState::Ready);
            drop(view);
            if !ready {
                continue;
            }
            let (hits, pending) = all_hits(&lmpg, &text);
            if pending == 0 {
                let want: Vec<u64> = holders.keys().copied().collect();
                assert_eq!(hits, want, "seed {seed}, step {step}");
            }
        }
        // Settled: build to the end, apply everything.
        let current = id.unwrap_or_else(|| define(&lmpg));
        settle(&lmpg, current);
        while text.apply(&lmpg, current, 100).unwrap() > 0 {}
        let (hits, pending) = all_hits(&lmpg, &text);
        assert_eq!(pending, 0, "seed {seed}");
        assert_eq!(
            hits,
            holders.keys().copied().collect::<Vec<_>>(),
            "seed {seed}"
        );
    }
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
    let run = |top_k, candidates, count| {
        text.search(
            lmpg.tenant,
            &view,
            rushdb_textsearch::TextQuery {
                index: "articles",
                query: "database",
                opts: LexicalOptions::default(),
                top_k,
                count,
            },
            candidates,
        )
        .unwrap()
    };
    let top = run(5, None, true);
    assert_eq!((top.hits.len(), top.matches), (5, Some(31)));
    // Uncounted, the same hits (and the same scores) come back.
    let uncounted = run(5, None, false);
    assert_eq!(uncounted.matches, None);
    let owners = |hits: &rushdb_textsearch::TextHits| -> Vec<(u64, u64)> {
        hits.hits
            .iter()
            .map(|hit| (hit.owner_ord, hit.score.to_bits()))
            .collect()
    };
    assert_eq!(owners(&uncounted), owners(&top));
    let all = run(rushdb_textsearch::ALL_HITS, None, false);
    assert_eq!((all.hits.len(), all.matches), (31, Some(31)));
    // A large finite `top_k` is sized by the matches too.
    let large = run(1_000_000, None, false);
    assert_eq!((large.hits.len(), large.matches), (31, Some(31)));
    assert_eq!(owners(&large), owners(&all));
    // Candidates bound the count too.
    let scoped = run(
        5,
        Some(Arc::new(roaring::RoaringTreemap::from([other.ordinal]))),
        true,
    );
    assert_eq!((scoped.hits.len(), scoped.matches), (1, Some(1)));
}

#[test]
fn ties_at_the_kth_score_go_to_the_lowest_owners() {
    let (lmpg, text) = setup();
    define(
        &lmpg,
        "articles",
        "Article",
        "*",
        TextConsistency::Immediate,
    );
    // Twelve records with the same text score the same.
    let tied: Vec<_> = (0..12).map(|_| article(&lmpg, "tie", "same")).collect();
    // Rewriting the lowest owners re-adds their documents at the end of the
    // index, so index order no longer follows owner order.
    for &record in &tied[..6] {
        for body in ["changed", "same"] {
            lmpg.update_fields(record, vec![("body".into(), FieldChange::Set(json!(body)))])
                .unwrap();
            // Applied one at a time, each change reaches the index.
            text.search(
                lmpg.tenant,
                &lmpg.store.read_view().unwrap(),
                rushdb_textsearch::TextQuery {
                    index: "articles",
                    query: "tie",
                    opts: LexicalOptions::default(),
                    top_k: 1,
                    count: false,
                },
                None,
            )
            .unwrap();
        }
    }
    let mut lowest: Vec<u64> = tied.iter().map(|record| record.ordinal).collect();
    lowest.sort_unstable();
    let view = lmpg.store.read_view().unwrap();
    for top_k in [1, 3, 5, 11] {
        for count in [false, true] {
            let hits = text
                .search(
                    lmpg.tenant,
                    &view,
                    rushdb_textsearch::TextQuery {
                        index: "articles",
                        query: "tie",
                        opts: LexicalOptions::default(),
                        top_k,
                        count,
                    },
                    None,
                )
                .unwrap();
            let owners: Vec<u64> = hits.hits.iter().map(|hit| hit.owner_ord).collect();
            assert_eq!(owners, lowest[..top_k], "top {top_k}, count {count}");
        }
    }
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

#[test]
fn plain_text_is_searched_as_words() {
    let (lmpg, text) = setup();
    let a = article(
        &lmpg,
        "Adhesion molecules and chemokines: relation to inflammation",
        "role of the virus' genome",
    );
    article(&lmpg, "Unrelated", "nothing here");
    define(
        &lmpg,
        "articles",
        "Article",
        "*",
        TextConsistency::Immediate,
    );
    for query in [
        "inflammatory chemokines: role",
        "virus' genome",
        "AND OR NOT chemokines",
    ] {
        assert_eq!(
            search(&lmpg, &text, "articles", query).0,
            vec![a.ordinal],
            "{query}"
        );
    }
    assert!(search(&lmpg, &text, "articles", "NOT").0.is_empty());
    let all = LexicalOptions {
        all_terms: true,
        ..LexicalOptions::default()
    };
    assert_eq!(
        search_opts(
            &lmpg,
            &text,
            "articles",
            "chemokines: inflammation",
            all,
            None
        )
        .0,
        vec![a.ordinal]
    );
    assert!(
        search_opts(&lmpg, &text, "articles", "chemokines absent", all, None)
            .0
            .is_empty()
    );
}

#[test]
fn the_query_syntax_is_opt_in_and_reports_what_does_not_parse() {
    let (lmpg, text) = setup();
    let a = article(&lmpg, "graph database", "storage");
    article(&lmpg, "relational database", "storage");
    define(
        &lmpg,
        "articles",
        "Article",
        "*",
        TextConsistency::Immediate,
    );
    let syntax = LexicalOptions {
        syntax: rushdb_textsearch::TextSyntax::Query,
        ..LexicalOptions::default()
    };
    assert_eq!(
        search_opts(
            &lmpg,
            &text,
            "articles",
            "database -relational",
            syntax,
            None
        )
        .0,
        vec![a.ordinal]
    );
    let view = lmpg.store.read_view().unwrap();
    let attempt = |index: &str, query: &str| {
        text.search(
            lmpg.tenant,
            &view,
            rushdb_textsearch::TextQuery {
                index,
                query,
                opts: syntax,
                top_k: 10,
                count: false,
            },
            None,
        )
        .unwrap_err()
    };
    assert!(matches!(
        attempt("articles", "title:("),
        rushdb_textsearch::TextSearchError::InvalidQuery(_)
    ));
    // The index's internal owner column is not searchable.
    assert!(matches!(
        attempt("articles", "owner:1"),
        rushdb_textsearch::TextSearchError::InvalidQuery(_)
    ));
    assert!(matches!(
        attempt("missing", "database"),
        rushdb_textsearch::TextSearchError::UnknownIndex(_)
    ));
    // As plain words the same string is two terms, neither indexed.
    assert!(search(&lmpg, &text, "articles", "title:(").0.is_empty());
}

#[test]
fn the_applier_merges_the_segments_its_batches_write() {
    let (lmpg, text) = setup();
    let id = define(&lmpg, "articles", "Article", "*", TextConsistency::Eventual);
    // Ten apply batches' worth of changes, each batch committing a segment.
    for i in 0..5_000 {
        article(&lmpg, &format!("note {i}"), "graph storage");
    }
    while text
        .apply_tenant(&lmpg, std::time::Duration::from_secs(60))
        .unwrap()
        > 0
    {}
    let segments = text.segments(lmpg.tenant, id).unwrap();
    assert!(segments <= 4, "{segments} segments after ten batches");
    assert_eq!(search(&lmpg, &text, "articles", "graph").1, 0);
}
