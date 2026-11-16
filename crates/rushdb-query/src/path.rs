//! `query.path`: the shortest path between two records over relationships
//! of one type and direction, within `maxHops`.
//!
//! A bidirectional breadth-first search over the type's adjacency
//! ([`TypeAdjacency`]: the resident graph when the snapshot's cache holds or
//! buys it, otherwise the store's `adj` rows). Each round expands the
//! smaller frontier by one whole level; the first level on which the two
//! searches meet holds a shortest path, and the shortest meeting of that
//! level is returned. A shortest path never repeats a record, so it is also
//! a trail. Every record on the path, its ends included, must exist and
//! pass the caller's `passable` test (labels and policy); the search stops
//! with a resource error once it has reached [`TRAIL_VISIT_BUDGET`] records,
//! never running unbounded.

use std::collections::{HashMap, HashSet};

use rushdb_lmpg::reader::try_access as access;
use rushdb_types::ids::TenantId;

use crate::adjacency::{Adjacency, TypeAdjacency};
use crate::expand::TRAIL_VISIT_BUDGET;
use crate::{Direction, QueryError, QuerySnapshot};

/// One shortest-path question.
pub struct PathRequest<'a> {
    /// Ordinal of the first record.
    pub from: u64,
    /// Ordinal of the last record.
    pub to: u64,
    /// The relationship type followed (every type when `None`).
    pub rel_type: Option<&'a str>,
    /// Which way relationships are followed from `from` towards `to`.
    pub direction: Direction,
    /// Most relationships on the path.
    pub max_hops: u64,
    /// Labels a record on the path may carry (any label when empty).
    pub labels: &'a [String],
}

/// The ordinals of the records on a shortest path from `req.from` to
/// `req.to`, both included, or `None` when none is within `max_hops`.
/// `label_allowed` is the policy's label test, applied to every record on
/// the path.
///
/// # Errors
/// [`QueryError::ResourceLimit`] when the search reaches more than
/// [`TRAIL_VISIT_BUDGET`] records, [`QueryError::Cancelled`] when
/// `is_cancelled` turns true, and storage errors.
pub fn shortest_path(
    snap: &QuerySnapshot,
    tenant: TenantId,
    req: &PathRequest<'_>,
    label_allowed: &dyn Fn(&str) -> bool,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Option<Vec<u64>>, QueryError> {
    let cancel = || {
        if is_cancelled() {
            Err(QueryError::Cancelled)
        } else {
            Ok(())
        }
    };
    let passable = |ord: u64| -> Result<bool, QueryError> {
        Ok(
            access::record_by_ord(snap, tenant, ord)?.is_some_and(|header| {
                (req.labels.is_empty() || req.labels.contains(&header.label))
                    && label_allowed(&header.label)
            }),
        )
    };
    let adjacency = TypeAdjacency::open(snap, tenant, req.rel_type, req.direction, &cancel)?;
    bidirectional(
        &adjacency,
        req.from,
        req.to,
        req.max_hops,
        &passable,
        &cancel,
        TRAIL_VISIT_BUDGET,
    )
}

/// Records reached by one side of the search: record -> the record it was
/// reached from (itself for the side's end).
type Parents = HashMap<u64, u64>;

/// Bidirectional breadth-first search from `from` (forward) and `to`
/// (backward), at most `max` hops, through records `passable` admits,
/// reaching at most `visits` records.
fn bidirectional(
    adjacency: &dyn Adjacency,
    from: u64,
    to: u64,
    max: u64,
    passable: &dyn Fn(u64) -> Result<bool, QueryError>,
    cancel: &dyn Fn() -> Result<(), QueryError>,
    visits: usize,
) -> Result<Option<Vec<u64>>, QueryError> {
    if !passable(from)? || !passable(to)? {
        return Ok(None);
    }
    if from == to {
        return Ok(Some(vec![from]));
    }
    let mut sides = [Side::new(from, true), Side::new(to, false)];
    // Records `passable` refused, shared by both sides.
    let mut refused: HashSet<u64> = HashSet::new();
    let mut edges = Vec::new();
    while sides[0].depth + sides[1].depth < max
        && !sides[0].frontier.is_empty()
        && !sides[1].frontier.is_empty()
    {
        let near = usize::from(sides[1].frontier.len() < sides[0].frontier.len());
        let (expand, other) = if near == 0 {
            let (a, b) = sides.split_at_mut(1);
            (&mut a[0], &b[0])
        } else {
            let (a, b) = sides.split_at_mut(1);
            (&mut b[0], &a[0])
        };
        let mut next = Vec::new();
        // The shortest meeting on this level: (path hops, meeting record).
        let mut best: Option<(u64, u64)> = None;
        for &node in &expand.frontier {
            cancel()?;
            edges.clear();
            adjacency.neighbours(node, expand.forward, &mut edges)?;
            for &(neighbour, _) in &edges {
                if expand.parents.contains_key(&neighbour) || refused.contains(&neighbour) {
                    continue;
                }
                let met = other.depths.get(&neighbour).copied();
                if met.is_none() && !passable(neighbour)? {
                    refused.insert(neighbour);
                    continue;
                }
                if expand.parents.len() + other.parents.len() >= visits {
                    return Err(QueryError::ResourceLimit(
                        "path search reaches more records than its budget; lower maxHops or name labels"
                            .into(),
                    ));
                }
                expand.parents.insert(neighbour, node);
                expand.depths.insert(neighbour, expand.depth + 1);
                if let Some(other_depth) = met {
                    let hops = expand.depth + 1 + other_depth;
                    if best.is_none_or(|(b, _)| hops < b) {
                        best = Some((hops, neighbour));
                    }
                }
                next.push(neighbour);
            }
        }
        expand.depth += 1;
        expand.frontier = next;
        if let Some((_, meeting)) = best {
            return Ok(Some(join(&sides[0].parents, &sides[1].parents, meeting)));
        }
    }
    Ok(None)
}

/// One direction of the search.
struct Side {
    /// Forward from `from`, or backward from `to`.
    forward: bool,
    parents: Parents,
    depths: HashMap<u64, u64>,
    frontier: Vec<u64>,
    /// Hops from this side's end to its frontier.
    depth: u64,
}

impl Side {
    fn new(end: u64, forward: bool) -> Self {
        Self {
            forward,
            parents: HashMap::from([(end, end)]),
            depths: HashMap::from([(end, 0)]),
            frontier: vec![end],
            depth: 0,
        }
    }
}

/// The path through `meeting`: forward parents back to `from`, then
/// backward parents on to `to`.
fn join(forward: &Parents, backward: &Parents, meeting: u64) -> Vec<u64> {
    let mut path = vec![meeting];
    let mut at = meeting;
    while forward[&at] != at {
        at = forward[&at];
        path.push(at);
    }
    path.reverse();
    let mut at = meeting;
    while backward[&at] != at {
        at = backward[&at];
        path.push(at);
    }
    path
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;

    /// A directed multigraph read along `direction`, as `TypeAdjacency`
    /// reads one relationship type.
    struct Graph {
        edges: Vec<(u64, u64)>,
        direction: Direction,
    }

    impl Adjacency for Graph {
        fn neighbours(
            &self,
            node: u64,
            forward: bool,
            out: &mut Vec<(u64, u64)>,
        ) -> Result<(), QueryError> {
            for (rel, &(a, b)) in (0u64..).zip(&self.edges) {
                let outgoing = match self.direction {
                    Direction::Outgoing => forward,
                    Direction::Incoming => !forward,
                    Direction::Both => {
                        if a == node {
                            out.push((b, rel));
                        }
                        if b == node && a != b {
                            out.push((a, rel));
                        }
                        continue;
                    }
                };
                if outgoing && a == node {
                    out.push((b, rel));
                } else if !outgoing && b == node {
                    out.push((a, rel));
                }
            }
            Ok(())
        }

        fn undirected(&self) -> bool {
            self.direction == Direction::Both
        }
    }

    /// Plain breadth-first distance from `from` to `to` through `open`
    /// records: the reference the bidirectional search must agree with.
    fn reference(graph: &Graph, from: u64, to: u64, max: u64, open: &HashSet<u64>) -> Option<u64> {
        if !open.contains(&from) || !open.contains(&to) {
            return None;
        }
        let mut seen = HashMap::from([(from, 0u64)]);
        let mut queue = VecDeque::from([from]);
        let mut edges = Vec::new();
        while let Some(node) = queue.pop_front() {
            let depth = seen[&node];
            if node == to {
                return Some(depth);
            }
            if depth == max {
                continue;
            }
            edges.clear();
            graph.neighbours(node, true, &mut edges).unwrap();
            for &(next, _) in &edges {
                if open.contains(&next) && !seen.contains_key(&next) {
                    seen.insert(next, depth + 1);
                    queue.push_back(next);
                }
            }
        }
        None
    }

    fn xorshift(seed: &mut u64) -> u64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        *seed
    }

    #[test]
    fn bidirectional_search_agrees_with_reference_bfs_on_random_graphs() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let cancel = || Ok(());
        for round in 0..400 {
            let nodes = 2 + xorshift(&mut seed) % 40;
            let edge_count = xorshift(&mut seed) % (nodes * 3);
            let edges = (0..edge_count)
                .map(|_| (xorshift(&mut seed) % nodes, xorshift(&mut seed) % nodes))
                .collect();
            let direction = [Direction::Outgoing, Direction::Incoming, Direction::Both]
                [usize::try_from(round % 3).unwrap()];
            let graph = Graph { edges, direction };
            // Every fourth round closes about a fifth of the records.
            let open: HashSet<u64> = (0..nodes)
                .filter(|_| round % 4 != 0 || !xorshift(&mut seed).is_multiple_of(5))
                .collect();
            let passable = |ord: u64| Ok(open.contains(&ord));
            for _ in 0..10 {
                let (from, to) = (xorshift(&mut seed) % nodes, xorshift(&mut seed) % nodes);
                let max = xorshift(&mut seed) % 8;
                let want = reference(&graph, from, to, max, &open);
                let got =
                    bidirectional(&graph, from, to, max, &passable, &cancel, usize::MAX).unwrap();
                assert_eq!(
                    got.as_ref().map(|path| path.len() as u64 - 1),
                    want,
                    "round {round}: {from} -> {to} within {max} over {:?}",
                    graph.edges
                );
                let Some(path) = got else { continue };
                assert_eq!((path[0], path[path.len() - 1]), (from, to));
                assert_eq!(path.iter().collect::<HashSet<_>>().len(), path.len());
                for step in path.windows(2) {
                    assert!(open.contains(&step[1]));
                    let mut next = Vec::new();
                    graph.neighbours(step[0], true, &mut next).unwrap();
                    assert!(
                        next.iter().any(|&(n, _)| n == step[1]),
                        "{step:?} is no edge"
                    );
                }
            }
        }
    }

    #[test]
    fn a_path_search_stops_at_its_visit_budget() {
        let graph = Graph {
            edges: (0..100).map(|i| (i, i + 1)).collect(),
            direction: Direction::Outgoing,
        };
        let yes = |_: u64| Ok(true);
        let cancel = || Ok(());
        assert_eq!(
            bidirectional(&graph, 0, 100, 100, &yes, &cancel, 1_000)
                .unwrap()
                .map(|path| path.len()),
            Some(101)
        );
        assert!(matches!(
            bidirectional(&graph, 0, 100, 100, &yes, &cancel, 10),
            Err(QueryError::ResourceLimit(_))
        ));
    }
}
