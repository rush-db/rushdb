//! Aggregate outputs that read through a traversal alias or an expression:
//! `"$alias.field"` in `select` and `groupBy`, alias metrics and `$collect`
//! under a block nested below the alias, `$sum` over an expression, and
//! `order` on an aggregate's outputs.

use std::sync::Arc;

use rushdb_lmpg::{import, Lmpg};
use rushdb_query::document::parse_document;
use rushdb_query::{aggregate, aggregate_to_rows, find_records};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::{json, Value};

/// CUSTOMER Ann (Berlin) → ORDER O1 (paid) → PRODUCT Pen (office), Lamp
/// (home); Ann → ORDER O2 → Mug (home); Bob (Paris) → O3 → Pencil (office);
/// Cat (Berlin) → O4 (paid) → Chair (home).
fn shop() -> Lmpg {
    let lmpg = Lmpg::new(Arc::new(Store::new_in_memory()), TenantId::from_ordinal(77));
    let doc = |label: &str, fields: Value| import::import_document(&lmpg, label, &fields).unwrap();
    let customers = [
        (
            "Ann",
            "Berlin",
            vec![
                (
                    "O1",
                    "paid",
                    vec![("Pen", "office", 2, 5), ("Lamp", "home", 30, 1)],
                ),
                ("O2", "open", vec![("Mug", "home", 8, 2)]),
            ],
        ),
        (
            "Bob",
            "Paris",
            vec![("O3", "open", vec![("Pencil", "office", 1, 10)])],
        ),
        (
            "Cat",
            "Berlin",
            vec![("O4", "paid", vec![("Chair", "home", 50, 2)])],
        ),
    ];
    for (name, city, orders) in customers {
        let customer = doc("CUSTOMER", json!({"name": name, "city": city}));
        for (code, status, products) in orders {
            let order = doc("ORDER", json!({"code": code, "status": status}));
            lmpg.create_relationship("PLACED", customer, order, vec![])
                .unwrap();
            for (product, category, price, qty) in products {
                let product = doc(
                    "PRODUCT",
                    json!({"name": product, "category": category, "price": price, "qty": qty}),
                );
                lmpg.create_relationship("CONTAINS", order, product, vec![])
                    .unwrap();
            }
        }
    }
    lmpg
}

fn rows(lmpg: &Lmpg, document: &Value) -> Vec<Value> {
    let parsed = parse_document(document).unwrap();
    let req = parsed.aggregate.expect("an aggregation");
    let buckets = aggregate(lmpg, &req).unwrap();
    aggregate_to_rows(lmpg, &buckets, &req).unwrap()
}

fn sorted_by(mut rows: Vec<Value>, key: &str) -> Vec<Value> {
    rows.sort_by(|a, b| a[key].to_string().cmp(&b[key].to_string()));
    rows
}

#[test]
fn a_sum_over_an_expression_is_a_row_output() {
    let lmpg = shop();
    let revenue = json!({"$sum": {"$multiply": ["$record.price", "$record.qty"]}});
    assert_eq!(
        rows(
            &lmpg,
            &json!({"labels": ["PRODUCT"], "select": {"revenue": revenue}})
        ),
        vec![json!({"revenue": 166.0})]
    );
    assert_eq!(
        sorted_by(
            rows(
                &lmpg,
                &json!({"labels": ["PRODUCT"], "select": {"revenue": revenue},
                        "groupBy": ["$record.category"]})
            ),
            "category"
        ),
        vec![
            json!({"category": "home", "revenue": 146.0}),
            json!({"category": "office", "revenue": 20.0}),
        ]
    );
    let doubled = json!({"$sum": {"$add": ["$record.price", "$record.price"]}});
    assert_eq!(
        rows(
            &lmpg,
            &json!({"labels": ["PRODUCT"], "select": {"d": doubled}})
        ),
        vec![json!({"d": 182.0})]
    );
    // No match: the global rollup's expression sum is null, like `$sum`.
    assert_eq!(
        rows(
            &lmpg,
            &json!({"labels": ["PRODUCT"], "where": {"price": 999}, "select": {"revenue": revenue}})
        ),
        vec![json!({"revenue": null})]
    );
}

#[test]
fn group_by_an_alias_field_groups_by_the_related_record() {
    let lmpg = shop();
    let grouped = rows(
        &lmpg,
        &json!({"labels": ["ORDER"], "where": {"CUSTOMER": {"$alias": "$c"}},
                "select": {"n": {"$count": "*"}}, "groupBy": ["$c.city"],
                "orderBy": {"n": "desc"}}),
    );
    assert_eq!(
        grouped,
        vec![
            json!({"city": "Berlin", "n": 3}),
            json!({"city": "Paris", "n": 1})
        ]
    );
}

#[test]
fn an_alias_field_in_select_is_read_from_the_related_record() {
    let lmpg = shop();
    let document = json!({"labels": ["ORDER"], "where": {"CUSTOMER": {"$alias": "$c"}},
        "select": {"code": "$record.code", "city": "$c.city"}});
    let projected = rows(&lmpg, &document);
    assert_eq!(
        sorted_by(projected, "code"),
        vec![
            json!({"code": "O1", "city": "Berlin"}),
            json!({"code": "O2", "city": "Berlin"}),
            json!({"code": "O3", "city": "Paris"}),
            json!({"code": "O4", "city": "Berlin"}),
        ]
    );
    // With a metric beside it, too.
    let with_metric = json!({"labels": ["ORDER"], "where": {"CUSTOMER": {"$alias": "$c"}},
        "select": {"code": "$record.code", "city": "$c.city", "n": {"$count": "*"}}});
    for row in rows(&lmpg, &with_metric) {
        assert!(row.get("city").is_some(), "{row}");
    }
    // Without an alias the same select is a listing, as before.
    let listing =
        parse_document(&json!({"labels": ["ORDER"], "select": {"code": "$record.code"}})).unwrap();
    assert!(listing.aggregate.is_none());
    assert_eq!(
        find_records(&lmpg, &listing.search).unwrap().records.len(),
        4
    );
}

#[test]
fn an_alias_reference_to_an_unbound_alias_is_refused() {
    for document in [
        json!({"labels": ["ORDER"], "where": {"CUSTOMER": {"$alias": "$c"}},
               "select": {"n": {"$count": "*"}}, "groupBy": ["$nope.city"]}),
        json!({"labels": ["ORDER"], "select": {"n": {"$count": "*"}}, "groupBy": ["$related.city"]}),
        json!({"labels": ["ORDER"], "select": {"code": "$record.code", "city": "$c.city"}}),
    ] {
        let Err(error) = parse_document(&document) else {
            panic!("{document} was accepted");
        };
        let error = error.to_string();
        assert!(
            error.contains("which no `$alias` in `where` binds"),
            "{document}: {error}"
        );
    }
}

#[test]
fn an_alias_binds_only_records_matching_the_blocks_below_it() {
    let lmpg = shop();
    let office = json!({"ORDER": {"$alias": "$o", "PRODUCT": {"category": "office"}}});
    let counted = rows(
        &lmpg,
        &json!({"labels": ["CUSTOMER"], "where": office,
                "select": {"name": "$record.name", "orders": {"$count": "$o"}},
                "groupBy": ["$record.name"]}),
    );
    assert_eq!(
        sorted_by(counted, "name"),
        vec![
            json!({"name": "Ann", "orders": 1}),
            json!({"name": "Bob", "orders": 1})
        ]
    );
    // `$collect` from the alias binds the same orders.
    let parsed = parse_document(&json!({"labels": ["CUSTOMER"], "where": office,
        "select": {"name": "$record.name",
                   "orders": {"$collect": {"from": "$o", "select": {"code": "$o.code"}}}}}))
    .unwrap();
    let records = find_records(&lmpg, &parsed.search).unwrap().records;
    let ann = records
        .iter()
        .find(|r| r["name"] == json!("Ann"))
        .expect("Ann");
    let codes: Vec<&Value> = ann["orders"]
        .as_array()
        .expect("collected orders")
        .iter()
        .map(|o| &o["code"])
        .collect();
    assert_eq!(codes, vec![&json!("O1")]);
    // An alias bound below the block still sees the full path.
    let products = rows(
        &lmpg,
        &json!({"labels": ["CUSTOMER"],
                "where": {"ORDER": {"status": "paid", "PRODUCT": {"$alias": "$p"}}},
                "select": {"name": "$record.name", "products": {"$count": "$p"}},
                "groupBy": ["$record.name"]}),
    );
    assert_eq!(
        sorted_by(products, "name"),
        vec![
            json!({"name": "Ann", "products": 2}),
            json!({"name": "Cat", "products": 1})
        ]
    );
}

#[test]
fn an_aggregate_is_ordered_by_its_outputs_only() {
    let document = |order: Value| {
        json!({"labels": ["PRODUCT"], "select": {"total": {"$sum": "$record.price"}},
               "groupBy": ["$record.category"], "orderBy": order})
    };
    let Err(error) = parse_document(&document(json!({"totl": "desc"}))) else {
        panic!("an unknown order key was accepted");
    };
    let error = error.to_string();
    assert!(
        error.contains("`totl`") && error.contains("`total`") && error.contains("`category`"),
        "{error}"
    );
    let lmpg = shop();
    assert_eq!(
        rows(&lmpg, &document(json!({"total": "desc"}))),
        vec![
            json!({"category": "home", "total": 88.0}),
            json!({"category": "office", "total": 3.0})
        ]
    );
    assert_eq!(
        rows(&lmpg, &document(json!({"category": "asc"})))[0]["category"],
        json!("home")
    );
    // A listing may order by a property no record has.
    let listing = parse_document(&json!({"labels": ["PRODUCT"], "orderBy": {"nothing": "asc"}}));
    assert!(listing.is_ok());
}
