# Architecture

This section explains how RushDB works inside: how a request travels from a
wire protocol to durable storage, how data is laid out on disk, how snapshots
and transactions are isolated, and how the engine bounds its own memory and
disk use. It is written for operators who need to reason about behavior under
load, for contributors, and for anyone evaluating the design.

The pages describe the engine as it is today. Where a component has a known
limitation, the page states it as a user-facing limitation.

## Pages

Read them roughly in this order; later pages build on earlier ones.

1. [Overview](overview.md) — process model, crate map, data directory, and
   the life of a request from a transport to storage and back.
2. [Storage engine](storage-engine.md) — the log-first design: the segmented
   write-ahead log is the truth, redb is a checkpointed materialization.
   Crash recovery, checkpoints, retention, compaction, and retry receipts.
3. [Write path](write-path.md) — admission, planning and expansion, the
   sequencer overlay, commit groups, WAL append and fsync, apply-time
   derivation, publication, and writer fairness.
4. [MVCC and snapshots](mvcc-and-snapshots.md) — read views, snapshot
   pinning, version history, garbage collection, and the isolation contract.
5. [LMPG layout](lmpg-layout.md) — the Labeled Meta-Property Graph
   occurrence layout: key families, record and relationship headers, property
   families and definitions, the tenant prefix, and reconstruction.
6. [Derived state](derived-state.md) — what the engine derives from base
   rows on disk and in memory, how it is kept exact on every commit, and
   what is rebuilt after a restart.
7. [Query execution](query-execution.md) — the native query document,
   access-path planning, scans and declared indexes, pages and cursors,
   SQL through Apache DataFusion, graph languages through Grafeo, and
   execution budgets.
8. [Vector internals](vector-internals.md) — canonical vector storage, the
   hot/frozen segment ANN cache, merges, exact rerank, and the ANN budget.
9. [Text internals](text-internals.md) — Tantivy indexes per declared text
   index, the pending-row applier, and `eventual` vs `immediate`
   consistency.
10. [Multitenancy](multitenancy.md) — the tenant directory, the 5-byte
    keyspace prefix, fail-closed scope resolution, and isolation
    guarantees.
11. [Replication](replication.md) — streaming WAL replication, follower
    promotion, attaching a follower from a base backup, and the in-process
    Raft Preview.
12. [Resource governance](resource-governance.md) — the managed byte-budget
    tree, request and body budgets, the scheduler, admission and shedding,
    and the disk guard.

## See also

- [Getting started: concepts](../getting-started/concepts.md)
- [Operations](../operations/README.md)
- [Configuration reference](../reference/configuration-reference.md)
- [Glossary](../reference/glossary.md)
