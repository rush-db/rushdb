//! `RushDB` consensus integration.
//!
//! Election, membership, log replication, quorum acknowledgement, and
//! snapshot protocol semantics are delegated exclusively to `OpenRaft`.
//! `RushDB` supplies command/state-machine and network adapters only.

pub mod openraft_core;

pub use openraft_core::{
    ClusterReplicator, InProcessNetworkFactory, NodeId, OpenRaft, OpenRaftCluster, ReplicatedCommit,
};
