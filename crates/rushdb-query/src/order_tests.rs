//! Every order-first page equals the same page read predicate-first (every
//! candidate read and sorted), which is the definition of the order.

use super::*;
use crate::matches::{plan_page, run_pass, Matches};
use rushdb_lmpg::plan::FieldChange;
use serde_json::json;

/// 60 `A` records (`p`: a number for most, an explicit null for every
/// seventh, missing for every fifth; `g` indexed, `hit` on every third), 40
/// `B` records without `p`, and 30 `C` records whose `p` is a string. Many
/// values repeat, so ties are ordered by ordinal.
fn store() -> Lmpg {
    let lmpg = Lmpg::new(
        Arc::new(rushdb_storage::Store::new_in_memory()),
        rushdb_types::ids::TenantId::from_ordinal(9),
    );
    let mut tx = lmpg.begin_batch();
    for n in 0u64..130 {
        let (label, mut fields) = match n {
            0..60 => {
                let mut fields = vec![(
                    "g".to_owned(),
                    FieldChange::Set(json!(if n % 3 == 0 { "hit" } else { "miss" })),
                )];
                if n % 5 != 0 {
                    let value = if n % 7 == 0 {
                        json!(null)
                    } else {
                        json!(n % 9)
                    };
                    fields.push(("p".to_owned(), FieldChange::Set(value)));
                }
                ("A", fields)
            }
            60..100 => ("B", vec![("q".to_owned(), FieldChange::Set(json!(n)))]),
            _ => (
                "C",
                vec![(
                    "p".to_owned(),
                    FieldChange::Set(json!(format!("s{}", n % 4))),
                )],
            ),
        };
        // Interleave the labels' ordinals.
        let label = match n % 3 {
            0 if label == "B" => "A",
            _ => label,
        };
        fields.sort_by(|a, b| a.0.cmp(&b.0));
        tx.ops.push(rushdb_lmpg::BatchOp::CreateRecord {
            label: label.into(),
            explicit_id: None,
            fields,
        });
    }
    lmpg.run_batch(tx).unwrap();
    // `g` filters; `p` orders through an index on `A` (integers) and `C`
    // (strings). Across both labels `p` has two types: no index order.
    for (name, label, property) in [("a_g", "A", "g"), ("a_p", "A", "p"), ("c_p", "C", "p")] {
        let def = lmpg
            .define_index(
                name,
                label,
                property,
                rushdb_types::index::IndexKind::Ordered,
            )
            .unwrap();
        while !matches!(
            lmpg.index_step(def.id, 10_000).unwrap(),
            rushdb_lmpg::index::IndexStep::Finished | rushdb_lmpg::index::IndexStep::Idle
        ) {}
    }
    lmpg
}

/// The page and total read with the order planner, and predicate-first.
fn both_ways(lmpg: &Lmpg, req: &SearchRequest, skip: usize, limit: usize) -> [(Vec<u64>, u64); 2] {
    let snap: QuerySnapshot = Arc::new(lmpg.store.read_view().unwrap());
    let page = |matches: Matches| {
        let roots = matches
            .retained
            .into_iter()
            .skip(skip.saturating_sub(matches.offset))
            .take(limit)
            .collect();
        (roots, matches.total)
    };
    [
        page(run_matches(lmpg, req, &|| false, &snap, skip, limit, None).unwrap()),
        page(
            run_pass(
                lmpg,
                req,
                &|| false,
                &snap,
                (skip, limit),
                plan_page(lmpg, req, &|| false, &snap, (skip, limit), None, false).unwrap(),
                0,
            )
            .unwrap()
            .unwrap(),
        ),
    ]
}

#[test]
fn order_first_pages_equal_predicate_first_pages() {
    let lmpg = store();
    let snap: QuerySnapshot = Arc::new(lmpg.store.read_view().unwrap());
    let mut sources = std::collections::BTreeSet::new();
    for labels in [
        json!([]),
        json!(["A"]),
        json!(["A", "B"]),
        json!(["C"]),
        json!(["B"]),
    ] {
        for filter in [json!({}), json!({"g": "hit"})] {
            for order in [
                json!({}),
                json!("asc"),
                json!("desc"),
                json!({"p": "asc"}),
                json!({"p": "desc"}),
            ] {
                for (skip, limit) in [
                    (0, 5),
                    (3, 4),
                    (20, 7),
                    (55, 10),
                    (95, 5),
                    (126, 9),
                    (400, 5),
                ] {
                    let document = json!({"labels": labels, "where": filter,
                        "orderBy": order, "skip": skip, "limit": limit});
                    let req = document::parse_document(&document).unwrap().search;
                    let [planned, oracle] = both_ways(&lmpg, &req, skip, limit);
                    assert_eq!(planned, oracle, "{document}");
                    if let Some(order) = explain_order(&lmpg, &req, &snap, skip, limit).unwrap() {
                        sources.insert((order.source, order.reverse_walk));
                    }
                }
            }
        }
    }
    // Every order-first path was taken somewhere above.
    for source in [
        ("storage", true),
        ("id", false),
        ("id", true),
        ("property", false),
        ("index", false),
    ] {
        assert!(
            sources.contains(&source),
            "{source:?} never planned: {sources:?}"
        );
    }
}

#[test]
fn explain_plans_the_order_from_counters_alone() {
    // Under a working-set cap no owner set fits in, explain still answers:
    // it reads counters, never the matches.
    let lmpg = store().with_request_limits(rushdb_lmpg::RequestLimits {
        query_memory_max_bytes: Some(1),
        ..Default::default()
    });
    let snap: QuerySnapshot = Arc::new(lmpg.store.read_view().unwrap());
    let unfiltered =
        document::parse_document(&json!({"labels": ["A"], "orderBy": "desc", "limit": 5}))
            .unwrap()
            .search;
    let order = explain_order(&lmpg, &unfiltered, &snap, 0, 5).unwrap();
    assert_eq!(order.map(|order| order.source), Some("id"));
    // A filtered page's choice rests on its exact match count: not reported.
    let filtered = document::parse_document(
        &json!({"labels": ["A"], "where": {"g": "hit"}, "orderBy": "desc", "limit": 5}),
    )
    .unwrap()
    .search;
    assert_eq!(explain_order(&lmpg, &filtered, &snap, 0, 5).unwrap(), None);
}
