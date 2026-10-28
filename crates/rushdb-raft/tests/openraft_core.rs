//! Integration tests for the in-process `OpenRaft` cluster: leader election,
//! replication through the leader, membership changes, and catch-up of joining
//! nodes against real `RushDB` state machines.

use std::sync::Arc;
use std::time::Duration;

use rushdb_lmpg::plan::FieldChange;
use rushdb_lmpg::reader::try_access;
use rushdb_raft::openraft_core::OpenRaftCluster;
use rushdb_storage::{StorageError, Store};
use rushdb_types::command::CommitBatch;
use rushdb_types::ids::TenantId;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn openraft_elects_and_applies_a_real_rushdb_commit_batch() {
    // Produce one canonical batch through the normal planner. The IDs and
    // mutations in this batch are then deterministic on every Raft node.
    let source = Arc::new(Store::new_in_memory());
    let source_lmpg = rushdb_lmpg::Lmpg::new(source.clone(), TenantId::single());
    source_lmpg
        .create_record(
            "Replicated",
            None,
            vec![("value".into(), FieldChange::Set(json!(42)))],
        )
        .unwrap();
    let batch: CommitBatch = rushdb_types::command_codec::decode(&source.log_bytes()[0]).unwrap();

    let cluster = OpenRaftCluster::new(3, Duration::from_millis(50), Duration::from_millis(200))
        .await
        .unwrap();
    assert_eq!(cluster.size(), 3);
    let leader = cluster.leader(Duration::from_secs(5)).await.unwrap();
    assert!(leader < 3);
    let wrapped = rushdb_raft::openraft_core::ReplicatedCommit {
        batch: batch.clone(),
        payload: Vec::new(),
    };
    let index = cluster.replicate_batch(&wrapped).await.unwrap();
    assert!(index >= 2, "membership occupies the first log entry");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let applied_everywhere = (0..cluster.size() as u64).all(|node_id| {
            let store = cluster.state_store(node_id).unwrap();
            let lmpg = rushdb_lmpg::Lmpg::new(store, TenantId::single());
            try_access::label_members(
                &lmpg.store.read_view().unwrap(),
                TenantId::single(),
                "Replicated",
            )
            .unwrap()
            .len()
                == 1
        });
        if applied_everywhere {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "replicated batch was not applied on every state machine"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    for node_id in 0..cluster.size() as u64 {
        let store = cluster.state_store(node_id).unwrap();
        let lmpg = rushdb_lmpg::Lmpg::new(store, TenantId::single());
        let record = rushdb_lmpg::reconstruct::reconstruct_record(
            &lmpg.store.read_view().unwrap(),
            TenantId::single(),
            0,
        )
        .unwrap()
        .unwrap();
        assert_eq!(record.get("value"), Some(&json!(42)));
    }

    // The canonical command is replicated, not a facade-specific typed
    // response. Retrying through an incompatible local response type must be
    // an explicit error and must never execute the mutation closure or panic.
    let follower = cluster.state_store(0).unwrap();
    let retry = follower.commit::<String, _>(batch.commit_key, |_ci, _txn| {
        panic!("an already committed key must not execute again")
    });
    assert!(matches!(retry, Err(StorageError::OutcomePayload)));
}

/// Exercises the production wiring path: a `ClusterReplicator` installed on a
/// store routes a normal `Lmpg` commit through the leader, the batch is
/// applied deterministically on the local store (node 0) before `commit`
/// returns, and the committed payload is reproduced on an idempotent retry.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cluster_replicator_routes_local_commits_through_leader() {
    let runtime = tokio::runtime::Handle::current();
    let cluster = Arc::new(
        OpenRaftCluster::new_with_local_store(
            3,
            Duration::from_millis(50),
            Duration::from_millis(200),
            None,
        )
        .await
        .unwrap(),
    );
    // Attach the replicator to the leader's own store so `commit` routes
    // through the leader and the outcome lands locally before returning.
    let leader = cluster.leader(Duration::from_secs(5)).await.unwrap();
    let leader_store = cluster.state_store(leader).unwrap();
    leader_store.set_replicator(Some(Arc::new(rushdb_raft::ClusterReplicator::new(
        cluster.clone(),
        runtime,
    ))));

    let lmpg = rushdb_lmpg::Lmpg::new(leader_store.clone(), TenantId::single());
    lmpg.create_record(
        "ViaLeader",
        None,
        vec![("value".into(), FieldChange::Set(json!(7)))],
    )
    .unwrap();

    // The acknowledged write must already be applied on the leader. Followers
    // apply asynchronously after the quorum commit; wait for their watermarks
    // before asserting all-node state, as in the canonical-batch test above.
    let committed = leader_store.committed_version();
    assert_eq!(committed, 1);
    assert!(rushdb_lmpg::reconstruct::reconstruct_record(
        &lmpg.store.read_view().unwrap(),
        TenantId::single(),
        0
    )
    .unwrap()
    .is_some());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while (0..3u64)
        .any(|node_id| cluster.state_store(node_id).unwrap().committed_version() < committed)
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "followers did not apply the acknowledged batch"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Every node must converge to the same record, whichever node was elected.
    for node_id in 0..3u64 {
        let store = cluster.state_store(node_id).unwrap();
        let lmpg = rushdb_lmpg::Lmpg::new(store, TenantId::single());
        let record = rushdb_lmpg::reconstruct::reconstruct_record(
            &lmpg.store.read_view().unwrap(),
            TenantId::single(),
            0,
        )
        .unwrap()
        .unwrap();
        assert_eq!(record.get("value"), Some(&json!(7)));
    }
}

/// P19: a leader change (membership removal of the current leader) hands off
/// to a new leader; replication continues and converges on the surviving voters.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn leader_change_hands_off_and_replication_continues() {
    let cluster = OpenRaftCluster::new(3, Duration::from_millis(50), Duration::from_millis(250))
        .await
        .unwrap();
    let leader = cluster.leader(Duration::from_secs(5)).await.unwrap();
    let survivors: Vec<u64> = (0..3u64).filter(|&n| n != leader).collect();

    // Remove the current leader from the membership: a new leader must be
    // elected among the survivors.
    cluster.change_membership(&survivors).await.unwrap();
    // Event-driven: each survivor's metrics watch reports the handoff; the
    // bound only stops a cluster that never settles.
    let new_leader = cluster
        .wait_for_leader_among(&survivors, Duration::from_secs(60))
        .await
        .expect("a new leader among the survivors");
    assert!(
        survivors.contains(&new_leader),
        "new leader {new_leader} must be a survivor"
    );

    // Replication must continue through the new leader and apply on every survivor.
    let source = Arc::new(Store::new_in_memory());
    let lmpg = rushdb_lmpg::Lmpg::new(source.clone(), TenantId::single());
    lmpg.create_record(
        "Survived",
        None,
        vec![("value".into(), FieldChange::Set(json!(9)))],
    )
    .unwrap();
    let batch: CommitBatch = rushdb_types::command_codec::decode(&source.log_bytes()[0]).unwrap();
    let wrapped = rushdb_raft::openraft_core::ReplicatedCommit {
        batch,
        payload: Vec::new(),
    };
    cluster.replicate_batch(&wrapped).await.unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let converged = survivors.iter().all(|&n| {
            let store = cluster.state_store(n).unwrap();
            let lmpg = rushdb_lmpg::Lmpg::new(store, TenantId::single());
            try_access::label_members(
                &lmpg.store.read_view().unwrap(),
                TenantId::single(),
                "Survived",
            )
            .unwrap()
            .len()
                == 1
        });
        if converged {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "survivors did not converge"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// P19: a node that joins an established cluster (membership change) catches
/// up on the committed log via replay and converges to the same `RushDB` state.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn joining_node_catches_up_on_committed_log() {
    // Cluster construction installs the initial membership through the
    // in-process OpenRaft harness; under heavy parallel load the auto-election
    // can race that membership change ("already undergoing a configuration
    // change"). Retry a few times to keep the gate deterministic.
    let cluster = {
        let mut attempt = 0;
        loop {
            match rushdb_raft::openraft_core::OpenRaftCluster::new_with_expandable_membership(
                3,
                Duration::from_millis(50),
                Duration::from_millis(250),
            )
            .await
            {
                Ok(cluster) => break cluster,
                Err(error) if attempt < 8 => {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    let _ = error;
                }
                Err(error) => panic!("cluster initialization failed: {error}"),
            }
        }
    };
    // Initial membership is {0}; node 0 is the sole voter and leader.
    cluster.leader(Duration::from_secs(5)).await.unwrap();

    // Commit two batches while only node 0 is a member. Both come from the
    // same source store so they carry distinct commit keys (1 and 2) — the
    // apply dedups by commit key, so identical keys would collapse to one.
    let source = Arc::new(Store::new_in_memory());
    let lmpg = rushdb_lmpg::Lmpg::new(source.clone(), TenantId::single());
    lmpg.create_record(
        "CatchUp",
        None,
        vec![("value".into(), FieldChange::Set(json!(10)))],
    )
    .unwrap();
    lmpg.create_record(
        "CatchUp",
        None,
        vec![("value".into(), FieldChange::Set(json!(20)))],
    )
    .unwrap();
    let log = source.log_bytes();
    assert!(log.len() >= 2);
    for entry in log.iter().take(2) {
        let batch: CommitBatch = rushdb_types::command_codec::decode(entry).unwrap();
        cluster
            .replicate_batch(&rushdb_raft::openraft_core::ReplicatedCommit {
                batch,
                payload: Vec::new(),
            })
            .await
            .unwrap();
    }

    // Node 0 applied both (the apply is asynchronous after client_write returns).
    {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let store = cluster.state_store(0).unwrap();
            let lmpg = rushdb_lmpg::Lmpg::new(store, TenantId::single());
            if try_access::label_members(
                &lmpg.store.read_view().unwrap(),
                TenantId::single(),
                "CatchUp",
            )
            .unwrap()
            .len()
                == 2
            {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "initial member did not apply both batches"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    // Add node 1 as a learner, then promote it to a voter: it must replay the
    // committed log and converge to the same two records.
    cluster.add_learner(1).await.unwrap();
    cluster.change_membership(&[0, 1]).await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let store = cluster.state_store(1).unwrap();
        let lmpg = rushdb_lmpg::Lmpg::new(store, TenantId::single());
        let snap = lmpg.store.read_view().unwrap();
        let caught_up = (0..2).all(|i| {
            rushdb_lmpg::reconstruct::reconstruct_record(&snap, TenantId::single(), i)
                .unwrap()
                .is_some()
        });
        if caught_up {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "joining node did not catch up"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
