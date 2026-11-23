//! The graph a graph-language mutation runs against: a copy-on-write layer
//! over the tenant's LMPG view. Reads fall through to LMPG; only the records
//! and relationships the statement creates or changes are held in memory,
//! so a write costs what it touches, not the size of the tenant. When the
//! statement succeeds, the layer's diff is the LMPG commit; when it fails,
//! the layer is dropped.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::Range;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use grafeo_common::change::{
    Before, Change, ChangeMark, DataOp, EdgeImage, Entity, Labels, NodeImage, PendingVersion,
    Properties,
};
use grafeo_common::types::{
    ArcStr, EdgeId, EpochId, NodeId, PropertyKey, ReadView as GView, TransactionId, Value,
};
use grafeo_common::utils::error::Result as GResult;
use grafeo_core::graph::apply::{Applied, ApplyError, Writer};
use grafeo_core::graph::lpg::{Edge, Node};
use grafeo_core::graph::traits::ValueRange;
use grafeo_core::graph::{BackendTransaction, Direction, GraphBackend, GraphStore};
use grafeo_core::statistics::Statistics;
use rushdb_lmpg::reader::try_access as access;

use crate::lmpg_store::{values_equal, LmpgGraphStore};
use crate::mutation_budget::{property_bytes, MutationBudget};
use crate::{all_fields, changed_fields, LanguageError};

/// Ids of created nodes and edges start here, above every LMPG ordinal.
const CREATED_BASE: u64 = 1 << 62;

/// The base (LMPG) version of a node or edge the statement touched.
struct Original<T, R> {
    entity: T,
    reference: R,
}

#[derive(Default)]
struct State {
    /// Current version of every touched or created node; `None` once deleted.
    nodes: HashMap<NodeId, Option<Node>>,
    edges: HashMap<EdgeId, Option<Edge>>,
    original_nodes: BTreeMap<NodeId, Original<Node, rushdb_lmpg::RecordRef>>,
    original_edges: BTreeMap<EdgeId, Original<Edge, rushdb_lmpg::RelationshipRef>>,
    created_nodes: Vec<NodeId>,
    created_edges: Vec<EdgeId>,
    /// Adjacency of created edges.
    added_out: HashMap<NodeId, Vec<(NodeId, EdgeId)>>,
    added_in: HashMap<NodeId, Vec<(NodeId, EdgeId)>>,
    next_id: u64,
    /// Nodes touched or created, once that passed `max_nodes`.
    too_large: Option<usize>,
}

impl State {
    fn working_nodes(&self) -> usize {
        self.original_nodes.len() + self.created_nodes.len()
    }
}

/// The exact LMPG delta of a statement, with the layer ids of what it
/// creates (in batch order).
pub(crate) struct GraphDelta {
    pub(crate) batch: rushdb_lmpg::BatchTx,
    pub(crate) created_node_ids: Vec<NodeId>,
    pub(crate) created_edge_ids: Vec<EdgeId>,
}

pub(crate) struct OverlayStore {
    base: LmpgGraphStore,
    budget: MutationBudget,
    max_nodes: usize,
    state: RwLock<State>,
}

impl OverlayStore {
    pub(crate) fn new(base: LmpgGraphStore, max_nodes: usize, max_bytes: usize) -> Self {
        Self {
            base,
            budget: MutationBudget::new(max_bytes),
            max_nodes,
            state: RwLock::new(State {
                next_id: CREATED_BASE,
                ..State::default()
            }),
        }
    }

    pub(crate) fn budget(&self) -> &MutationBudget {
        &self.budget
    }

    fn read_state(&self) -> RwLockReadGuard<'_, State> {
        self.state
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write_state(&self) -> RwLockWriteGuard<'_, State> {
        self.state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Every failure the statement hit: a storage error, the node bound, or
    /// the byte budget.
    pub(crate) fn check(&self) -> Result<(), LanguageError> {
        self.base.text_check()?;
        self.base.check()?;
        if let Some(nodes) = self.read_state().too_large {
            return Err(LanguageError::GraphTooLarge(nodes));
        }
        self.budget.check()
    }

    fn current_node(&self, id: NodeId) -> GResult<Option<Node>> {
        if let Some(entry) = self.read_state().nodes.get(&id) {
            return Ok(entry.clone());
        }
        if id.as_u64() >= CREATED_BASE {
            return Ok(None);
        }
        self.base.get(|| self.base.try_node(id))
    }

    fn current_edge(&self, id: EdgeId) -> GResult<Option<Edge>> {
        if let Some(entry) = self.read_state().edges.get(&id) {
            return Ok(entry.clone());
        }
        if id.as_u64() >= CREATED_BASE {
            return Ok(None);
        }
        self.base.get(|| self.base.try_edge(id))
    }

    /// Bring base node `id` into the layer; `false` when it does not exist,
    /// is deleted, or the working set is full.
    fn touch_node(&self, state: &mut State, id: NodeId) -> bool {
        if let Some(entry) = state.nodes.get(&id) {
            return entry.is_some();
        }
        if id.as_u64() >= CREATED_BASE || !self.admit_node(state) {
            return false;
        }
        let Some(node) = self.base.read(|| self.base.try_node(id)) else {
            return false;
        };
        let header = self.base.read(|| {
            access::record_by_ord(self.base.view().as_ref(), self.base.tenant(), id.as_u64())
        });
        let Some(header) = header else {
            return false;
        };
        self.budget.admit(node_bytes(&node));
        state.nodes.insert(id, Some(node.clone()));
        state.original_nodes.insert(
            id,
            Original {
                entity: node,
                reference: rushdb_lmpg::RecordRef {
                    record_id: header.record_id,
                    ordinal: header.ordinal,
                },
            },
        );
        true
    }

    fn touch_edge(&self, state: &mut State, id: EdgeId) -> bool {
        if let Some(entry) = state.edges.get(&id) {
            return entry.is_some();
        }
        if id.as_u64() >= CREATED_BASE {
            return false;
        }
        let Some(edge) = self.base.read(|| self.base.try_edge(id)) else {
            return false;
        };
        let header = self.base.read(|| {
            access::relationship_by_ord(self.base.view().as_ref(), self.base.tenant(), id.as_u64())
        });
        let Some(header) = header else {
            return false;
        };
        self.budget.admit(edge_bytes(&edge));
        state.edges.insert(id, Some(edge.clone()));
        state.original_edges.insert(
            id,
            Original {
                entity: edge,
                reference: rushdb_lmpg::RelationshipRef {
                    relationship_id: header.relationship_id,
                    ordinal: header.ordinal,
                },
            },
        );
        true
    }

    /// Count one more node in the working set (`max_nodes`).
    fn admit_node(&self, state: &mut State) -> bool {
        let next = state.working_nodes() + 1;
        if next > self.max_nodes {
            state.too_large.get_or_insert(next);
            return false;
        }
        true
    }

    /// Ids the overlay overrides: the base answer for them is stale.
    fn touched_nodes(&self) -> Vec<(NodeId, Option<Node>)> {
        self.read_state()
            .nodes
            .iter()
            .map(|(id, node)| (*id, node.clone()))
            .collect()
    }

    /// Base ids corrected by the layer: overridden ids are dropped, and the
    /// touched or created nodes `keep` accepts are added.
    fn corrected(&self, base: Vec<NodeId>, keep: impl Fn(&Node) -> bool) -> Vec<NodeId> {
        let touched = self.touched_nodes();
        let overridden: std::collections::HashSet<NodeId> =
            touched.iter().map(|(id, _)| *id).collect();
        let mut out: Vec<NodeId> = base
            .into_iter()
            .filter(|id| !overridden.contains(id))
            .collect();
        out.extend(
            touched
                .into_iter()
                .filter_map(|(id, node)| node.filter(|node| keep(node)).map(|_| id)),
        );
        out.sort_unstable();
        out
    }

    /// The LMPG delta of everything the statement did.
    pub(crate) fn delta(&self) -> Result<GraphDelta, LanguageError> {
        self.check()?;
        let state = self.read_state();
        let mut batch = rushdb_lmpg::BatchTx { ops: Vec::new() };
        Self::changes_to_originals(&state, &mut batch, |id| self.current_node_in(&state, id))?;
        let (created_node_ids, node_public_ids) = Self::created_nodes(&state, &mut batch)?;
        Self::edge_updates(&state, &mut batch);
        let mut created_edge_ids = Vec::new();
        for id in &state.created_edges {
            let Some(Some(edge)) = state.edges.get(id) else {
                continue;
            };
            let endpoint = |node: NodeId, role: &str| -> Result<[u8; 16], LanguageError> {
                if self.current_node_in(&state, node).is_none() {
                    return Err(LanguageError::UnsupportedMutation(format!(
                        "new relationship {role} was deleted"
                    )));
                }
                match node_public_ids.get(&node) {
                    Some(id) => Ok(*id),
                    None => self.base_record_id(&state, node),
                }
            };
            let source_id = endpoint(edge.src, "source")?;
            let target_id = endpoint(edge.dst, "target")?;
            created_edge_ids.push(*id);
            batch
                .ops
                .push(rushdb_lmpg::BatchOp::CreateRelationshipById {
                    rel_type: edge.edge_type.to_string(),
                    source_id,
                    target_id,
                    fields: all_fields(&edge.properties_as_btree()),
                });
        }
        drop(state);
        self.check()?;
        Ok(GraphDelta {
            batch,
            created_node_ids,
            created_edge_ids,
        })
    }

    /// Field updates of touched records (label changes are refused), then
    /// standalone relationship deletes, then record deletes (a deleted
    /// record's relationships go with it: LMPG cascades).
    fn changes_to_originals(
        state: &State,
        batch: &mut rushdb_lmpg::BatchTx,
        current_node: impl Fn(NodeId) -> Option<Node>,
    ) -> Result<(), LanguageError> {
        for (id, original) in &state.original_nodes {
            let Some(Some(node)) = state.nodes.get(id) else {
                continue;
            };
            if labels_of(node) != labels_of(&original.entity) {
                return Err(LanguageError::UnsupportedMutation(
                    "LMPG records have one primary label; graph-statement label mutation is unavailable (use native retypeRecord)".into(),
                ));
            }
            let fields = changed_fields(
                &original.entity.properties_as_btree(),
                &node.properties_as_btree(),
            );
            if !fields.is_empty() {
                batch.ops.push(rushdb_lmpg::BatchOp::UpdateFields {
                    owner: rushdb_types::ids::OwnerKind::Record,
                    owner_ord: original.reference.ordinal,
                    owner_public_id: original.reference.record_id,
                    fields,
                });
            }
        }
        for (id, original) in &state.original_edges {
            if matches!(state.edges.get(id), Some(None))
                && current_node(original.entity.src).is_some()
                && current_node(original.entity.dst).is_some()
            {
                batch.ops.push(rushdb_lmpg::BatchOp::DeleteRelationship {
                    rel: original.reference,
                });
            }
        }
        for (id, original) in &state.original_nodes {
            if matches!(state.nodes.get(id), Some(None)) {
                batch.ops.push(rushdb_lmpg::BatchOp::DeleteRecord {
                    record: original.reference,
                });
            }
        }
        Ok(())
    }

    /// Creates of the records the statement made, with the public ids
    /// chosen for them (so relationships in the same batch resolve them).
    #[expect(
        clippy::type_complexity,
        reason = "the created ids in batch order and their public ids, consumed together"
    )]
    fn created_nodes(
        state: &State,
        batch: &mut rushdb_lmpg::BatchTx,
    ) -> Result<(Vec<NodeId>, HashMap<NodeId, [u8; 16]>), LanguageError> {
        let mut public_ids = HashMap::new();
        let mut created = Vec::new();
        for id in &state.created_nodes {
            let Some(Some(node)) = state.nodes.get(id) else {
                continue;
            };
            if node.labels.len() != 1 {
                return Err(LanguageError::UnsupportedMutation(
                    "new LMPG records require exactly one label".into(),
                ));
            }
            let record_id = *uuid::Uuid::now_v7().as_bytes();
            public_ids.insert(*id, record_id);
            created.push(*id);
            batch.ops.push(rushdb_lmpg::BatchOp::CreateRecord {
                label: node.labels[0].to_string(),
                explicit_id: Some(record_id),
                fields: all_fields(&node.properties_as_btree()),
            });
        }
        Ok((created, public_ids))
    }

    /// Field updates of touched relationships.
    fn edge_updates(state: &State, batch: &mut rushdb_lmpg::BatchTx) {
        for (id, original) in &state.original_edges {
            let Some(Some(edge)) = state.edges.get(id) else {
                continue;
            };
            let fields = changed_fields(
                &original.entity.properties_as_btree(),
                &edge.properties_as_btree(),
            );
            if !fields.is_empty() {
                batch.ops.push(rushdb_lmpg::BatchOp::UpdateFields {
                    owner: rushdb_types::ids::OwnerKind::Relationship,
                    owner_ord: original.reference.ordinal,
                    owner_public_id: original.reference.relationship_id,
                    fields,
                });
            }
        }
    }

    fn current_node_in(&self, state: &State, id: NodeId) -> Option<Node> {
        match state.nodes.get(&id) {
            Some(entry) => entry.clone(),
            None if id.as_u64() >= CREATED_BASE => None,
            None => self.base.read(|| self.base.try_node(id)),
        }
    }

    fn base_record_id(&self, state: &State, id: NodeId) -> Result<[u8; 16], LanguageError> {
        if let Some(original) = state.original_nodes.get(&id) {
            return Ok(original.reference.record_id);
        }
        Ok(
            access::record_by_ord(self.base.view().as_ref(), self.base.tenant(), id.as_u64())?
                .ok_or_else(|| LanguageError::Initialization("record vanished".into()))?
                .record_id,
        )
    }

    /// The public id of node `id` after the commit: `created` for a new one,
    /// the record's id otherwise.
    pub(crate) fn public_node_id(
        &self,
        id: NodeId,
        created: &HashMap<NodeId, [u8; 16]>,
    ) -> Option<[u8; 16]> {
        if let Some(public) = created.get(&id) {
            return Some(*public);
        }
        self.base_record_id(&self.read_state(), id).ok()
    }

    /// The public id of edge `id` after the commit.
    pub(crate) fn public_edge_id(
        &self,
        id: EdgeId,
        created: &HashMap<EdgeId, [u8; 16]>,
    ) -> Option<[u8; 16]> {
        if let Some(public) = created.get(&id) {
            return Some(*public);
        }
        if let Some(original) = self.read_state().original_edges.get(&id) {
            return Some(original.reference.relationship_id);
        }
        access::relationship_by_ord(self.base.view().as_ref(), self.base.tenant(), id.as_u64())
            .ok()
            .flatten()
            .map(|header| header.relationship_id)
    }
}

fn labels_of(node: &Node) -> Vec<String> {
    node.labels.iter().map(ToString::to_string).collect()
}

fn node_bytes(node: &Node) -> usize {
    256usize
        .saturating_add(
            node.labels
                .iter()
                .map(grafeo_common::types::ArcStr::len)
                .sum::<usize>(),
        )
        .saturating_add(
            node.properties
                .iter()
                .map(|(key, value)| property_bytes(key.as_str(), value))
                .sum::<usize>(),
        )
}

fn edge_bytes(edge: &Edge) -> usize {
    256usize
        .saturating_add(edge.edge_type.len())
        .saturating_add(
            edge.properties
                .iter()
                .map(|(key, value)| property_bytes(key.as_str(), value))
                .sum::<usize>(),
        )
}

fn conditions_hold(node: &Node, conditions: &[(&str, Value)]) -> bool {
    conditions.iter().all(|(property, expected)| {
        node.get_property(property)
            .is_some_and(|actual| values_equal(actual, expected))
    })
}

/// What [`GraphBackend::apply`] answers for a write to something its writer
/// does not see: nothing changed for a transaction, an error for replay.
fn unseen(writer: Writer, entity: Entity) -> Result<Applied, ApplyError> {
    match writer {
        Writer::Replay { .. } => Err(ApplyError::Missing(entity)),
        _ => Ok(Applied::Unchanged),
    }
}

/// A write that changed what the layer holds, undone from `before`.
const fn changed(before: Before) -> Applied {
    Applied::Changed {
        before,
        version: PendingVersion::Created,
    }
}

/// A refusal the layer already recorded (node bound, byte budget):
/// [`OverlayStore::check`] reports it once the statement returns.
fn refused() -> ApplyError {
    ApplyError::Refused("the statement's working set is full".into())
}

fn properties_of(properties: &grafeo_common::types::PropertyMap) -> Properties {
    properties
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

impl GraphStore for OverlayStore {
    fn node(&self, id: NodeId, _: GView) -> GResult<Option<Node>> {
        self.current_node(id)
    }
    fn edge(&self, id: EdgeId, _: GView) -> GResult<Option<Edge>> {
        self.current_edge(id)
    }
    fn node_property(&self, id: NodeId, key: &str, _: GView) -> GResult<Option<Value>> {
        if let Some(entry) = self.read_state().nodes.get(&id) {
            return Ok(entry
                .as_ref()
                .and_then(|node| node.get_property(key).cloned()));
        }
        if id.as_u64() >= CREATED_BASE {
            return Ok(None);
        }
        self.base.get(|| self.base.try_node_property(id, key))
    }
    fn edge_property(&self, id: EdgeId, key: &str, _: GView) -> GResult<Option<Value>> {
        Ok(self
            .current_edge(id)?
            .and_then(|edge| edge.get_property(key).cloned()))
    }
    fn node_labels(&self, id: NodeId, view: GView) -> GResult<Option<Labels>> {
        if let Some(entry) = self.read_state().nodes.get(&id) {
            return Ok(entry.as_ref().map(|node| node.labels.clone()));
        }
        if id.as_u64() >= CREATED_BASE {
            return Ok(None);
        }
        self.base.node_labels(id, view)
    }
    fn edge_type(&self, id: EdgeId, view: GView) -> GResult<Option<ArcStr>> {
        if let Some(entry) = self.read_state().edges.get(&id) {
            return Ok(entry.as_ref().map(|edge| edge.edge_type.clone()));
        }
        if id.as_u64() >= CREATED_BASE {
            return Ok(None);
        }
        self.base.edge_type(id, view)
    }
    fn edges(
        &self,
        node: NodeId,
        direction: Direction,
        types: &[ArcStr],
        _: GView,
    ) -> GResult<Vec<(NodeId, EdgeId)>> {
        if self.current_node(node)?.is_none() {
            return Ok(Vec::new());
        }
        let mut edges = if node.as_u64() >= CREATED_BASE {
            Vec::new()
        } else {
            self.base
                .get(|| self.base.try_edges(node, direction, types))?
        };
        let state = self.read_state();
        edges.retain(|(_, edge)| !matches!(state.edges.get(edge), Some(None)));
        let of_type = |edge: &EdgeId| {
            types.is_empty()
                || state.edges.get(edge).is_some_and(|edge| {
                    edge.as_ref()
                        .is_some_and(|edge| types.contains(&edge.edge_type))
                })
        };
        let added = |map: &HashMap<NodeId, Vec<(NodeId, EdgeId)>>| {
            map.get(&node)
                .into_iter()
                .flatten()
                .filter(|(_, edge)| of_type(edge))
                .copied()
                .collect::<Vec<_>>()
        };
        match direction {
            Direction::Outgoing => edges.extend(added(&state.added_out)),
            Direction::Incoming => edges.extend(added(&state.added_in)),
            Direction::Both => {
                edges.extend(added(&state.added_out));
                edges.extend(added(&state.added_in));
            }
            _ => {}
        }
        drop(state);
        edges.sort_unstable();
        edges.dedup();
        Ok(edges)
    }
    fn has_backward_adjacency(&self) -> bool {
        true
    }
    fn node_ids(&self, view: GView) -> GResult<Vec<NodeId>> {
        Ok(self.corrected(self.base.node_ids(view)?, |_| true))
    }
    fn nodes_by_label(&self, label: &str, view: GView) -> GResult<Vec<NodeId>> {
        Ok(
            self.corrected(self.base.nodes_by_label(label, view)?, |node| {
                node.has_label(label)
            }),
        )
    }
    fn node_count(&self, view: GView) -> GResult<usize> {
        let state = self.read_state();
        let removed = state
            .original_nodes
            .keys()
            .filter(|id| matches!(state.nodes.get(id), Some(None)))
            .count();
        let created = state
            .created_nodes
            .iter()
            .filter(|id| matches!(state.nodes.get(id), Some(Some(_))))
            .count();
        drop(state);
        Ok((self.base.node_count(view)? + created).saturating_sub(removed))
    }
    fn edge_count(&self, view: GView) -> GResult<usize> {
        let state = self.read_state();
        let removed = state
            .original_edges
            .keys()
            .filter(|id| matches!(state.edges.get(id), Some(None)))
            .count();
        let created = state
            .created_edges
            .iter()
            .filter(|id| matches!(state.edges.get(id), Some(Some(_))))
            .count();
        drop(state);
        Ok((self.base.edge_count(view)? + created).saturating_sub(removed))
    }
    fn has_property_index(&self, property: &str) -> bool {
        self.base.has_property_index(property)
    }
    fn find_nodes_by_property(
        &self,
        property: &str,
        value: &Value,
        view: GView,
    ) -> GResult<Vec<NodeId>> {
        self.find_nodes_by_properties(&[(property, value.clone())], view)
    }
    fn find_nodes_by_properties(
        &self,
        conditions: &[(&str, Value)],
        view: GView,
    ) -> GResult<Vec<NodeId>> {
        Ok(self.corrected(
            self.base.find_nodes_by_properties(conditions, view)?,
            |node| conditions_hold(node, conditions),
        ))
    }
    fn find_nodes_in_range(
        &self,
        property: &str,
        range: &ValueRange,
        view: GView,
    ) -> GResult<Vec<NodeId>> {
        Ok(self.corrected(
            self.base.find_nodes_in_range(property, range, view)?,
            |node| {
                node.get_property(property)
                    .is_some_and(|value| range.contains(value))
            },
        ))
    }
    fn statistics(&self) -> Arc<Statistics> {
        self.base.statistics()
    }
    fn label_node_estimate(&self, label: &str) -> u64 {
        self.base.label_node_estimate(label)
    }
    fn node_estimate(&self) -> u64 {
        self.base.node_estimate()
    }
    fn committed_epoch(&self) -> EpochId {
        self.base.committed_epoch()
    }
    fn all_labels(&self) -> GResult<Vec<String>> {
        let mut labels: BTreeSet<String> = self.base.all_labels()?.into_iter().collect();
        for (_, node) in self.touched_nodes() {
            labels.extend(node.iter().flat_map(labels_of));
        }
        Ok(labels.into_iter().collect())
    }
    fn all_edge_types(&self) -> GResult<Vec<String>> {
        let mut types: BTreeSet<String> = self.base.all_edge_types()?.into_iter().collect();
        types.extend(
            self.read_state()
                .edges
                .values()
                .flatten()
                .map(|edge| edge.edge_type.to_string()),
        );
        Ok(types.into_iter().collect())
    }
    fn all_property_keys(&self) -> GResult<Vec<String>> {
        let mut keys: BTreeSet<String> = self.base.all_property_keys()?.into_iter().collect();
        let state = self.read_state();
        for node in state.nodes.values().flatten() {
            keys.extend(node.properties.iter().map(|(key, _)| key.to_string()));
        }
        for edge in state.edges.values().flatten() {
            keys.extend(edge.properties.iter().map(|(key, _)| key.to_string()));
        }
        Ok(keys.into_iter().collect())
    }

    // Text search reads the committed snapshot: what the statement itself
    // writes is not searchable until it commits.
    fn has_text_index(&self, label: &str, property: &str) -> bool {
        self.base.has_text_index(label, property)
    }
    fn score_text(
        &self,
        node_id: NodeId,
        label: &str,
        property: &str,
        query: &str,
        view: GView,
    ) -> GResult<Option<f64>> {
        self.base.score_text(node_id, label, property, query, view)
    }
    fn text_search(
        &self,
        label: &str,
        property: &str,
        query: &str,
        k: usize,
        view: GView,
    ) -> GResult<Vec<(NodeId, f64)>> {
        self.base.text_search(label, property, query, k, view)
    }
    fn text_search_with_threshold(
        &self,
        label: &str,
        property: &str,
        query: &str,
        threshold: f64,
        view: GView,
    ) -> GResult<Vec<(NodeId, f64)>> {
        self.base
            .text_search_with_threshold(label, property, query, threshold, view)
    }
}

/// Grafeo writes through these. The layer is the statement's only
/// transaction, so a commit keeps what was applied (the statement's delta
/// is read afterwards) and a rollback writes the before-images back.
/// Refusals of the node bound and the byte budget are recorded first and
/// surface through [`OverlayStore::check`].
impl GraphBackend for OverlayStore {
    fn reserve_node_ids(&self, count: u64) -> Result<Range<u64>, ApplyError> {
        Ok(Self::reserve(&mut self.write_state(), count))
    }
    fn reserve_edge_ids(&self, count: u64) -> Result<Range<u64>, ApplyError> {
        Ok(Self::reserve(&mut self.write_state(), count))
    }
    fn begin(
        &self,
        transaction: TransactionId,
        snapshot: EpochId,
    ) -> Result<BackendTransaction, ApplyError> {
        Ok(BackendTransaction::new(transaction, snapshot))
    }
    fn apply(&self, op: &DataOp, writer: Writer) -> Result<Applied, ApplyError> {
        match op {
            DataOp::CreateNode {
                id,
                labels,
                properties,
            } => self.apply_create_node(*id, labels, properties),
            DataOp::CreateEdge {
                id,
                src,
                dst,
                edge_type,
                properties,
            } => self.apply_create_edge(*id, *src, *dst, edge_type, properties),
            DataOp::DeleteNode { id } => {
                let mut state = self.write_state();
                if !self.touch_node(&mut state, *id) {
                    return unseen(writer, Entity::Node(*id));
                }
                let Some(Some(node)) = state.nodes.insert(*id, None) else {
                    return unseen(writer, Entity::Node(*id));
                };
                Ok(changed(Before::Node(Box::new(NodeImage {
                    labels: node.labels.clone(),
                    properties: properties_of(&node.properties),
                }))))
            }
            DataOp::DeleteEdge { id } => {
                let mut state = self.write_state();
                if !self.touch_edge(&mut state, *id) {
                    return unseen(writer, Entity::Edge(*id));
                }
                let Some(Some(edge)) = state.edges.insert(*id, None) else {
                    return unseen(writer, Entity::Edge(*id));
                };
                if id.as_u64() >= CREATED_BASE {
                    Self::unlink(&mut state, &edge);
                }
                Ok(changed(Before::Edge(Box::new(EdgeImage {
                    src: edge.src,
                    dst: edge.dst,
                    edge_type: edge.edge_type.clone(),
                    properties: properties_of(&edge.properties),
                    deleted_with: None,
                }))))
            }
            DataOp::SetNodeProperty { id, key, value } => {
                if !self.budget.admit(property_bytes(key.as_str(), value)) {
                    return Err(refused());
                }
                self.with_node(*id, writer, |node| {
                    let before = node.get_property(key.as_str()).cloned();
                    set_or_remove(&mut node.properties, key, value);
                    Some(Before::Value(before))
                })
            }
            DataOp::RemoveNodeProperty { id, key } => self.with_node(*id, writer, |node| {
                node.remove_property(key.as_str())
                    .map(|before| Before::Value(Some(before)))
            }),
            DataOp::SetEdgeProperty { id, key, value } => {
                if !self.budget.admit(property_bytes(key.as_str(), value)) {
                    return Err(refused());
                }
                self.with_edge(*id, writer, |edge| {
                    let before = edge.get_property(key.as_str()).cloned();
                    set_or_remove(&mut edge.properties, key, value);
                    Some(Before::Value(before))
                })
            }
            DataOp::RemoveEdgeProperty { id, key } => self.with_edge(*id, writer, |edge| {
                edge.remove_property(key.as_str())
                    .map(|before| Before::Value(Some(before)))
            }),
            DataOp::AddNodeLabel { id, label } => {
                if !self.budget.admit(64usize.saturating_add(label.len())) {
                    return Err(refused());
                }
                self.with_node(*id, writer, |node| {
                    if node.has_label(label) {
                        return None;
                    }
                    let before = node.labels.clone();
                    node.add_label(label.clone());
                    Some(Before::Labels(before))
                })
            }
            DataOp::RemoveNodeLabel { id, label } => self.with_node(*id, writer, |node| {
                let before = node.labels.clone();
                node.remove_label(label).then_some(Before::Labels(before))
            }),
            DataOp::InsertTriple { .. } | DataOp::DeleteTriple { .. } => Err(ApplyError::Refused(
                "a tenant graph holds no RDF triples".into(),
            )),
        }
    }
    fn commit(
        &self,
        _: BackendTransaction,
        _: &mut dyn Iterator<Item = &Change>,
        _: EpochId,
    ) -> Result<(), ApplyError> {
        Ok(())
    }
    fn rollback(
        &self,
        _: BackendTransaction,
        changes: &mut dyn DoubleEndedIterator<Item = &Change>,
    ) -> Result<(), ApplyError> {
        self.undo(changes)
    }
    fn rollback_to(
        &self,
        _: BackendTransaction,
        _: ChangeMark,
        changes_since: &mut dyn DoubleEndedIterator<Item = &Change>,
    ) -> Result<(), ApplyError> {
        self.undo(changes_since)
    }
    fn collect(&self, _: EpochId) {}
    fn durable_epoch(&self) -> EpochId {
        EpochId::INITIAL
    }
    fn flush(&self, _: EpochId) -> Result<(), ApplyError> {
        Ok(())
    }
}

/// Set `key` to `value` in `properties`; a null removes it (Cypher and GQL
/// semantics).
fn set_or_remove(
    properties: &mut grafeo_common::types::PropertyMap,
    key: &PropertyKey,
    value: &Value,
) {
    if value.is_null() {
        properties.remove(key);
    } else {
        properties.insert(key.clone(), value.clone());
    }
}

impl OverlayStore {
    fn reserve(state: &mut State, count: u64) -> Range<u64> {
        let start = state.next_id;
        state.next_id = start.saturating_add(count);
        start..state.next_id
    }

    fn apply_create_node(
        &self,
        id: NodeId,
        labels: &Labels,
        properties: &Properties,
    ) -> Result<Applied, ApplyError> {
        let mut state = self.write_state();
        if state.nodes.contains_key(&id) {
            return Err(ApplyError::Exists(Entity::Node(id)));
        }
        let mut node = Node::with_labels(id, labels.iter().cloned());
        for (key, value) in properties {
            set_or_remove(&mut node.properties, key, value);
        }
        if !self.admit_node(&mut state) || !self.budget.admit(node_bytes(&node)) {
            return Err(refused());
        }
        state.nodes.insert(id, Some(node));
        state.created_nodes.push(id);
        Ok(changed(Before::Absent))
    }

    fn apply_create_edge(
        &self,
        id: EdgeId,
        src: NodeId,
        dst: NodeId,
        edge_type: &ArcStr,
        properties: &Properties,
    ) -> Result<Applied, ApplyError> {
        for end in [src, dst] {
            if self
                .current_node(end)
                .map_err(|error| ApplyError::Refused(error.to_string()))?
                .is_none()
            {
                return Err(ApplyError::Missing(Entity::Node(end)));
            }
        }
        let mut edge = Edge::new(id, src, dst, edge_type.clone());
        for (key, value) in properties {
            set_or_remove(&mut edge.properties, key, value);
        }
        if !self.budget.admit(edge_bytes(&edge)) {
            return Err(refused());
        }
        let mut state = self.write_state();
        if state.edges.contains_key(&id) {
            return Err(ApplyError::Exists(Entity::Edge(id)));
        }
        state.edges.insert(id, Some(edge));
        state.created_edges.push(id);
        state.added_out.entry(src).or_default().push((dst, id));
        state.added_in.entry(dst).or_default().push((src, id));
        Ok(changed(Before::Absent))
    }

    /// Change node `id` in the layer; `change` answers the before-image, or
    /// `None` when it changed nothing.
    fn with_node(
        &self,
        id: NodeId,
        writer: Writer,
        change: impl FnOnce(&mut Node) -> Option<Before>,
    ) -> Result<Applied, ApplyError> {
        let mut state = self.write_state();
        if !self.touch_node(&mut state, id) {
            return unseen(writer, Entity::Node(id));
        }
        let Some(Some(node)) = state.nodes.get_mut(&id) else {
            return unseen(writer, Entity::Node(id));
        };
        Ok(change(node).map_or(Applied::Unchanged, changed))
    }

    fn with_edge(
        &self,
        id: EdgeId,
        writer: Writer,
        change: impl FnOnce(&mut Edge) -> Option<Before>,
    ) -> Result<Applied, ApplyError> {
        let mut state = self.write_state();
        if !self.touch_edge(&mut state, id) {
            return unseen(writer, Entity::Edge(id));
        }
        let Some(Some(edge)) = state.edges.get_mut(&id) else {
            return unseen(writer, Entity::Edge(id));
        };
        Ok(change(edge).map_or(Applied::Unchanged, changed))
    }

    /// Drop a created edge from the layer's adjacency.
    fn unlink(state: &mut State, edge: &Edge) {
        for (map, node) in [
            (&mut state.added_out, edge.src),
            (&mut state.added_in, edge.dst),
        ] {
            if let Some(list) = map.get_mut(&node) {
                list.retain(|(_, id)| *id != edge.id);
            }
        }
    }

    /// Undo `changes`, last to first, from their before-images.
    fn undo(
        &self,
        changes: &mut dyn DoubleEndedIterator<Item = &Change>,
    ) -> Result<(), ApplyError> {
        let mut state = self.write_state();
        while let Some(change) = changes.next_back() {
            let Change::Data { op, before, .. } = change else {
                return Err(ApplyError::Refused(
                    "the layer records no bulk or RDF writes".into(),
                ));
            };
            Self::undo_one(&mut state, op, before)?;
        }
        Ok(())
    }

    fn undo_one(state: &mut State, op: &DataOp, before: &Before) -> Result<(), ApplyError> {
        let broken = || ApplyError::Refused(format!("no undo for a {} write", op.kind()));
        match (op, before) {
            (DataOp::CreateNode { id, .. }, Before::Absent) => {
                state.nodes.remove(id);
                state.created_nodes.retain(|created| created != id);
            }
            (DataOp::CreateEdge { id, .. }, Before::Absent) => {
                if let Some(Some(edge)) = state.edges.remove(id) {
                    Self::unlink(state, &edge);
                }
                state.created_edges.retain(|created| created != id);
            }
            (DataOp::DeleteNode { id }, Before::Node(image)) => {
                let mut node = Node::with_labels(*id, image.labels.iter().cloned());
                for (key, value) in &image.properties {
                    node.set_property(key.clone(), value.clone());
                }
                state.nodes.insert(*id, Some(node));
            }
            (DataOp::DeleteEdge { id }, Before::Edge(image)) => {
                let mut edge = Edge::new(*id, image.src, image.dst, image.edge_type.clone());
                for (key, value) in &image.properties {
                    edge.set_property(key.clone(), value.clone());
                }
                if id.as_u64() >= CREATED_BASE {
                    state
                        .added_out
                        .entry(edge.src)
                        .or_default()
                        .push((edge.dst, *id));
                    state
                        .added_in
                        .entry(edge.dst)
                        .or_default()
                        .push((edge.src, *id));
                }
                state.edges.insert(*id, Some(edge));
            }
            (
                DataOp::SetNodeProperty { id, key, .. } | DataOp::RemoveNodeProperty { id, key },
                Before::Value(value),
            ) => {
                let node = state
                    .nodes
                    .get_mut(id)
                    .and_then(Option::as_mut)
                    .ok_or_else(broken)?;
                set_or_remove(
                    &mut node.properties,
                    key,
                    value.as_ref().unwrap_or(&Value::Null),
                );
            }
            (
                DataOp::SetEdgeProperty { id, key, .. } | DataOp::RemoveEdgeProperty { id, key },
                Before::Value(value),
            ) => {
                let edge = state
                    .edges
                    .get_mut(id)
                    .and_then(Option::as_mut)
                    .ok_or_else(broken)?;
                set_or_remove(
                    &mut edge.properties,
                    key,
                    value.as_ref().unwrap_or(&Value::Null),
                );
            }
            (
                DataOp::AddNodeLabel { id, .. } | DataOp::RemoveNodeLabel { id, .. },
                Before::Labels(labels),
            ) => {
                let node = state
                    .nodes
                    .get_mut(id)
                    .and_then(Option::as_mut)
                    .ok_or_else(broken)?;
                node.labels.clone_from(labels);
            }
            _ => return Err(broken()),
        }
        Ok(())
    }
}
