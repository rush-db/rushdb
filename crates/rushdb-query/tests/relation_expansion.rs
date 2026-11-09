//! `$relation` with `hops`: trail semantics, the two plans that answer it
//! (backward expansion from the targets, a forward check per candidate)
//! agreeing with a brute-force trail oracle over stored and resident
//! adjacency, a cost that follows the anchor's neighbourhood rather than
//! the label (BENCH-10), and the resident graph's rent-or-buy rule and
//! snapshot exactness.

use std::collections::BTreeSet;
use std::sync::Arc;

use rushdb_lmpg::index::IndexStep;
use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{Lmpg, RecordRef};
use rushdb_query::document::parse_document;
use rushdb_query::{explain_expansion, find_records, find_records_in_view, SearchRequest};
use rushdb_storage::{resources::StorageResources, Store};
use rushdb_types::ids::TenantId;
use rushdb_types::index::IndexKind;
use serde_json::{json, Value};

fn setup() -> Lmpg {
    Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(73))
}

/// A store whose traversals always read adjacency rows (no graph cache).
fn setup_without_graphs() -> Lmpg {
    let store = Store::new_with_resources(StorageResources {
        graph_cache_bytes: 0,
        ..StorageResources::default()
    })
    .unwrap();
    Lmpg::new(Arc::new(store), TenantId::from_ordinal(73))
}

/// Adjacency rows one search reads from storage.
fn adjacency_reads(lmpg: &Lmpg, req: &SearchRequest) -> (BTreeSet<u64>, u64) {
    rushdb_storage::read_accounting::enable(true);
    let _ = rushdb_storage::read_accounting::take_local();
    let found = node_ids(lmpg, req);
    let reads = rushdb_storage::read_accounting::take_local()
        .into_iter()
        .filter(|((_, family), _)| family == "legacy/adj")
        .map(|(_, count)| count)
        .sum();
    (found, reads)
}

fn person(lmpg: &Lmpg, label: &str, id: u64, city: &str) -> RecordRef {
    lmpg.create_record(
        label,
        None,
        vec![
            ("node_id".into(), FieldChange::Set(json!(id))),
            ("city".into(), FieldChange::Set(json!(city))),
        ],
    )
    .unwrap()
}

fn request(document: &Value) -> SearchRequest {
    parse_document(document).expect("valid query").search
}

fn node_ids(lmpg: &Lmpg, req: &SearchRequest) -> BTreeSet<u64> {
    find_records(lmpg, req)
        .expect("search")
        .records
        .iter()
        .map(|record| record["node_id"].as_u64().unwrap())
        .collect()
}

/// Both plans: the planner's (backward expansion when it chooses it) and the
/// forced per-candidate check.
fn both_plans(lmpg: &Lmpg, document: &Value) -> (BTreeSet<u64>, BTreeSet<u64>) {
    let planned = request(document);
    let mut checked = planned.clone();
    checked.force_scan = true;
    (node_ids(lmpg, &planned), node_ids(lmpg, &checked))
}

fn neighbourhood_query(anchor: u64, direction: &str, min: u64, max: u64) -> Value {
    json!({"labels": ["P"], "where": {"P": {
        "$relation": {"type": "K", "direction": direction, "hops": {"min": min, "max": max}},
        "node_id": anchor
    }}})
}

#[test]
fn the_anchor_matches_itself_only_over_a_closed_trail() {
    let lmpg = setup();
    let a = person(&lmpg, "P", 1, "x");
    let b = person(&lmpg, "P", 2, "x");
    let c = person(&lmpg, "P", 3, "x");
    lmpg.create_relationship("K", a, b, vec![]).unwrap();
    // a - b and b - c: going a -> b -> a would reuse the one relationship.
    lmpg.create_relationship("K", b, c, vec![]).unwrap();
    for plan in [false, true] {
        let mut req = request(&neighbourhood_query(1, "both", 1, 2));
        req.force_scan = plan;
        assert_eq!(node_ids(&lmpg, &req), BTreeSet::from([2, 3]), "scan={plan}");
    }
    // A second relationship a - b closes a trail of two hops through a.
    lmpg.create_relationship("K", b, a, vec![]).unwrap();
    let (planned, checked) = both_plans(&lmpg, &neighbourhood_query(1, "both", 1, 2));
    assert_eq!(planned, BTreeSet::from([1, 2, 3]));
    assert_eq!(checked, planned);
    // Exactly two hops: b is one hop away, but a - b - a over the two
    // relationships is a trail of two, and c - b - a is two.
    let (planned, checked) = both_plans(&lmpg, &neighbourhood_query(1, "both", 2, 2));
    assert_eq!(planned, BTreeSet::from([1, 3]));
    assert_eq!(checked, planned);
}

#[test]
fn a_triangle_closes_within_three_hops() {
    let lmpg = setup();
    let a = person(&lmpg, "P", 1, "x");
    let b = person(&lmpg, "P", 2, "x");
    let c = person(&lmpg, "P", 3, "x");
    lmpg.create_relationship("K", a, b, vec![]).unwrap();
    lmpg.create_relationship("K", b, c, vec![]).unwrap();
    lmpg.create_relationship("K", c, a, vec![]).unwrap();
    for (direction, max, expected) in [
        ("out", 2, vec![2, 3]),
        ("out", 3, vec![1, 2, 3]),
        ("in", 2, vec![2, 3]),
        ("both", 2, vec![2, 3]),
        ("both", 3, vec![1, 2, 3]),
    ] {
        let (planned, checked) = both_plans(&lmpg, &neighbourhood_query(1, direction, 1, max));
        let expected: BTreeSet<u64> = expected.into_iter().collect();
        assert_eq!(planned, expected, "{direction} max {max}");
        assert_eq!(checked, expected, "{direction} max {max} (per candidate)");
    }
}

#[test]
fn a_narrow_target_is_expanded_from_and_a_broad_one_is_checked_per_candidate() {
    let lmpg = setup();
    for id in 0..40 {
        person(&lmpg, "P", id, "x");
    }
    let view = lmpg.store.read_view().unwrap();
    let narrow = request(&neighbourhood_query(7, "both", 1, 2));
    let broad = request(&json!({"labels": ["P"], "where": {"P": {
        "$relation": {"type": "K", "direction": "both"}, "city": "x"}}}));
    let tenant = lmpg.tenant;
    let planned = |req: &SearchRequest| {
        explain_expansion(&view, tenant, &req.labels, req.where_.as_ref().unwrap()).unwrap()
    };
    assert_eq!(planned(&narrow), Some(1));
    assert_eq!(planned(&broad), None);
}

/// Every record of label `P` from which a trail of `min..=max` hops of type
/// `K` leads to a target, by enumerating trails.
struct Graph {
    labels: Vec<&'static str>,
    cities: Vec<&'static str>,
    edges: Vec<(usize, usize, &'static str)>,
}

impl Graph {
    fn steps(&self, node: usize, direction: &str) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for (rel, &(source, target, ty)) in self.edges.iter().enumerate() {
            if ty != "K" {
                continue;
            }
            if matches!(direction, "out" | "both") && source == node {
                out.push((target, rel));
            }
            if matches!(direction, "in" | "both")
                && target == node
                && !(direction == "both" && source == node)
            {
                out.push((source, rel));
            }
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn reaches(
        &self,
        node: usize,
        depth: u64,
        used: &mut Vec<usize>,
        direction: &str,
        (min, max): (u64, u64),
        passable: &dyn Fn(usize) -> bool,
        target: &dyn Fn(usize) -> bool,
    ) -> bool {
        for (other, rel) in self.steps(node, direction) {
            if used.contains(&rel) || !passable(other) {
                continue;
            }
            if target(other) && depth + 1 >= min {
                return true;
            }
            if depth + 1 < max {
                used.push(rel);
                let found = self.reaches(
                    other,
                    depth + 1,
                    used,
                    direction,
                    (min, max),
                    passable,
                    target,
                );
                used.pop();
                if found {
                    return true;
                }
            }
        }
        false
    }
}

#[test]
fn both_plans_agree_with_a_trail_oracle_on_random_graphs() {
    oracle_rounds(setup);
}

#[test]
fn both_plans_agree_with_a_trail_oracle_reading_adjacency_from_storage() {
    oracle_rounds(setup_without_graphs);
}

fn oracle_rounds(setup: fn() -> Lmpg) {
    let mut seed = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = move |bound: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        usize::try_from(seed % bound as u64).unwrap()
    };
    for round in 0..12 {
        let lmpg = setup();
        let nodes = 10 + next(8);
        let graph = Graph {
            labels: (0..nodes)
                .map(|_| if next(5) == 0 { "Q" } else { "P" })
                .collect(),
            cities: (0..nodes).map(|_| ["x", "y", "z"][next(3)]).collect(),
            edges: (0..nodes * 2)
                .map(|_| {
                    (
                        next(nodes),
                        next(nodes),
                        if next(6) == 0 { "L" } else { "K" },
                    )
                })
                .collect(),
        };
        let refs: Vec<RecordRef> = (0..nodes)
            .map(|i| person(&lmpg, graph.labels[i], i as u64, graph.cities[i]))
            .collect();
        for &(source, target, ty) in &graph.edges {
            lmpg.create_relationship(ty, refs[source], refs[target], vec![])
                .unwrap();
        }
        let anchor = next(nodes);
        for direction in ["out", "in", "both"] {
            for hops in [(1, 1), (1, 2), (2, 2), (2, 3), (1, 3), (3, 3)] {
                for (target_label, filter) in [
                    ("P", json!({"node_id": anchor})),
                    ("P", json!({"city": "y"})),
                    ("Q", json!({})),
                    ("P", json!({})),
                ] {
                    let mut block = json!({"$relation": {"type": "K", "direction": direction,
                        "hops": {"min": hops.0, "max": hops.1}}});
                    for (key, value) in filter.as_object().unwrap() {
                        block[key] = value.clone();
                    }
                    let document = json!({"labels": ["P"], "where": {target_label: block}});
                    let passable = |node: usize| graph.labels[node] == target_label;
                    let target = |node: usize| {
                        passable(node)
                            && filter.as_object().unwrap().iter().all(|(key, value)| {
                                match key.as_str() {
                                    "node_id" => value.as_u64() == Some(node as u64),
                                    _ => value.as_str() == Some(graph.cities[node]),
                                }
                            })
                    };
                    let expected: BTreeSet<u64> = (0..nodes)
                        .filter(|&node| graph.labels[node] == "P")
                        .filter(|&node| {
                            graph.reaches(
                                node,
                                0,
                                &mut Vec::new(),
                                direction,
                                hops,
                                &passable,
                                &target,
                            )
                        })
                        .map(|node| node as u64)
                        .collect();
                    let (planned, checked) = both_plans(&lmpg, &document);
                    assert_eq!(planned, expected, "round {round}: {document}");
                    assert_eq!(checked, expected, "round {round} per candidate: {document}");
                }
            }
        }
    }
}

/// Backend reads of one anchored two-hop query on a ring lattice of `nodes`
/// records (each linked to its next three), with `node_id` indexed.
fn anchored_reads(nodes: u64) -> u64 {
    let lmpg = setup();
    let refs: Vec<RecordRef> = (0..nodes).map(|id| person(&lmpg, "P", id, "x")).collect();
    for i in 0..refs.len() {
        for step in 1..=3 {
            lmpg.create_relationship("K", refs[i], refs[(i + step) % refs.len()], vec![])
                .unwrap();
        }
    }
    let def = lmpg
        .define_index("p_node_id", "P", "node_id", IndexKind::Ordered)
        .unwrap();
    while !matches!(
        lmpg.index_step(def.id, 10_000).unwrap(),
        IndexStep::Finished | IndexStep::Idle
    ) {}
    let req = request(&neighbourhood_query(nodes / 2, "both", 1, 2));
    rushdb_storage::read_accounting::enable(true);
    let _ = rushdb_storage::read_accounting::take_local();
    let found = node_ids(&lmpg, &req);
    let reads = rushdb_storage::read_accounting::take_local().values().sum();
    // Six neighbours at one hop and six more at two; the anchor's shortest
    // closed trail is a triangle, three hops.
    assert_eq!(found.len(), 12, "{found:?}");
    reads
}

#[test]
fn an_anchored_neighbourhood_costs_the_same_in_a_ten_times_larger_label() {
    let small = anchored_reads(400);
    let large = anchored_reads(4_000);
    assert!(
        large <= small + small / 5,
        "reads grew with the label: {small} at 400 records, {large} at 4,000"
    );
}

#[test]
fn traversals_build_a_resident_graph_once_their_reads_pay_for_it() {
    let lmpg = setup();
    let refs: Vec<RecordRef> = (0..200).map(|id| person(&lmpg, "P", id, "x")).collect();
    for i in 0..refs.len() {
        lmpg.create_relationship("K", refs[i], refs[(i + 1) % refs.len()], vec![])
            .unwrap();
    }
    // 200 relationships: one build reads 200 rows. Each query below reads
    // about 2 scans and a few rows per visited record.
    let req = request(&neighbourhood_query(100, "both", 1, 3));
    let mut spent = 0;
    let mut queries = 0;
    loop {
        let (found, reads) = adjacency_reads(&lmpg, &req);
        assert_eq!(found, BTreeSet::from([97, 98, 99, 101, 102, 103]));
        queries += 1;
        if reads == 0 || queries > 20 {
            break;
        }
        spent += reads;
    }
    assert!(queries <= 20, "never built a resident graph");
    assert!(
        spent >= 150,
        "built after {spent} rows read, less than one build's cost"
    );
    // Resident from now on, at this snapshot.
    assert_eq!(adjacency_reads(&lmpg, &req).1, 0);

    // The same traversal on a store without a graph cache always reads rows.
    let plain = setup_without_graphs();
    let refs: Vec<RecordRef> = (0..200).map(|id| person(&plain, "P", id, "x")).collect();
    for i in 0..refs.len() {
        plain
            .create_relationship("K", refs[i], refs[(i + 1) % refs.len()], vec![])
            .unwrap();
    }
    for _ in 0..30 {
        assert!(adjacency_reads(&plain, &req).1 > 0);
    }
}

#[test]
fn a_resident_graph_answers_only_its_own_snapshot() {
    let lmpg = setup();
    let refs: Vec<RecordRef> = (0..50).map(|id| person(&lmpg, "P", id, "x")).collect();
    for i in 0..refs.len() - 1 {
        lmpg.create_relationship("K", refs[i], refs[i + 1], vec![])
            .unwrap();
    }
    let req = request(&neighbourhood_query(10, "out", 1, 1));
    let before = Arc::new(lmpg.store.read_view().unwrap());
    // Read until the graph of this snapshot is resident.
    while adjacency_reads(&lmpg, &req).1 > 0 {}
    lmpg.create_relationship("K", refs[30], refs[10], vec![])
        .unwrap();
    let (found, reads) = adjacency_reads(&lmpg, &req);
    assert_eq!(
        found,
        BTreeSet::from([9, 30]),
        "the new relationship is seen"
    );
    assert!(
        reads > 0,
        "a commit moved the reader past the resident graph"
    );
    while adjacency_reads(&lmpg, &req).1 > 0 {}
    assert_eq!(adjacency_reads(&lmpg, &req).0, BTreeSet::from([9, 30]));
    let pinned: BTreeSet<u64> = find_records_in_view(&lmpg, &req, &|| false, before)
        .unwrap()
        .records
        .iter()
        .map(|record| record["node_id"].as_u64().unwrap())
        .collect();
    assert_eq!(
        pinned,
        BTreeSet::from([9]),
        "the older snapshot keeps its answer"
    );
}
