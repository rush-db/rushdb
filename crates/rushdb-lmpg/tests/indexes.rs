//! Declared ordered indexes: entries are derived rows that always equal
//! a from-scratch derivation (the scrub oracle), during a build, under
//! concurrent writers, and through a drop.

use std::sync::Arc;

use rushdb_lmpg::index::{self, IndexStep};
use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{scrub, Lmpg};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use rushdb_types::index::{IndexKind, IndexState};
use serde_json::json;

const TENANT: TenantId = TenantId::from_ordinal(4040);

fn engine() -> Lmpg {
    Lmpg::new(Arc::new(Store::new_in_memory()), TENANT)
}

fn assert_oracle(eng: &Lmpg, context: &str) {
    let view = eng.store.read_view().unwrap();
    let violations = scrub::scrub(&view, eng.tenant).unwrap();
    assert!(violations.is_empty(), "{context}: {violations:?}");
}

fn entry_count(eng: &Lmpg, id: u32) -> usize {
    let view = eng.store.read_view().unwrap();
    index::entries(&view, eng.tenant, id, &[], &[])
        .unwrap()
        .len()
}

fn build(eng: &Lmpg, id: u32, batch: usize) {
    for _ in 0..10_000 {
        match eng.index_step(id, batch).unwrap() {
            IndexStep::Finished | IndexStep::Idle => return,
            IndexStep::Advanced(_) => {}
        }
    }
    panic!("build did not finish");
}

#[test]
fn building_index_covers_existing_records_and_tracks_edits() {
    let eng = engine();
    let mut records = Vec::new();
    for i in 0..25 {
        records.push(
            eng.create_record(
                "Item",
                None,
                vec![("price".into(), FieldChange::Set(json!(i % 7)))],
            )
            .unwrap(),
        );
    }
    eng.create_record(
        "Other",
        None,
        vec![("price".into(), FieldChange::Set(json!(1)))],
    )
    .unwrap();
    let def = eng
        .define_index("item_price", "Item", "price", IndexKind::Ordered)
        .unwrap();
    assert_eq!(def.state, IndexState::Building { cursor: 0 });
    assert_oracle(&eng, "defined");
    // Step in small batches, editing covered and uncovered records between
    // steps: the oracle holds after every commit.
    let mut steps = 0;
    loop {
        let step = eng.index_step(def.id, 4).unwrap();
        steps += 1;
        let r = records[steps * 3 % records.len()];
        eng.update_fields(
            r,
            vec![("price".into(), FieldChange::Set(json!(100 + steps)))],
        )
        .unwrap();
        assert_oracle(&eng, &format!("step {steps}"));
        if step == IndexStep::Finished {
            break;
        }
    }
    let view = eng.store.read_view().unwrap();
    assert_eq!(
        index::by_name(&view, TENANT, "item_price")
            .unwrap()
            .unwrap()
            .state,
        IndexState::Ready
    );
    assert_eq!(
        entry_count(&eng, def.id),
        25,
        "one entry per Item, none for Other"
    );
    assert_eq!(
        index::ready_ordered(&view, TENANT, "Item", "price")
            .unwrap()
            .unwrap()
            .id,
        def.id
    );
}

#[test]
fn index_build_is_complete_under_concurrent_writes() {
    let eng = Arc::new(engine());
    let mut seed = Vec::new();
    for i in 0..400 {
        seed.push(
            eng.create_record("Item", None, vec![("n".into(), FieldChange::Set(json!(i)))])
                .unwrap(),
        );
    }
    let def = eng
        .define_index("item_n", "Item", "n", IndexKind::Ordered)
        .unwrap();
    let writers: Vec<_> = (0..8)
        .map(|w| {
            let eng = Arc::clone(&eng);
            let seed = seed.clone();
            std::thread::spawn(move || {
                for i in 0..50 {
                    let r = seed[(w * 50 + i) % seed.len()];
                    eng.update_fields(r, vec![("n".into(), FieldChange::Set(json!(1000 + i)))])
                        .expect("an edit never conflicts with the backfill");
                    eng.create_record("Item", None, vec![("n".into(), FieldChange::Set(json!(i)))])
                        .unwrap();
                }
            })
        })
        .collect();
    build(&eng, def.id, 16);
    for writer in writers {
        writer.join().unwrap();
    }
    assert_oracle(&eng, "after concurrent build");
    assert_eq!(entry_count(&eng, def.id), 400 + 8 * 50);
}

#[test]
fn drop_index_removes_entries_and_definition() {
    let eng = engine();
    for i in 0..30 {
        eng.create_record("Item", None, vec![("n".into(), FieldChange::Set(json!(i)))])
            .unwrap();
    }
    let def = eng
        .define_index("item_n", "Item", "n", IndexKind::Ordered)
        .unwrap();
    build(&eng, def.id, 1_000);
    assert_eq!(entry_count(&eng, def.id), 30);
    eng.drop_index("item_n").unwrap();
    assert_oracle(&eng, "dropping");
    // Records created during the drop are not covered past the cursor.
    let mut steps = 0;
    while eng.index_step(def.id, 7).unwrap() != IndexStep::Finished {
        steps += 1;
        eng.create_record(
            "Item",
            None,
            vec![("n".into(), FieldChange::Set(json!(99)))],
        )
        .unwrap();
        assert_oracle(&eng, "drop step");
    }
    assert!(steps >= 4);
    let view = eng.store.read_view().unwrap();
    assert!(index::by_name(&view, TENANT, "item_n").unwrap().is_none());
    assert!(index::definitions(&view, TENANT).unwrap().is_empty());
    assert_eq!(entry_count(&eng, def.id), 0);
    assert_oracle(&eng, "dropped");
}

#[test]
fn entries_are_ordered_by_value_and_hold_every_array_element() {
    let eng = engine();
    let a = eng
        .create_record(
            "Item",
            None,
            vec![("tags".into(), FieldChange::Set(json!(["b", "a"])))],
        )
        .unwrap();
    let b = eng
        .create_record(
            "Item",
            None,
            vec![("tags".into(), FieldChange::Set(json!("a")))],
        )
        .unwrap();
    let def = eng
        .define_index("item_tags", "Item", "tags", IndexKind::Ordered)
        .unwrap();
    build(&eng, def.id, 10);
    let view = eng.store.read_view().unwrap();
    let rows = index::entries(&view, TENANT, def.id, &[], &[]).unwrap();
    let decoded: Vec<(String, u64)> = rows
        .iter()
        .map(|(entry, owner)| {
            let scalar = rushdb_types::encoding::decode_order_preserving(
                rushdb_types::encoding::logical_from_tag(entry[0]).unwrap(),
                &entry[1..],
            )
            .unwrap();
            let rushdb_types::value::Scalar::Utf8(text) = scalar else {
                panic!("utf8 entry")
            };
            (text, *owner)
        })
        .collect();
    let mut expected = vec![
        ("a".to_string(), a.ordinal),
        ("a".to_string(), b.ordinal),
        ("b".to_string(), a.ordinal),
    ];
    expected.sort();
    assert_eq!(decoded, expected);
}

#[test]
fn duplicate_names_and_unknown_drops_are_refused() {
    let eng = engine();
    eng.define_index("i", "Item", "n", IndexKind::Ordered)
        .unwrap();
    assert!(eng
        .define_index("i", "Item", "m", IndexKind::Ordered)
        .is_err());
    assert!(eng.drop_index("missing").is_err());
}

/// A structural index marks the records whose embedding a topology change
/// alters: the endpoints of a new relationship and their neighbours (an
/// embedding reads two hops), and no record further away.
#[test]
fn structural_index_marks_the_two_hop_neighbourhood_of_a_change() {
    let eng = engine();
    let chain: Vec<_> = (0..5)
        .map(|_| eng.create_record("Node", None, vec![]).unwrap())
        .collect();
    for pair in chain.windows(2) {
        eng.create_relationship("NEXT", pair[0], pair[1], vec![])
            .unwrap();
    }
    let def = eng
        .define_index(
            "shape",
            "Node",
            "*",
            IndexKind::Structural { dimensions: 16 },
        )
        .unwrap();
    while eng.index_step(def.id, 100).unwrap() != IndexStep::Idle {}
    let view = eng.store.read_view().unwrap();
    let all = index::pending(&view, TENANT, def.id, 100).unwrap();
    assert_eq!(all.len(), 5, "the build marks every member");
    eng.index_applied(def.id, view.commit_index(), all).unwrap();

    let extra = eng.create_record("Node", None, vec![]).unwrap();
    eng.create_relationship("NEXT", chain[2], extra, vec![])
        .unwrap();
    let view = eng.store.read_view().unwrap();
    let mut marked = index::pending(&view, TENANT, def.id, 100).unwrap();
    marked.sort_unstable();
    let mut expected = vec![
        chain[1].ordinal,
        chain[2].ordinal,
        chain[3].ordinal,
        extra.ordinal,
    ];
    expected.sort_unstable();
    assert_eq!(marked, expected);

    // A structural index reads topology: it names no property.
    assert!(eng
        .define_index(
            "bad",
            "Node",
            "name",
            IndexKind::Structural { dimensions: 16 }
        )
        .is_err());
    assert!(eng
        .define_index("tiny", "Node", "*", IndexKind::Structural { dimensions: 2 })
        .is_err());
}
