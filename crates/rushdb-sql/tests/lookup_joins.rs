//! Joins through relationship tables start from the selective side and
//! fetch the rest by key: `review JOIN edges JOIN business WHERE
//! business.is_open = 0` reads the closed businesses, their edges through
//! adjacency and their reviews by id, whatever the other reviews number.

use std::collections::BTreeSet;
use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{BatchOp, BatchTx, Lmpg};
use rushdb_sql::{
    execute_sql, execute_sql_with_parameters_budget, reset_work_counters, take_work_counters,
    SqlError,
};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::{json, Value};

const BUSINESSES: usize = 200;

/// 200 businesses (every 20th closed, `is_open = 0`) and `reviews` reviews,
/// review `n` linked to business `n % 200` by `HAS_BUSINESS`; the reviews
/// past the first 4,000 all belong to open businesses.
fn yelp(reviews: usize) -> Lmpg {
    let lmpg = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::single());
    let businesses = lmpg
        .run_batch(BatchTx {
            ops: (0..BUSINESSES)
                .map(|n| BatchOp::CreateRecord {
                    label: "Business".into(),
                    explicit_id: None,
                    fields: vec![
                        ("name".into(), FieldChange::Set(json!(format!("b{n:03}")))),
                        (
                            "is_open".into(),
                            FieldChange::Set(json!(i64::from(n % 20 != 0))),
                        ),
                    ],
                })
                .collect(),
        })
        .unwrap()
        .created_records;
    for chunk in (0..reviews).collect::<Vec<_>>().chunks(5_000) {
        let created = lmpg
            .run_batch(BatchTx {
                ops: chunk
                    .iter()
                    .map(|&n| BatchOp::CreateRecord {
                        label: "Review".into(),
                        explicit_id: None,
                        fields: vec![
                            ("review_id".into(), FieldChange::Set(json!(n))),
                            ("stars".into(), FieldChange::Set(json!(n % 5 + 1))),
                        ],
                    })
                    .collect(),
            })
            .unwrap()
            .created_records;
        let ops = chunk
            .iter()
            .zip(created)
            .map(|(&n, review)| {
                // Past the first 4,000, only open businesses get reviews.
                let business = if n < 4_000 {
                    n % BUSINESSES
                } else {
                    n % 19 + 1
                };
                BatchOp::CreateRelationship {
                    rel_type: "HAS_BUSINESS".into(),
                    source: review,
                    target: businesses[business],
                    fields: Vec::new(),
                }
            })
            .collect();
        lmpg.run_batch(BatchTx { ops }).unwrap();
    }
    lmpg
}

const CLOSED_REVIEWS: &str =
    "SELECT DISTINCT r.__id, r.review_id, r.stars, b.name AS business_name \
     FROM review r JOIN rushdb_edges.has_business e ON e.__source_id = r.__id \
     JOIN business b ON b.__id = e.__target_id \
     WHERE b.is_open = 0 \
     ORDER BY r.stars ASC NULLS FIRST, b.name ASC NULLS LAST, r.__id ASC";

fn plan(lmpg: &Lmpg, sql: &str) -> String {
    execute_sql(lmpg, &format!("EXPLAIN {sql}"))
        .unwrap()
        .iter()
        .rev()
        .find_map(|row| row.get("plan").and_then(Value::as_str).map(str::to_owned))
        .unwrap()
}

#[test]
fn a_join_through_edges_starts_from_the_selective_side() {
    let lmpg = yelp(4_000);
    let plan = plan(&lmpg, CLOSED_REVIEWS);
    // Closed businesses, then their edges by target, then their reviews by
    // id: no hash join builds over a whole label.
    let lines: Vec<&str> = plan.lines().map(str::trim).collect();
    let at = |needle: &str| lines.iter().position(|line| line.contains(needle));
    let (reviews, edges, scan) = (
        at("LookupJoinExec: key=__id, scan=label Review").expect(&plan),
        at("LookupJoinExec: key=__target_id, scan=relationship HAS_BUSINESS").expect(&plan),
        at("RushScan: label=Business").expect(&plan),
    );
    assert!(reviews < edges && edges < scan, "{plan}");
    assert!(!plan.contains("HashJoinExec"), "{plan}");

    // The 200 reviews of the 10 closed businesses, each with its business.
    let rows = execute_sql(&lmpg, CLOSED_REVIEWS).unwrap();
    assert_eq!(rows.len(), 200);
    let ids: BTreeSet<u64> = rows
        .iter()
        .map(|row| row["review_id"].as_u64().unwrap())
        .collect();
    assert_eq!(ids, (0..4_000).filter(|n| n % 20 == 0).collect());
    for row in &rows {
        let n = row["review_id"].as_u64().unwrap();
        assert_eq!(
            row["business_name"],
            json!(format!("b{:03}", n % 200)),
            "{row:?}"
        );
    }
    // Ordered by stars, then business name, then id.
    let keys: Vec<(u64, String)> = rows
        .iter()
        .map(|row| {
            (
                row["stars"].as_u64().unwrap(),
                row["business_name"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert!(keys.windows(2).all(|pair| pair[0] <= pair[1]));
}

#[test]
fn a_join_through_edges_reads_the_matches_not_the_label() {
    // Ten times the reviews, all of open businesses: the closed businesses'
    // reviews are found through adjacency, so the records read stay the same.
    let read = |reviews: usize| {
        let lmpg = yelp(reviews);
        reset_work_counters();
        assert_eq!(execute_sql(&lmpg, CLOSED_REVIEWS).unwrap().len(), 200);
        take_work_counters().reconstructed_rows
    };
    let (small, large) = (read(4_000), read(40_000));
    assert_eq!(
        small, large,
        "{small} records read at 4,000 reviews, {large} at 40,000"
    );
}

#[test]
fn lookup_and_hash_joins_agree() {
    // Joined on a non-key column the planner keeps its hash join; on the
    // key it looks rows up. Both read the same pairs.
    let lmpg = yelp(4_000);
    let by_key = execute_sql(
        &lmpg,
        "SELECT r.review_id AS r, b.name AS b FROM review r \
         JOIN rushdb_edges.has_business e ON e.__source_id = r.__id \
         JOIN business b ON b.__id = e.__target_id WHERE r.stars = 5 ORDER BY r.review_id",
    )
    .unwrap();
    let by_value = execute_sql(
        &lmpg,
        "SELECT r.review_id AS r, b.name AS b FROM review r \
         JOIN rushdb_edges.has_business e ON lower(e.__source_id) = lower(r.__id) \
         JOIN business b ON lower(b.__id) = lower(e.__target_id) WHERE r.stars = 5 ORDER BY r.review_id",
    )
    .unwrap();
    assert_eq!(by_key.len(), 800);
    assert_eq!(by_key, by_value);
    // Counting joined rows projects no column and still counts them.
    let count = execute_sql(
        &lmpg,
        "SELECT COUNT(*) AS c FROM business b \
         JOIN rushdb_edges.has_business e ON b.__id = e.__target_id WHERE b.is_open = 0",
    )
    .unwrap();
    assert_eq!(count[0]["c"], json!(200));
}

#[test]
fn each_limit_names_itself() {
    let lmpg = yelp(400);
    let error =
        execute_sql_with_parameters_budget(&lmpg, CLOSED_REVIEWS, None, Some(64)).unwrap_err();
    assert!(matches!(error, SqlError::ResultTooLarge(64)), "{error:?}");
    assert!(
        error.to_string().contains("response size limit (64 bytes)"),
        "{error}"
    );
    let row_limit = SqlError::RowLimit(100_001).to_string();
    assert!(
        row_limit.contains("100000 row safety limit (100001 rows)"),
        "{row_limit}"
    );
}

/// 300 reviews (`stars` = n % 5, a long `text`) of 30 businesses; every
/// tenth review is linked to its business twice, so a join row repeats.
fn reviews_with_text() -> Lmpg {
    let lmpg = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::single());
    let businesses = lmpg
        .run_batch(BatchTx {
            ops: (0..30)
                .map(|n| BatchOp::CreateRecord {
                    label: "Business".into(),
                    explicit_id: None,
                    fields: vec![
                        ("name".into(), FieldChange::Set(json!(format!("b{n:02}")))),
                        ("is_open".into(), FieldChange::Set(json!(1))),
                    ],
                })
                .collect(),
        })
        .unwrap()
        .created_records;
    let reviews = lmpg
        .run_batch(BatchTx {
            ops: (0..300)
                .map(|n| BatchOp::CreateRecord {
                    label: "Review".into(),
                    explicit_id: None,
                    fields: vec![
                        ("stars".into(), FieldChange::Set(json!(n % 5))),
                        (
                            "text".into(),
                            FieldChange::Set(json!(format!("review {n} {}", "x".repeat(200)))),
                        ),
                    ],
                })
                .collect(),
        })
        .unwrap()
        .created_records;
    let mut ops = Vec::new();
    for (n, review) in reviews.into_iter().enumerate() {
        for _ in 0..if n % 10 == 0 { 2 } else { 1 } {
            ops.push(BatchOp::CreateRelationship {
                rel_type: "HAS_BUSINESS".into(),
                source: review,
                target: businesses[n % 30],
                fields: Vec::new(),
            });
        }
    }
    lmpg.run_batch(BatchTx { ops }).unwrap();
    lmpg
}

#[test]
fn a_top_k_reads_wide_columns_for_its_rows_only() {
    let lmpg = reviews_with_text();
    for distinct in ["DISTINCT ", ""] {
        let base = format!(
            "SELECT {distinct}r.__id, r.stars, r.text, b.name FROM review r \
             JOIN rushdb_edges.has_business e ON e.__source_id = r.__id \
             JOIN business b ON b.__id = e.__target_id WHERE b.is_open = 1 \
             ORDER BY r.stars ASC NULLS FIRST, b.name ASC NULLS LAST, r.__id ASC"
        );
        let all = execute_sql(&lmpg, &base).unwrap();
        // Without DISTINCT the repeated edges repeat rows.
        assert_eq!(all.len(), if distinct.is_empty() { 330 } else { 300 });
        for limit in [1, 7, 25, 400] {
            let limited = format!("{base} LIMIT {limit}");
            let plan = plan(&lmpg, &limited);
            // Reviews are looked up twice: their ordering columns for every
            // match, their `text` for the rows returned (fewer than the
            // label holds; past that a hash join of the label is cheaper).
            if limit < 300 {
                assert_eq!(
                    plan.matches("LookupJoinExec: key=__id, scan=label Review")
                        .count(),
                    2,
                    "{plan}"
                );
            }
            assert_eq!(
                plan.contains("DistinctTopKExec"),
                !distinct.is_empty(),
                "{plan}"
            );
            let rows = execute_sql(&lmpg, &limited).unwrap();
            let expected: Vec<_> = all.iter().take(limit).cloned().collect();
            assert_eq!(rows, expected, "{limited}");
        }
    }
}

#[test]
fn counting_distinct_rows_groups_by_the_record_key() {
    // How the platform totals a search: a count over the search's own query.
    let lmpg = reviews_with_text();
    let count = "SELECT count(*) AS total FROM (SELECT DISTINCT r.__id, r.stars, r.text, b.name \
         FROM review r JOIN rushdb_edges.has_business e ON e.__source_id = r.__id \
         JOIN business b ON b.__id = e.__target_id WHERE b.is_open = 1 \
         ORDER BY r.stars ASC NULLS FIRST, b.name ASC NULLS LAST, r.__id ASC) AS subquery";
    assert_eq!(execute_sql(&lmpg, count).unwrap()[0]["total"], json!(300));
    // The DISTINCT groups by the review's id and the business name, not the
    // review's other columns.
    let plan = plan(&lmpg, count);
    let grouping = plan
        .lines()
        .find(|line| {
            line.contains("AggregateExec") && line.contains("gby=[") && !line.contains("gby=[]")
        })
        .expect(&plan);
    assert!(
        !grouping.contains("text") && !grouping.contains("stars"),
        "{plan}"
    );
    // Without the count, the same DISTINCT keeps every column.
    let rows = execute_sql(
        &lmpg,
        &count["SELECT count(*) AS total FROM (".len()..count.len() - ") AS subquery".len()],
    )
    .unwrap();
    assert_eq!(rows.len(), 300);
    assert!(rows.iter().all(|row| row["text"]
        .as_str()
        .is_some_and(|text| text.starts_with("review "))));
}

/// 300 reviews of 30 businesses (review `n` → business `n % 30`), each
/// business located in one of 3 cities (business `m` → city `m % 3`), and 5
/// businesses named `a…` with no review and no city.
fn three_hops() -> Lmpg {
    let lmpg = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::single());
    let create = |label: &str, fields: Vec<Vec<(String, FieldChange)>>| {
        lmpg.run_batch(BatchTx {
            ops: fields
                .into_iter()
                .map(|fields| BatchOp::CreateRecord {
                    label: label.into(),
                    explicit_id: None,
                    fields,
                })
                .collect(),
        })
        .unwrap()
        .created_records
    };
    let named = |name: String| vec![("name".into(), FieldChange::Set(json!(name)))];
    let cities = create("City", (0..3).map(|n| named(format!("c{n}"))).collect());
    let businesses = create(
        "Business",
        (0..30).map(|n| named(format!("b{n:02}"))).collect(),
    );
    create(
        "Business",
        (0..5)
            .map(|n| {
                let mut fields = named(format!("a{n}"));
                fields.push((
                    "text".into(),
                    FieldChange::Set(json!(format!("lonely {n}"))),
                ));
                fields
            })
            .collect(),
    );
    let reviews = create(
        "Review",
        (0..300)
            .map(|n| {
                vec![
                    ("review_id".into(), FieldChange::Set(json!(n))),
                    (
                        "text".into(),
                        FieldChange::Set(json!(format!("review {n} {}", "x".repeat(100)))),
                    ),
                ]
            })
            .collect(),
    );
    let link = |rel_type: &str, source, target| BatchOp::CreateRelationship {
        rel_type: rel_type.into(),
        source,
        target,
        fields: Vec::new(),
    };
    let mut ops: Vec<BatchOp> = reviews
        .iter()
        .enumerate()
        .map(|(n, &review)| link("HAS_BUSINESS", review, businesses[n % 30]))
        .collect();
    ops.extend(
        businesses
            .iter()
            .enumerate()
            .map(|(m, &business)| link("LOCATED_IN", business, cities[m % 3])),
    );
    lmpg.run_batch(BatchTx { ops }).unwrap();
    lmpg
}

#[test]
fn a_lookup_reads_its_own_key_when_names_repeat() {
    // Every table's `__id` is selected, so past the first lookup the stream
    // carries several columns named `__id` (and endpoint names): each lookup
    // must key on its own column, not on the first one of that name.
    let lmpg = three_hops();
    let query = |on: fn(&str, &str) -> String| {
        format!(
            "SELECT r.__id, b.__id AS bid, c.__id AS cid, r.review_id AS r, b.name AS b, \
             c.name AS c FROM review r \
             JOIN rushdb_edges.has_business e ON {} \
             JOIN business b ON {} \
             JOIN rushdb_edges.located_in l ON {} \
             JOIN city c ON {} ORDER BY r.review_id",
            on("e.__source_id", "r.__id"),
            on("b.__id", "e.__target_id"),
            on("l.__source_id", "b.__id"),
            on("c.__id", "l.__target_id"),
        )
    };
    let by_key = query(|a, b| format!("{a} = {b}"));
    let plan = plan(&lmpg, &by_key);
    // Every join is a lookup, so each one reads its key out of a stream
    // that repeats `__id` and an endpoint name.
    assert_eq!(plan.matches("LookupJoinExec").count(), 4, "{plan}");
    let rows = execute_sql(&lmpg, &by_key).unwrap();
    assert_eq!(rows.len(), 300);
    for row in &rows {
        let n = row["r"].as_u64().unwrap();
        assert_eq!(row["b"], json!(format!("b{:02}", n % 30)), "{row:?}");
        assert_eq!(row["c"], json!(format!("c{}", n % 30 % 3)), "{row:?}");
    }
    // The same pairs as hash joins on computed keys.
    let by_value = execute_sql(&lmpg, &query(|a, b| format!("lower({a}) = lower({b})"))).unwrap();
    assert_eq!(rows, by_value);
}

#[test]
fn an_outer_join_keeps_its_unmatched_rows_under_a_top_k() {
    // `r.text` rides on `r.__id`, which the left join pads with null for the
    // five businesses without reviews: those rows must stay, with null text.
    let lmpg = three_hops();
    let sql = "SELECT b.__id, b.name, r.__id AS rid, r.text FROM business b \
               LEFT JOIN rushdb_edges.has_business e ON e.__target_id = b.__id \
               LEFT JOIN review r ON r.__id = e.__source_id \
               ORDER BY b.name ASC, r.review_id ASC NULLS FIRST";
    let all = execute_sql(&lmpg, sql).unwrap();
    assert_eq!(all.len(), 305);
    for limit in [3, 8, 40] {
        let rows = execute_sql(&lmpg, &format!("{sql} LIMIT {limit}")).unwrap();
        let expected: Vec<_> = all.iter().take(limit).cloned().collect();
        assert_eq!(rows, expected, "LIMIT {limit}");
    }
    let first = &all[0];
    assert_eq!(first["name"], json!("a0"));
    assert_eq!(first["rid"], Value::Null);
    assert_eq!(first["text"], Value::Null);
    // The preserved side still defers its own wide column.
    let lonely = "SELECT b.__id, b.name, b.text, r.__id AS rid FROM business b \
                  LEFT JOIN rushdb_edges.has_business e ON e.__target_id = b.__id \
                  LEFT JOIN review r ON r.__id = e.__source_id \
                  ORDER BY b.name ASC, r.review_id ASC NULLS FIRST";
    let rows = execute_sql(&lmpg, &format!("{lonely} LIMIT 5")).unwrap();
    assert_eq!(rows.len(), 5);
    assert!(rows.iter().all(|row| row["text"]
        .as_str()
        .is_some_and(|t| t.starts_with("lonely "))));
}

/// One business with 3,000 reviews (more than a fetch batch of edges) and
/// one with 2.
fn hub() -> Lmpg {
    let lmpg = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::single());
    let businesses = lmpg
        .run_batch(BatchTx {
            ops: ["hub", "small"]
                .into_iter()
                .map(|name| BatchOp::CreateRecord {
                    label: "Business".into(),
                    explicit_id: None,
                    fields: vec![("name".into(), FieldChange::Set(json!(name)))],
                })
                .collect(),
        })
        .unwrap()
        .created_records;
    let reviews = lmpg
        .run_batch(BatchTx {
            ops: (0..3_002)
                .map(|n| BatchOp::CreateRecord {
                    label: "Review".into(),
                    explicit_id: None,
                    fields: vec![("review_id".into(), FieldChange::Set(json!(n)))],
                })
                .collect(),
        })
        .unwrap()
        .created_records;
    let ops = reviews
        .into_iter()
        .enumerate()
        .map(|(n, review)| BatchOp::CreateRelationship {
            rel_type: "HAS_BUSINESS".into(),
            source: review,
            target: businesses[usize::from(n >= 3_000)],
            fields: Vec::new(),
        })
        .collect();
    lmpg.run_batch(BatchTx { ops }).unwrap();
    lmpg
}

#[test]
fn a_hub_is_joined_a_batch_of_edges_at_a_time() {
    let lmpg = hub();
    // Endpoint ids alone (read from adjacency) and full relationship rows.
    for (select, from) in [
        ("count(*) AS c", ""),
        (
            "count(DISTINCT r.review_id) AS c",
            " JOIN review r ON r.__id = e.__source_id",
        ),
    ] {
        let sql = format!(
            "SELECT b.name, {select} FROM business b \
             JOIN rushdb_edges.has_business e ON e.__target_id = b.__id{from} \
             GROUP BY b.name ORDER BY b.name"
        );
        assert!(plan(&lmpg, &sql).contains("LookupJoinExec"), "{sql}");
        let rows = execute_sql(&lmpg, &sql).unwrap();
        let counts: Vec<Value> = rows.into_iter().map(Value::Object).collect();
        assert_eq!(
            counts,
            vec![
                json!({ "name": "hub", "c": 3_000 }),
                json!({ "name": "small", "c": 2 }),
            ],
            "{sql}"
        );
    }
}

#[test]
fn an_edge_lookup_stops_at_the_deadline() {
    let lmpg = hub().with_request_limits(rushdb_lmpg::RequestLimits {
        deadline: Some(std::time::Instant::now()),
        ..Default::default()
    });
    let error = execute_sql(
        &lmpg,
        "SELECT count(*) AS c FROM business b \
         JOIN rushdb_edges.has_business e ON e.__target_id = b.__id",
    )
    .unwrap_err();
    assert!(matches!(error, SqlError::Cancelled), "{error:?}");
}

#[test]
fn a_distinct_top_k_counts_its_rows_against_the_cap() {
    let sql = "SELECT DISTINCT r.__id, r.stars, r.text FROM review r \
               ORDER BY r.stars ASC NULLS FIRST, r.text ASC LIMIT 100000";
    let lmpg = reviews_with_text();
    assert!(plan(&lmpg, sql).contains("DistinctTopKExec"));
    assert_eq!(execute_sql(&lmpg, sql).unwrap().len(), 300);
    // 300 rows of ~230 bytes each do not fit in 32 KiB.
    let capped = reviews_with_text().with_request_limits(rushdb_lmpg::RequestLimits {
        query_memory_max_bytes: Some(32 * 1024),
        ..Default::default()
    });
    let error = execute_sql(&capped, sql).unwrap_err();
    assert!(
        matches!(error, SqlError::MemoryLimit(cap) if cap == 32 * 1024),
        "{error:?}"
    );
}

#[test]
fn a_relationship_scan_stops_at_the_deadline() {
    let lmpg = hub().with_request_limits(rushdb_lmpg::RequestLimits {
        deadline: Some(std::time::Instant::now()),
        ..Default::default()
    });
    let error =
        execute_sql(&lmpg, "SELECT count(*) AS c FROM rushdb_edges.has_business").unwrap_err();
    assert!(matches!(error, SqlError::Cancelled), "{error:?}");
}
