//! An unindexed leaf's owner walk in fixed-size parts: the owner set, the
//! owner-budget error and cancellation do not depend on the threads lent.

use std::cell::Cell;
use std::sync::{Arc, Mutex};

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::{BatchOp, Lmpg};
use serde_json::{json, Value};

use super::leaf_walk::WALK_PART_OWNERS;
use super::{leaf_owners, LeafScope, OwnerSet};
use crate::{Expr, FieldPred, QueryError, QuerySnapshot};

/// Tests that set the process-wide thread budget run one at a time.
static THREADS: Mutex<()> = Mutex::new(());

/// Scalar records: more than two parts of owners.
const TRIPS: u64 = 10_000;
/// Records holding `payment` as an array, under their own label.
const ARRAYS: u64 = 300;

const PAYMENTS: [&str; 4] = ["card", "cash", "other", "none"];

/// One fixture record: its ordinal and the values the leaves read.
struct Row {
    ordinal: u64,
    label: &'static str,
    payments: Vec<&'static str>,
    passengers: u64,
}

struct Fixture {
    snap: QuerySnapshot,
    tenant: rushdb_types::ids::TenantId,
    /// The live records.
    rows: Vec<Row>,
}

fn fixture() -> Fixture {
    let lmpg = Lmpg::new(
        Arc::new(rushdb_storage::Store::new_in_memory()),
        rushdb_types::ids::TenantId::from_ordinal(1),
    );
    let set = |name: &str, value: Value| (name.to_owned(), FieldChange::Set(value));
    let mut rows = Vec::new();
    let mut batch = lmpg.begin_batch();
    let mut specs = Vec::new();
    for i in 0..TRIPS + ARRAYS {
        let (label, payments) = if i < TRIPS {
            ("Trip", vec![PAYMENTS[usize::try_from(i % 4).unwrap()]])
        } else if i % 2 == 0 {
            ("Fleet", vec!["cash", "card"])
        } else {
            ("Fleet", vec!["other"])
        };
        let payment = if label == "Trip" {
            json!(payments[0])
        } else {
            json!(payments)
        };
        batch.ops.push(BatchOp::CreateRecord {
            label: label.into(),
            explicit_id: None,
            fields: vec![set("payment", payment), set("passengers", json!(i % 6))],
        });
        specs.push((label, payments, i % 6));
    }
    let created = lmpg.run_batch(batch).unwrap().created_records;
    // Every seventh record is deleted: its presence and occurrence rows go
    // with it.
    for (i, (record, (label, payments, passengers))) in created.into_iter().zip(specs).enumerate() {
        if i % 7 == 3 {
            lmpg.delete_record(record).unwrap();
        } else {
            rows.push(Row {
                ordinal: record.ordinal,
                label,
                payments,
                passengers,
            });
        }
    }
    Fixture {
        snap: Arc::new(lmpg.store.read_view().unwrap()),
        tenant: lmpg.tenant,
        rows,
    }
}

impl Fixture {
    /// The owners `expr` selects under `labels`, by evaluating the fixture.
    fn expected(&self, expr: &Expr, labels: &[String]) -> OwnerSet {
        self.rows
            .iter()
            .filter(|row| labels.is_empty() || labels.iter().any(|l| l == row.label))
            .filter(|row| match expr {
                Expr::Field(FieldPred::Eq(_, value)) => {
                    row.payments.iter().any(|p| json!(p) == *value)
                }
                Expr::Field(FieldPred::In(_, values)) => {
                    row.payments.iter().any(|p| values.contains(&json!(p)))
                }
                Expr::Field(FieldPred::Gte(_, bound)) => {
                    f64::from(u32::try_from(row.passengers).unwrap()) >= *bound
                }
                other => panic!("no oracle for {other:?}"),
            })
            .map(|row| row.ordinal)
            .collect()
    }

    /// Every second live record under `labels` (earlier leaves are scoped
    /// to them): fewer than a leaf's population, more than one part when
    /// unscoped.
    fn candidates(&self, labels: &[String]) -> OwnerSet {
        self.rows
            .iter()
            .filter(|row| labels.is_empty() || labels.iter().any(|l| l == row.label))
            .step_by(2)
            .map(|row| row.ordinal)
            .collect()
    }

    fn walk(
        &self,
        threads: usize,
        expr: &Expr,
        labels: &[String],
        candidates: Option<&OwnerSet>,
        owner_limit: usize,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<OwnerSet, QueryError> {
        rushdb_storage::parallel::set_threads(threads);
        let scope = LeafScope {
            labels,
            owner_limit,
            candidates,
            is_cancelled,
        };
        let out = leaf_owners(&self.snap, self.tenant, expr, &scope);
        rushdb_storage::parallel::set_threads(1);
        Ok(out?.expect("the leaf is materialized"))
    }
}

fn leaves() -> Vec<Expr> {
    vec![
        Expr::Field(FieldPred::Eq("payment".into(), json!("card"))),
        Expr::Field(FieldPred::In(
            "payment".into(),
            vec![json!("card"), json!("other")],
        )),
        Expr::Field(FieldPred::Gte("passengers".into(), 2.0)),
    ]
}

#[test]
fn parted_walk_is_the_sequential_owner_set_at_any_thread_count() {
    let _threads = THREADS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let fx = fixture();
    assert!(fx.candidates(&[]).len() > u64::try_from(WALK_PART_OWNERS).unwrap());
    let never = || false;
    for labels in [Vec::new(), vec!["Trip".to_string()], vec!["Fleet".into()]] {
        let candidates = fx.candidates(&labels);
        for leaf in leaves() {
            let expected = fx.expected(&leaf, &labels);
            assert!(!expected.is_empty());
            for scope in [None, Some(&candidates)] {
                let expected = scope.map_or_else(|| expected.clone(), |c| &expected & c);
                for threads in [1, 8] {
                    let owners = fx
                        .walk(threads, &leaf, &labels, scope, usize::MAX, &never)
                        .unwrap();
                    assert_eq!(
                        owners,
                        expected,
                        "{leaf:?} under {labels:?}, candidates: {}, {threads} threads",
                        scope.is_some()
                    );
                }
            }
        }
    }
}

#[test]
fn owner_budget_fails_the_same_at_any_thread_count() {
    let _threads = THREADS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let fx = fixture();
    let candidates = fx.candidates(&[]);
    let never = || false;
    let leaf = &leaves()[0];
    for scope in [None, Some(&candidates)] {
        let matched = fx
            .walk(1, leaf, &[], scope, usize::MAX, &never)
            .unwrap()
            .len();
        let limit = usize::try_from(matched).unwrap();
        let mut errors = Vec::new();
        for threads in [1, 8] {
            // Exactly the matches fit; one fewer does not.
            let fits = fx.walk(threads, leaf, &[], scope, limit, &never).unwrap();
            assert_eq!(fits.len(), matched);
            let Err(error) = fx.walk(threads, leaf, &[], scope, limit - 1, &never) else {
                panic!("{threads} threads admitted more owners than the budget");
            };
            assert!(matches!(error, QueryError::ResourceLimit(_)), "{error}");
            errors.push(error.to_string());
        }
        assert_eq!(errors[0], errors[1]);
    }
}

#[test]
fn cancellation_stops_the_walk_between_rounds() {
    let _threads = THREADS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let fx = fixture();
    let leaf = &leaves()[0];
    // On one thread each round is one part, and the walk polls before each:
    // the fixture's population is more than one part.
    let polls = Cell::new(0u32);
    let counting = || {
        polls.set(polls.get() + 1);
        false
    };
    fx.walk(1, leaf, &[], None, usize::MAX, &counting).unwrap();
    assert!(polls.get() > 1, "one part held the whole population");
    // Cancelled after the first round (one thread), or before any (eight
    // threads run every part in one round).
    for (threads, rounds) in [(1, 1), (8, 0)] {
        let polls = Cell::new(0u32);
        let cancelled = || {
            polls.set(polls.get() + 1);
            polls.get() > rounds
        };
        let result = fx.walk(threads, leaf, &[], None, usize::MAX, &cancelled);
        assert!(
            matches!(result, Err(QueryError::Cancelled)),
            "{threads} threads"
        );
        assert_eq!(polls.get(), rounds + 1);
    }
}
