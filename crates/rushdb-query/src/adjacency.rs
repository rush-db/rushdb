//! Adjacency of one relationship type for traversals: read from the store's
//! `adj` rows, or from a resident graph (a layer of
//! `rushdb_storage::resident`, which decides when one is worth building and
//! drops it when a commit writes the tenant's adjacency).
//!
//! Every relationship has one outgoing and one incoming row,
//! `T/adj/{out|in}/<node:8>/<type>/<other:8>/<rel:8>`, keyed by record: a
//! record's neighbours are one range scan, and a whole type is a scan of
//! every outgoing row of the tenant.

use std::sync::Arc;

use rushdb_storage::{ReadView, ScanBounds, ScanProjection};
use rushdb_types::ids::TenantId;

use crate::{Direction, QueryError, QuerySnapshot};

/// Neighbours of a record along one relationship shape.
pub(crate) trait Adjacency {
    /// Append `(neighbour, relationship ordinal)` of `node` to `out`:
    /// along the shape's direction when `forward`, against it otherwise.
    fn neighbours(
        &self,
        node: u64,
        forward: bool,
        out: &mut Vec<(u64, u64)>,
    ) -> Result<(), QueryError>;

    /// Whether a trail may go back over the same relationship in the other
    /// direction (`both`): the relationship is then one undirected edge.
    fn undirected(&self) -> bool;
}

/// Which row directions a step along `direction` (or against it) reads.
fn row_directions(direction: Direction, forward: bool) -> &'static [bool] {
    // `true` = outgoing rows.
    match (direction, forward) {
        (Direction::Outgoing, true) | (Direction::Incoming, false) => &[true],
        (Direction::Incoming, true) | (Direction::Outgoing, false) => &[false],
        (Direction::Both, _) => &[true, false],
    }
}

/// Append one record's rows of both directions, dropping the incoming copy
/// of a self-loop (one relationship, listed under both).
fn both_directions(
    node: u64,
    out: &mut Vec<(u64, u64)>,
    mut read: impl FnMut(bool, &mut Vec<(u64, u64)>) -> Result<(), QueryError>,
) -> Result<(), QueryError> {
    read(true, out)?;
    let before = out.len();
    read(false, out)?;
    let mut at = before;
    while at < out.len() {
        let (other, rel) = out[at];
        if other == node && out[..before].contains(&(node, rel)) {
            out.remove(at);
        } else {
            at += 1;
        }
    }
    Ok(())
}

/// Adjacency of one relationship type (every type when `None`) at one
/// snapshot, resident when the graph cache holds or buys it.
pub(crate) enum TypeAdjacency {
    Resident {
        graph: Arc<ResidentGraph>,
        direction: Direction,
    },
    Store(StoreAdjacency),
}

impl TypeAdjacency {
    /// The adjacency of `rel_type` for a traversal along `direction`: the
    /// resident graph when the layer holds one valid at the snapshot;
    /// otherwise the store, whose reads are rented toward a background
    /// build.
    pub(crate) fn open(
        snap: &QuerySnapshot,
        tenant: TenantId,
        rel_type: Option<&str>,
        direction: Direction,
    ) -> Self {
        let key = GraphKey::new(tenant, rel_type);
        if let Some(graph) = snap.residents().layer::<Graphs>().get(&key, snap) {
            return Self::Resident { graph, direction };
        }
        Self::Store(StoreAdjacency {
            snap: Arc::clone(snap),
            tenant,
            rel_type: rel_type.map(str::to_owned),
            direction,
            rows: std::cell::Cell::new(0),
        })
    }
}

impl Adjacency for TypeAdjacency {
    fn neighbours(
        &self,
        node: u64,
        forward: bool,
        out: &mut Vec<(u64, u64)>,
    ) -> Result<(), QueryError> {
        match self {
            Self::Resident { graph, direction } => {
                let read = |outgoing: bool, out: &mut Vec<(u64, u64)>| {
                    out.extend_from_slice(graph.edges(node, outgoing));
                    Ok(())
                };
                if *direction == Direction::Both {
                    both_directions(node, out, read)
                } else {
                    read(row_directions(*direction, forward)[0], out)
                }
            }
            Self::Store(store) => store.neighbours(node, forward, out),
        }
    }

    fn undirected(&self) -> bool {
        match self {
            Self::Resident { direction, .. } => *direction == Direction::Both,
            Self::Store(store) => store.direction == Direction::Both,
        }
    }
}

/// Rows of every relationship of the tenant: what one build reads.
fn build_rows(snap: &ReadView, tenant: TenantId) -> Result<u64, QueryError> {
    Ok(rushdb_lmpg::derived::tenant_usage(snap, tenant)?.relationships)
}

/// One relationship type (every type when `None`) of one tenant.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct GraphKey {
    tenant: TenantId,
    rel_type: Option<String>,
}

impl GraphKey {
    fn new(tenant: TenantId, rel_type: Option<&str>) -> Self {
        Self {
            tenant,
            rel_type: rel_type.map(str::to_owned),
        }
    }
}

/// Resident graphs, one per relationship type, valid until a commit writes
/// the tenant's adjacency (its rows are keyed by record, not by type).
#[derive(Default)]
pub(crate) struct Graphs;

impl rushdb_storage::resident::Resident for Graphs {
    type Key = GraphKey;
    type Value = ResidentGraph;
    const NAME: &'static str = "graph";
    const BUDGET: rushdb_storage::resident::Budget = rushdb_storage::resident::Budget::Graph;

    fn deps(
        &self,
        key: &GraphKey,
        _view: &ReadView,
    ) -> Result<Vec<rushdb_storage::resident::Dep>, rushdb_storage::StorageError> {
        Ok(vec![rushdb_storage::resident::Dep::Adjacency(key.tenant)])
    }

    fn build(
        &self,
        key: &GraphKey,
        view: &ReadView,
        cx: &rushdb_storage::resident::BuildCx,
    ) -> Result<Option<ResidentGraph>, rushdb_storage::resident::BuildError> {
        let refused = std::cell::Cell::new(false);
        let built = ResidentGraph::build(
            view,
            key.tenant,
            key.rel_type.as_deref(),
            &|| Ok(()),
            &|bytes| {
                let fits = cx.admit(bytes).is_ok();
                refused.set(!fits);
                fits
            },
        );
        match built {
            Ok(graph) => Ok(Some(graph)),
            Err(_) if refused.get() => Err(rushdb_storage::resident::BuildError::Refused),
            Err(QueryError::Storage(error)) => Err(error.into()),
            Err(other) => Err(rushdb_storage::StorageError::InvalidRead(other.to_string()).into()),
        }
    }

    fn bytes(graph: &ResidentGraph) -> usize {
        graph.outgoing.bytes() + graph.incoming.bytes()
    }
}

/// Adjacency read from the store's `adj` rows: one key-only range scan per
/// record and direction. The rows it reads are rented toward a resident
/// graph when it is dropped.
pub(crate) struct StoreAdjacency {
    snap: QuerySnapshot,
    tenant: TenantId,
    rel_type: Option<String>,
    direction: Direction,
    rows: std::cell::Cell<u64>,
}

impl StoreAdjacency {
    fn scan(&self, outgoing: bool, node: u64, out: &mut Vec<(u64, u64)>) -> Result<(), QueryError> {
        let dir: &[u8] = if outgoing { b"out" } else { b"in" };
        let prefix = match self.rel_type.as_deref() {
            Some(rel_type) => {
                rushdb_lmpg::reader::keys_ext::adj_prefix_typed(self.tenant, dir, node, rel_type)
            }
            None => rushdb_types::keys::key(self.tenant, &[b"adj", dir, &node.to_be_bytes()]),
        };
        let mut cursor = self
            .snap
            .scan(ScanBounds::prefix(&prefix), ScanProjection::Keys)?;
        // A range open costs about one row.
        let mut rows = 1u64;
        loop {
            let batch = cursor.next_batch(4_096, 1 << 20)?;
            for row in batch.rows {
                let (other, rel) = adjacency_tail(&row.key)?;
                out.push((other, rel));
                rows += 1;
            }
            if batch.done {
                break;
            }
        }
        self.rows.set(self.rows.get().saturating_add(rows));
        Ok(())
    }

    fn neighbours(
        &self,
        node: u64,
        forward: bool,
        out: &mut Vec<(u64, u64)>,
    ) -> Result<(), QueryError> {
        if self.direction == Direction::Both {
            return both_directions(node, out, |outgoing, out| self.scan(outgoing, node, out));
        }
        self.scan(row_directions(self.direction, forward)[0], node, out)
    }
}

impl Drop for StoreAdjacency {
    fn drop(&mut self) {
        let rows = self.rows.get();
        if rows == 0 {
            return;
        }
        if let Ok(build) = build_rows(&self.snap, self.tenant) {
            self.snap.residents().layer::<Graphs>().rent(
                &GraphKey::new(self.tenant, self.rel_type.as_deref()),
                &self.snap,
                rows,
                build,
            );
        }
    }
}

/// `(other, relationship ordinal)` from the `…/<other:8>/<rel:8>` tail of an
/// adjacency key.
fn adjacency_tail(key: &[u8]) -> Result<(u64, u64), QueryError> {
    let Some(tail) = key.len().checked_sub(17).map(|at| &key[at..]) else {
        return Err(rushdb_storage::StorageError::CorruptLog.into());
    };
    let other = u64::from_be_bytes(tail[..8].try_into().unwrap_or_default());
    let rel = u64::from_be_bytes(tail[9..17].try_into().unwrap_or_default());
    Ok((other, rel))
}

/// Every relationship of one type at one snapshot, in both directions,
/// sorted by record for lookup (compressed sparse rows).
#[derive(PartialEq)]
pub(crate) struct ResidentGraph {
    outgoing: Rows,
    incoming: Rows,
}

/// One direction: `edges[start[i]..start[i + 1]]` are the
/// `(neighbour, relationship)` pairs of `nodes[i]`.
#[derive(PartialEq)]
struct Rows {
    nodes: Vec<u64>,
    start: Vec<usize>,
    edges: Vec<(u64, u64)>,
}

impl Rows {
    /// From `(node, neighbour, relationship)` triples sorted by node.
    fn from_sorted(triples: &[(u64, u64, u64)]) -> Self {
        let mut rows = Rows {
            nodes: Vec::new(),
            start: Vec::new(),
            edges: Vec::with_capacity(triples.len()),
        };
        for &(node, other, rel) in triples {
            if rows.nodes.last() != Some(&node) {
                rows.nodes.push(node);
                rows.start.push(rows.edges.len());
            }
            rows.edges.push((other, rel));
        }
        rows.start.push(rows.edges.len());
        rows
    }

    fn of(&self, node: u64) -> &[(u64, u64)] {
        match self.nodes.binary_search(&node) {
            Ok(at) => &self.edges[self.start[at]..self.start[at + 1]],
            Err(_) => &[],
        }
    }

    fn bytes(&self) -> usize {
        self.nodes.len() * 8 + self.start.len() * 8 + self.edges.len() * 16
    }
}

impl ResidentGraph {
    /// The resident graph of `rel_type` (every type when `None`) at the
    /// view's snapshot: the resident one, or one built now from every
    /// outgoing adjacency row of the tenant and offered to the layer.
    pub(crate) fn keep(
        snap: &ReadView,
        tenant: TenantId,
        rel_type: Option<&str>,
        cancel: &dyn Fn() -> Result<(), QueryError>,
    ) -> Result<Arc<Self>, QueryError> {
        let key = GraphKey::new(tenant, rel_type);
        let layer = snap.residents().layer::<Graphs>();
        if let Some(graph) = layer.get(&key, snap) {
            return Ok(graph);
        }
        let graph = Arc::new(Self::build(snap, tenant, rel_type, cancel, &|_| true)?);
        layer.offer(key, snap, Arc::clone(&graph));
        Ok(graph)
    }

    /// Every relationship of `rel_type` at `snap`. `admit(bytes)` is asked
    /// as the rows grow; `false` stops the build (a resource-limit error).
    fn build(
        snap: &ReadView,
        tenant: TenantId,
        rel_type: Option<&str>,
        cancel: &dyn Fn() -> Result<(), QueryError>,
        admit: &dyn Fn(usize) -> bool,
    ) -> Result<Self, QueryError> {
        let prefix = rushdb_types::keys::key(tenant, &[b"adj", b"out", b""]);
        let mut cursor = snap.scan(ScanBounds::prefix(&prefix), ScanProjection::Keys)?;
        let mut triples: Vec<(u64, u64, u64)> = Vec::new();
        loop {
            cancel()?;
            let batch = cursor.next_batch(4_096, 1 << 20)?;
            for row in batch.rows {
                // `<source:8>/<type>/<target:8>/<rel:8>` after the prefix.
                let Some(rest) = row.key.strip_prefix(prefix.as_slice()) else {
                    continue;
                };
                if rest.len() < 8 + 1 + 1 + 18 {
                    return Err(rushdb_storage::StorageError::CorruptLog.into());
                }
                if rel_type.is_some_and(|wanted| wanted.as_bytes() != &rest[9..rest.len() - 18]) {
                    continue;
                }
                let source = u64::from_be_bytes(rest[..8].try_into().unwrap_or_default());
                let (target, rel) = adjacency_tail(rest)?;
                triples.push((source, target, rel));
            }
            // Both directions of every triple end up resident.
            if !admit(triples.len().saturating_mul(2 * 8 * 4)) {
                return Err(QueryError::ResourceLimit("graph exceeds its budget".into()));
            }
            if batch.done {
                break;
            }
        }
        triples.sort_unstable();
        let outgoing = Rows::from_sorted(&triples);
        for triple in &mut triples {
            *triple = (triple.1, triple.0, triple.2);
        }
        triples.sort_unstable();
        let incoming = Rows::from_sorted(&triples);
        Ok(Self { outgoing, incoming })
    }

    /// `(neighbour, relationship)` pairs of `node`'s outgoing (or incoming)
    /// relationships.
    pub(crate) fn edges(&self, node: u64, outgoing: bool) -> &[(u64, u64)] {
        if outgoing {
            self.outgoing.of(node)
        } else {
            self.incoming.of(node)
        }
    }

    /// Every record with an outgoing relationship in the graph, ascending.
    pub(crate) fn sources(&self) -> &[u64] {
        &self.outgoing.nodes
    }
}
