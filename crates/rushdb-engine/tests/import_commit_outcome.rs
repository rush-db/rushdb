//! A known committed import must remain successful when its acknowledgement is late.

use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use rushdb_engine::{Engine, OperationClass};
use rushdb_query::SearchRequest;
use rushdb_storage::{Replicator, Store};
use rushdb_types::command::CommitBatch;
use serde_json::json;

struct DelayedAcknowledgement {
    store: Weak<Store>,
    deadline: Instant,
}

impl Replicator for DelayedAcknowledgement {
    fn replicate(&self, batch: &CommitBatch, payload: &[u8]) -> Result<(), String> {
        let store = self.store.upgrade().ok_or("store closed")?;
        store
            .apply_replicated_batch_with_payload(batch, Some(payload))
            .map_err(|error| error.to_string())?;
        // Publication has succeeded. Delay only its acknowledgement, so this
        // checks the committed outcome rather than racing planning cancellation.
        std::thread::sleep(
            self.deadline.saturating_duration_since(Instant::now()) + Duration::from_millis(1),
        );
        Ok(())
    }
}

#[test]
fn committed_import_succeeds_when_acknowledgement_passes_deadline() {
    let engine = Engine::new_in_memory();
    let mut context = engine
        .context(
            None,
            OperationClass::Write,
            Some(Duration::from_secs(2)),
            None,
        )
        .unwrap();
    engine
        .store
        .set_replicator(Some(Arc::new(DelayedAcknowledgement {
            store: Arc::downgrade(&engine.store),
            deadline: context.deadline().unwrap(),
        })));

    let result = engine.import_document_with_context(&mut context, "Audit", &json!({"value": 1}));
    assert!(context.check_deadline().is_err());
    result.expect("known committed import must not be reported as a conflict");
    let page = engine
        .search(
            None,
            &SearchRequest {
                labels: vec!["Audit".into()],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(page.total_matched, 1);
    assert_eq!(engine.store.committed_version(), 1);
    assert_eq!(page.records[0]["value"], json!(1));
}
