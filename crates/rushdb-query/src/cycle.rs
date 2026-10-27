//! `$cycle`: does a record lie on a closed trail (a path back to itself that
//! uses no relationship twice) of `min..=max` hops over relationships of one
//! type and direction?
//!
//! Enumerating trails per record is exponential in the hop count. Instead,
//! one graph per query and relationship shape is built from the adjacency
//! rows and split into the parts a cycle can live in (strongly connected
//! components for a direction; for `both`, components minus bridges). Per
//! record, a breadth-first search finds its shortest cycle `s`, which is
//! always a simple cycle and so a trail: `s` in range answers `true`; no
//! cycle within `max` hops answers `false`. Only when `s < min` must a longer
//! trail be searched for, depth-first and within a step budget that fails
//! the query with a resource error instead of running unbounded.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use rushdb_storage::{ReadView, ScanBounds, ScanProjection};
use rushdb_types::ids::TenantId;

use crate::{Direction, QueryError, RelatedExpr};

/// Trail expansions a `$cycle` check may spend searching for a cycle longer
/// than the record's shortest one.
const TRAIL_SEARCH_BUDGET: usize = 100_000;

/// Graphs already built for one query, by relationship type and direction.
type GraphCache = HashMap<(Option<String>, u8), Arc<CycleGraph>>;

/// Graphs built for one query, keyed by relationship type and direction.
#[derive(Default)]
pub(crate) struct CycleGraphs(RefCell<GraphCache>);

impl CycleGraphs {
    /// Whether the record `ord` lies on a closed trail matching the `$cycle`
    /// block `rel` (its type, direction and hops).
    pub(crate) fn on_cycle(
        &self,
        snap: &ReadView,
        tenant: TenantId,
        rel: &RelatedExpr,
        ord: u64,
        cancel: &dyn Fn() -> Result<(), QueryError>,
    ) -> Result<bool, QueryError> {
        let key = (
            rel.rel_type.clone(),
            match rel.direction {
                Direction::Outgoing => 0,
                Direction::Incoming => 1,
                Direction::Both => 2,
            },
        );
        let cached = self.0.borrow().get(&key).cloned();
        let graph = if let Some(graph) = cached {
            graph
        } else {
            let graph = Arc::new(CycleGraph::build(
                snap,
                tenant,
                rel.rel_type.as_deref(),
                rel.direction,
                cancel,
            )?);
            self.0.borrow_mut().insert(key, Arc::clone(&graph));
            graph
        };
        let (min, max) = rel.hops.unwrap_or((1, 1));
        graph.on_cycle(ord, min, max, cancel)
    }
}

/// The relationships of one type, as traversed in one direction.
pub(crate) struct CycleGraph {
    index: HashMap<u64, u32>,
    /// Per node: `(neighbour, relationship ordinal)` in traversal direction.
    adj: Vec<Vec<(u32, u64)>>,
    /// Strongly connected component (directed) or connected component (`both`).
    component: Vec<u32>,
    /// Whether the node can lie on any cycle at all.
    capable: Vec<bool>,
    undirected: bool,
}

impl CycleGraph {
    fn build(
        snap: &ReadView,
        tenant: TenantId,
        rel_type: Option<&str>,
        direction: Direction,
        cancel: &dyn Fn() -> Result<(), QueryError>,
    ) -> Result<Self, QueryError> {
        let mut graph = CycleGraph {
            index: HashMap::new(),
            adj: Vec::new(),
            component: Vec::new(),
            capable: Vec::new(),
            undirected: direction == Direction::Both,
        };
        // Every relationship has one outgoing adjacency row:
        // `T/adj/out/<source:8>/<type>/<target:8>/<rel:8>`.
        let prefix = rushdb_types::keys::key(tenant, &[b"adj", b"out", b""]);
        let mut cursor = snap.scan(ScanBounds::prefix(&prefix), ScanProjection::Keys)?;
        loop {
            cancel()?;
            let batch = cursor.next_batch(4_096, 1 << 20)?;
            for row in batch.rows {
                let Some(rest) = row.key.strip_prefix(prefix.as_slice()) else {
                    continue;
                };
                if rest.len() < 8 + 1 + 1 + 18 {
                    continue;
                }
                let source = u64::from_be_bytes(rest[..8].try_into().unwrap_or_default());
                let tail = &rest[rest.len() - 17..];
                let name = &rest[9..rest.len() - 18];
                if rel_type.is_some_and(|wanted| wanted.as_bytes() != name) {
                    continue;
                }
                let target = u64::from_be_bytes(tail[..8].try_into().unwrap_or_default());
                let rel = u64::from_be_bytes(tail[9..17].try_into().unwrap_or_default());
                let (from, to) = match direction {
                    Direction::Outgoing | Direction::Both => (source, target),
                    Direction::Incoming => (target, source),
                };
                let (from, to) = (graph.node(from), graph.node(to));
                graph.adj[from as usize].push((to, rel));
                if graph.undirected && from != to {
                    graph.adj[to as usize].push((from, rel));
                }
            }
            if batch.done {
                break;
            }
        }
        if graph.undirected {
            graph.mark_undirected();
        } else {
            graph.mark_directed();
        }
        Ok(graph)
    }

    fn node(&mut self, ord: u64) -> u32 {
        let next = u32::try_from(self.adj.len()).unwrap_or(u32::MAX);
        let index = *self.index.entry(ord).or_insert(next);
        if index == next {
            self.adj.push(Vec::new());
        }
        index
    }

    /// Strongly connected components (iterative Tarjan); a node can be on a
    /// cycle when its component has two or more nodes or it has a self-loop.
    fn mark_directed(&mut self) {
        let n = self.adj.len();
        let unset = u32::MAX;
        let (mut index, mut low) = (vec![unset; n], vec![0u32; n]);
        let mut on_stack = vec![false; n];
        let (mut stack, mut component) = (Vec::new(), vec![unset; n]);
        let (mut next_index, mut next_component) = (0u32, 0u32);
        for root in 0..n {
            if index[root] != unset {
                continue;
            }
            let mut work: Vec<(usize, usize)> = vec![(root, 0)];
            index[root] = next_index;
            low[root] = next_index;
            next_index += 1;
            stack.push(root);
            on_stack[root] = true;
            while let Some(&mut (v, ref mut at)) = work.last_mut() {
                if let Some(&(w, _)) = self.adj[v].get(*at) {
                    *at += 1;
                    let w = w as usize;
                    if index[w] == unset {
                        index[w] = next_index;
                        low[w] = next_index;
                        next_index += 1;
                        stack.push(w);
                        on_stack[w] = true;
                        work.push((w, 0));
                    } else if on_stack[w] {
                        low[v] = low[v].min(index[w]);
                    }
                    continue;
                }
                work.pop();
                if let Some(&(parent, _)) = work.last() {
                    low[parent] = low[parent].min(low[v]);
                }
                if low[v] == index[v] {
                    while let Some(w) = stack.pop() {
                        on_stack[w] = false;
                        component[w] = next_component;
                        if w == v {
                            break;
                        }
                    }
                    next_component += 1;
                }
            }
        }
        let mut sizes: HashMap<u32, usize> = HashMap::new();
        for &c in &component {
            *sizes.entry(c).or_default() += 1;
        }
        self.capable = (0..n)
            .map(|v| sizes[&component[v]] > 1 || self.adj[v].iter().any(|&(w, _)| w as usize == v))
            .collect();
        self.component = component;
    }

    /// Connected components, and which nodes touch a non-bridge edge
    /// (iterative bridge-finding; a node is on a cycle only through one).
    fn mark_undirected(&mut self) {
        let n = self.adj.len();
        let unset = u32::MAX;
        let (mut index, mut low) = (vec![unset; n], vec![0u32; n]);
        let mut component = vec![unset; n];
        let mut bridges: HashSet<u64> = HashSet::new();
        let mut next_index = 0u32;
        for root in 0..n {
            if index[root] != unset {
                continue;
            }
            let root_component = u32::try_from(root).unwrap_or(u32::MAX);
            // (node, relationship it was entered by, next adjacency slot)
            let mut work: Vec<(usize, Option<u64>, usize)> = vec![(root, None, 0)];
            index[root] = next_index;
            low[root] = next_index;
            next_index += 1;
            component[root] = root_component;
            while let Some(&mut (v, entered_by, ref mut at)) = work.last_mut() {
                if let Some(&(w, rel)) = self.adj[v].get(*at) {
                    *at += 1;
                    let w = w as usize;
                    if Some(rel) == entered_by {
                        continue;
                    }
                    if index[w] == unset {
                        index[w] = next_index;
                        low[w] = next_index;
                        next_index += 1;
                        component[w] = root_component;
                        work.push((w, Some(rel), 0));
                    } else {
                        low[v] = low[v].min(index[w]);
                    }
                    continue;
                }
                work.pop();
                if let Some(&(parent, _, _)) = work.last() {
                    low[parent] = low[parent].min(low[v]);
                    if low[v] > index[parent] {
                        if let Some(rel) = entered_by {
                            bridges.insert(rel);
                        }
                    }
                }
            }
        }
        self.capable = (0..n)
            .map(|v| self.adj[v].iter().any(|(_, rel)| !bridges.contains(rel)))
            .collect();
        self.component = component;
    }

    fn on_cycle(
        &self,
        ord: u64,
        min: u64,
        max: u64,
        cancel: &dyn Fn() -> Result<(), QueryError>,
    ) -> Result<bool, QueryError> {
        let Some(&start) = self.index.get(&ord) else {
            return Ok(false);
        };
        if !self.capable[start as usize] {
            return Ok(false);
        }
        match self.shortest_cycle(start, max, cancel)? {
            None => Ok(false),
            Some(shortest) if shortest >= min => Ok(shortest <= max),
            Some(_) => self.longer_trail(start, min, max, cancel),
        }
    }

    /// Length of the shortest cycle through `start` of at most `max` hops.
    fn shortest_cycle(
        &self,
        start: u32,
        max: u64,
        cancel: &dyn Fn() -> Result<(), QueryError>,
    ) -> Result<Option<u64>, QueryError> {
        let home = self.component[start as usize];
        // node -> (distance, first relationship on the path from `start`)
        let mut seen: HashMap<u32, (u64, u64)> = HashMap::new();
        let mut queue = VecDeque::new();
        let mut best: Option<u64> = None;
        for &(w, rel) in &self.adj[start as usize] {
            if w == start {
                best = Some(1);
                continue;
            }
            if self.component[w as usize] == home && !seen.contains_key(&w) {
                seen.insert(w, (1, rel));
                queue.push_back(w);
            } else if self.undirected {
                // A second relationship to an already-reached neighbour.
                if let Some(&(_, first)) = seen.get(&w) {
                    if first != rel {
                        best = Some(best.map_or(2, |b| b.min(2)));
                    }
                }
            }
        }
        while let Some(v) = queue.pop_front() {
            cancel()?;
            let (distance, branch) = seen[&v];
            if best.is_some_and(|b| 2 * distance + 1 > b) || distance >= max {
                if !self.undirected {
                    break;
                }
                continue;
            }
            for &(w, rel) in &self.adj[v as usize] {
                if w == start {
                    // Directed: the first way back closes the shortest cycle.
                    // Undirected: not over the relationship we left by.
                    if !self.undirected || rel != branch {
                        let length = distance + 1;
                        if !self.undirected {
                            return Ok((length <= max).then_some(length));
                        }
                        best = Some(best.map_or(length, |b| b.min(length)));
                    }
                    continue;
                }
                if self.component[w as usize] != home {
                    continue;
                }
                match seen.get(&w) {
                    None => {
                        seen.insert(w, (distance + 1, branch));
                        queue.push_back(w);
                    }
                    // Two branches from `start` meet: a simple cycle.
                    Some(&(other, other_branch)) if self.undirected && other_branch != branch => {
                        let length = distance + other + 1;
                        best = Some(best.map_or(length, |b| b.min(length)));
                    }
                    Some(_) => {}
                }
            }
        }
        Ok(best.filter(|&length| length <= max))
    }

    /// A closed trail of `min..=max` hops when the shortest cycle is shorter
    /// than `min`: depth-first over trails, within the step budget.
    fn longer_trail(
        &self,
        start: u32,
        min: u64,
        max: u64,
        cancel: &dyn Fn() -> Result<(), QueryError>,
    ) -> Result<bool, QueryError> {
        let home = self.component[start as usize];
        let mut used: HashSet<u64> = HashSet::new();
        let mut steps = 0usize;
        // (node, depth, next adjacency slot, relationship entered by)
        let mut work: Vec<(u32, u64, usize, Option<u64>)> = vec![(start, 0, 0, None)];
        while let Some(&mut (v, depth, ref mut at, _)) = work.last_mut() {
            let Some(&(w, rel)) = self.adj[v as usize].get(*at) else {
                if let Some((_, _, _, Some(rel))) = work.pop() {
                    used.remove(&rel);
                }
                continue;
            };
            *at += 1;
            if used.contains(&rel) || self.component[w as usize] != home {
                continue;
            }
            steps += 1;
            if steps > TRAIL_SEARCH_BUDGET {
                return Err(QueryError::ResourceLimit(
                    "$cycle search exceeds its work budget; narrow hops or add a filter".into(),
                ));
            }
            if steps.is_multiple_of(1_024) {
                cancel()?;
            }
            // Back home in range closes the trail; otherwise a trail may pass
            // through its start and carry on (a figure-eight).
            if w == start && depth + 1 >= min && depth < max {
                return Ok(true);
            }
            if depth + 1 < max {
                used.insert(rel);
                work.push((w, depth + 1, 0, Some(rel)));
            }
        }
        Ok(false)
    }
}
