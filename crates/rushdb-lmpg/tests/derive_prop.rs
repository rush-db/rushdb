//! DERIVE-PROP: derived rows are exact on every commit. Random write
//! sequences over two tenants (creates, field writes of every value shape,
//! retypes, deletes, relationships and their deletes, multi-op batches) are
//! checked after every commit against the whole tenant derived from scratch
//! (the scrubber): one disagreement fails with the seed, the step and the
//! ops that led to it.

use std::sync::Arc;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{BatchOp, BatchTx, Lmpg, RecordRef, RelationshipRef};
use rushdb_storage::Store;
use rushdb_types::ids::TenantId;
use serde_json::{json, Value};

/// A small deterministic generator (xorshift64*), so a failure replays from
/// its seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap_or(0)
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

const LABELS: [&str; 3] = ["A", "B", "C"];
const FIELDS: [&str; 4] = ["n", "s", "mixed", "list"];
const TYPES: [&str; 2] = ["KNOWS", "OWNS"];

/// Every value shape a property can hold, including a string longer than
/// the dictionary keeps inline.
fn value(rng: &mut Rng) -> FieldChange {
    let n = i64::try_from(rng.below(7)).unwrap_or(0);
    FieldChange::Set(match rng.below(9) {
        0 => json!(n),
        1 => json!(f64::from(u32::try_from(n).unwrap_or(0)) + 0.5),
        2 => json!(format!("v{n}")),
        3 => json!("x".repeat(100 + usize::try_from(n).unwrap_or(0))),
        4 => json!(n % 2 == 0),
        5 => Value::Null,
        6 => json!([n, n + 1]),
        7 => json!([format!("a{n}"), Value::Null]),
        _ => json!([]),
    })
}

fn fields(rng: &mut Rng) -> Vec<(String, FieldChange)> {
    let mut fields: Vec<(String, FieldChange)> = (0..=rng.below(3))
        .map(|_| ((*rng.pick(&FIELDS)).to_owned(), value(rng)))
        .collect();
    fields.sort_by(|a, b| a.0.cmp(&b.0));
    fields.dedup_by(|a, b| a.0 == b.0);
    fields
}

/// One tenant's live records (with labels) and relationships.
struct World {
    lmpg: Lmpg,
    records: Vec<(RecordRef, &'static str)>,
    relationships: Vec<RelationshipRef>,
}

impl World {
    /// One random step; a description of it for the failure message.
    fn step(&mut self, rng: &mut Rng) -> String {
        let choice = if self.records.len() < 2 {
            0
        } else {
            rng.below(8)
        };
        match choice {
            0 | 1 => {
                let label = *rng.pick(&LABELS);
                let fields = fields(rng);
                let described = format!("create {label} {fields:?}");
                let record = self.lmpg.create_record(label, None, fields).unwrap();
                self.records.push((record, label));
                described
            }
            2 | 3 => {
                let (record, _) = *rng.pick(&self.records);
                let fields = fields(rng);
                let described = format!("update {} {fields:?}", record.ordinal);
                self.lmpg.update_fields(record, fields).unwrap();
                described
            }
            4 => {
                let index = rng.below(self.records.len());
                let (record, from) = self.records[index];
                let to = *rng.pick(&LABELS);
                if to == from {
                    return "retype to the same label (skipped)".into();
                }
                self.lmpg.retype_record(record, from, to).unwrap();
                self.records[index].1 = to;
                format!("retype {} {from} -> {to}", record.ordinal)
            }
            5 => {
                let index = rng.below(self.records.len());
                let (record, _) = self.records.swap_remove(index);
                self.lmpg.delete_record(record).unwrap();
                // Its relationships went with it.
                let view = self.lmpg.store.read_view().unwrap();
                self.relationships.retain(|rel| {
                    rushdb_lmpg::reader::try_access::relationship_by_ord(
                        &view,
                        self.lmpg.tenant,
                        rel.ordinal,
                    )
                    .unwrap()
                    .is_some()
                });
                format!("delete {}", record.ordinal)
            }
            6 => {
                let (source, _) = *rng.pick(&self.records);
                let (target, _) = *rng.pick(&self.records);
                let rel_type = *rng.pick(&TYPES);
                let fields = if rng.below(2) == 0 {
                    fields(rng)
                } else {
                    Vec::new()
                };
                let described = format!("link {rel_type} {fields:?}");
                let rel = self
                    .lmpg
                    .create_relationship(rel_type, source, target, fields)
                    .unwrap();
                self.relationships.push(rel);
                described
            }
            _ => self.batch(rng),
        }
    }

    /// Several creates, an update of an existing record and, when there is
    /// one, a relationship delete, in one batch.
    fn batch(&mut self, rng: &mut Rng) -> String {
        // A batch: several creates and an update of an existing record.
        let mut ops: Vec<BatchOp> = (0..=rng.below(3))
            .map(|_| BatchOp::CreateRecord {
                label: (*rng.pick(&LABELS)).to_owned(),
                explicit_id: None,
                fields: fields(rng),
            })
            .collect();
        let (record, _) = *rng.pick(&self.records);
        ops.push(BatchOp::UpdateFields {
            owner: rushdb_types::ids::OwnerKind::Record,
            owner_ord: record.ordinal,
            owner_public_id: record.record_id,
            fields: fields(rng),
        });
        if let Some(index) =
            (!self.relationships.is_empty()).then(|| rng.below(self.relationships.len()))
        {
            ops.push(BatchOp::DeleteRelationship {
                rel: self.relationships.swap_remove(index),
            });
        }
        let labels: Vec<String> = ops
            .iter()
            .filter_map(|op| match op {
                BatchOp::CreateRecord { label, .. } => Some(label.clone()),
                _ => None,
            })
            .collect();
        let described = format!("batch of {} ops", ops.len());
        let created = self
            .lmpg
            .run_batch(BatchTx { ops })
            .unwrap()
            .created_records;
        for (record, label) in created.into_iter().zip(labels) {
            let label = LABELS.iter().copied().find(|l| *l == label).unwrap();
            self.records.push((record, label));
        }
        described
    }
}

fn violations(store: &Store, tenant: TenantId) -> Vec<String> {
    let view = store.read_view().unwrap();
    rushdb_lmpg::scrub::scrub_report(&view, tenant)
        .unwrap()
        .violations
        .into_iter()
        .map(|v| format!("{}: {}", v.what, v.detail))
        .collect()
}

#[test]
fn derived_rows_are_exact_after_every_random_commit() {
    for seed in [0x5EED_0001_u64, 0x5EED_0002, 0x5EED_0003] {
        let store = Arc::new(Store::new_in_memory());
        let mut rng = Rng(seed);
        let mut worlds: Vec<World> = [61, 62]
            .into_iter()
            .map(|ordinal| World {
                lmpg: Lmpg::new(Arc::clone(&store), TenantId::from_ordinal(ordinal)),
                records: Vec::new(),
                relationships: Vec::new(),
            })
            .collect();
        let mut history: Vec<String> = Vec::new();
        for step in 0..150 {
            let world = rng.below(worlds.len());
            let described = worlds[world].step(&mut rng);
            history.push(format!("{step}: tenant {world}: {described}"));
            for world in &worlds {
                let found = violations(&store, world.lmpg.tenant);
                assert!(
                    found.is_empty(),
                    "seed {seed:#x}, after:\n{}\nviolations: {found:?}",
                    history[history.len().saturating_sub(5)..].join("\n")
                );
            }
        }
    }
}
