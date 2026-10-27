//! The graph a graph-language mutation runs against: a copy-on-write layer
//! over the tenant's LMPG view. Reads fall through to LMPG; only the records
//! and relationships the statement creates or changes are held in memory,
//! so a write costs what it touches, not the size of the tenant. When the
//! statement succeeds, the layer's diff is the LMPG commit; when it fails,
//! the layer is dropped.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use grafeo_common::types::{EdgeId, EpochId, NodeId, PropertyKey, TransactionId, Value};
use grafeo_common::utils::hash::FxHashMap;
use grafeo_core::graph::lpg::{CompareOp, Edge, Node};
use grafeo_core::graph::{Direction, GraphStore, GraphStoreMut, GraphStoreSearch};
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

    fn current_node(&self, id: NodeId) -> Option<Node> {
        if let Some(entry) = self.read_state().nodes.get(&id) {
            return entry.clone();
        }
        if id.as_u64() >= CREATED_BASE {
            return None;
        }
        self.base.get_node(id)
    }

    fn current_edge(&self, id: EdgeId) -> Option<Edge> {
        if let Some(entry) = self.read_state().edges.get(&id) {
            return entry.clone();
        }
        if id.as_u64() >= CREATED_BASE {
            return None;
        }
        self.base.get_edge(id)
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
        let Some(node) = self.base.get_node(id) else {
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
        let Some(edge) = self.base.get_edge(id) else {
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

    fn next_id(state: &mut State) -> u64 {
        let id = state.next_id;
        state.next_id += 1;
        id
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
            None => self.base.get_node(id),
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

impl GraphStore for OverlayStore {
    fn get_node(&self, id: NodeId) -> Option<Node> {
        self.current_node(id)
    }
    fn get_edge(&self, id: EdgeId) -> Option<Edge> {
        self.current_edge(id)
    }
    fn get_node_versioned(&self, id: NodeId, _: EpochId, _: TransactionId) -> Option<Node> {
        self.current_node(id)
    }
    fn get_edge_versioned(&self, id: EdgeId, _: EpochId, _: TransactionId) -> Option<Edge> {
        self.current_edge(id)
    }
    fn get_node_at_epoch(&self, id: NodeId, _: EpochId) -> Option<Node> {
        self.current_node(id)
    }
    fn get_edge_at_epoch(&self, id: EdgeId, _: EpochId) -> Option<Edge> {
        self.current_edge(id)
    }
    fn get_node_property(&self, id: NodeId, key: &PropertyKey) -> Option<Value> {
        if let Some(entry) = self.read_state().nodes.get(&id) {
            return entry.as_ref()?.get_property(key.as_str()).cloned();
        }
        if id.as_u64() >= CREATED_BASE {
            return None;
        }
        self.base.get_node_property(id, key)
    }
    fn get_edge_property(&self, id: EdgeId, key: &PropertyKey) -> Option<Value> {
        self.current_edge(id)?.get_property(key.as_str()).cloned()
    }
    fn get_node_property_batch(&self, ids: &[NodeId], key: &PropertyKey) -> Vec<Option<Value>> {
        ids.iter()
            .map(|id| self.get_node_property(*id, key))
            .collect()
    }
    fn get_nodes_properties_batch(&self, ids: &[NodeId]) -> Vec<FxHashMap<PropertyKey, Value>> {
        ids.iter()
            .map(|id| {
                self.current_node(*id)
                    .map(|node| {
                        node.properties
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect()
    }
    fn get_nodes_properties_selective_batch(
        &self,
        ids: &[NodeId],
        keys: &[PropertyKey],
    ) -> Vec<FxHashMap<PropertyKey, Value>> {
        ids.iter()
            .map(|id| {
                keys.iter()
                    .filter_map(|key| self.get_node_property(*id, key).map(|v| (key.clone(), v)))
                    .collect()
            })
            .collect()
    }
    fn get_edges_properties_selective_batch(
        &self,
        ids: &[EdgeId],
        keys: &[PropertyKey],
    ) -> Vec<FxHashMap<PropertyKey, Value>> {
        ids.iter()
            .map(|id| {
                keys.iter()
                    .filter_map(|key| self.get_edge_property(*id, key).map(|v| (key.clone(), v)))
                    .collect()
            })
            .collect()
    }
    fn neighbors(&self, node: NodeId, direction: Direction) -> Vec<NodeId> {
        self.edges_from(node, direction)
            .into_iter()
            .map(|(other, _)| other)
            .collect()
    }
    fn edges_from(&self, node: NodeId, direction: Direction) -> Vec<(NodeId, EdgeId)> {
        if self.current_node(node).is_none() {
            return Vec::new();
        }
        let mut edges = if node.as_u64() >= CREATED_BASE {
            Vec::new()
        } else {
            self.base.edges_from(node, direction)
        };
        let state = self.read_state();
        edges.retain(|(_, edge)| !matches!(state.edges.get(edge), Some(None)));
        let added = |map: &HashMap<NodeId, Vec<(NodeId, EdgeId)>>| {
            map.get(&node).cloned().unwrap_or_default()
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
        edges.sort_unstable();
        edges.dedup();
        edges
    }
    fn out_degree(&self, node: NodeId) -> usize {
        self.edges_from(node, Direction::Outgoing).len()
    }
    fn in_degree(&self, node: NodeId) -> usize {
        self.edges_from(node, Direction::Incoming).len()
    }
    fn has_backward_adjacency(&self) -> bool {
        true
    }
    fn node_ids(&self) -> Vec<NodeId> {
        self.corrected(self.base.node_ids(), |_| true)
    }
    fn nodes_by_label(&self, label: &str) -> Vec<NodeId> {
        self.corrected(self.base.nodes_by_label(label), |node| {
            node.has_label(label)
        })
    }
    fn nodes_by_label_count(&self, label: &str) -> usize {
        self.nodes_by_label(label).len()
    }
    fn node_count(&self) -> usize {
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
        (self.base.node_count() + created).saturating_sub(removed)
    }
    fn edge_count(&self) -> usize {
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
        (self.base.edge_count() + created).saturating_sub(removed)
    }
    fn edge_type(&self, id: EdgeId) -> Option<grafeo_common::types::ArcStr> {
        self.current_edge(id).map(|edge| edge.edge_type)
    }
    fn edge_type_versioned(
        &self,
        id: EdgeId,
        _: EpochId,
        _: TransactionId,
    ) -> Option<grafeo_common::types::ArcStr> {
        self.edge_type(id)
    }
    fn has_property_index(&self, property: &str) -> bool {
        self.base.has_property_index(property)
    }
    fn find_nodes_by_property(&self, property: &str, value: &Value) -> Vec<NodeId> {
        self.find_nodes_by_properties(&[(property, value.clone())])
    }
    fn find_nodes_by_properties(&self, conditions: &[(&str, Value)]) -> Vec<NodeId> {
        self.corrected(self.base.find_nodes_by_properties(conditions), |node| {
            conditions_hold(node, conditions)
        })
    }
    fn find_nodes_in_range(
        &self,
        property: &str,
        min: Option<&Value>,
        max: Option<&Value>,
        min_inclusive: bool,
        max_inclusive: bool,
    ) -> Vec<NodeId> {
        self.corrected(
            self.base
                .find_nodes_in_range(property, min, max, min_inclusive, max_inclusive),
            |node| {
                node.get_property(property).is_some_and(|value| {
                    LmpgGraphStore::property_matches_range(
                        value,
                        min,
                        max,
                        min_inclusive,
                        max_inclusive,
                    )
                })
            },
        )
    }
    fn node_property_might_match(&self, _: &PropertyKey, _: CompareOp, _: &Value) -> bool {
        true
    }
    fn edge_property_might_match(&self, _: &PropertyKey, _: CompareOp, _: &Value) -> bool {
        true
    }
    fn statistics(&self) -> Arc<Statistics> {
        self.base.statistics()
    }
    fn estimate_label_cardinality(&self, label: &str) -> f64 {
        self.base.estimate_label_cardinality(label)
    }
    fn estimate_avg_degree(&self, edge_type: &str, outgoing: bool) -> f64 {
        self.base.estimate_avg_degree(edge_type, outgoing)
    }
    fn current_epoch(&self) -> EpochId {
        self.base.current_epoch()
    }
    fn all_labels(&self) -> Vec<String> {
        let mut labels: BTreeSet<String> = self.base.all_labels().into_iter().collect();
        for (_, node) in self.touched_nodes() {
            labels.extend(node.iter().flat_map(labels_of));
        }
        labels.into_iter().collect()
    }
    fn all_edge_types(&self) -> Vec<String> {
        let mut types: BTreeSet<String> = self.base.all_edge_types().into_iter().collect();
        types.extend(
            self.read_state()
                .edges
                .values()
                .flatten()
                .map(|edge| edge.edge_type.to_string()),
        );
        types.into_iter().collect()
    }
    fn all_property_keys(&self) -> Vec<String> {
        let mut keys: BTreeSet<String> = self.base.all_property_keys().into_iter().collect();
        let state = self.read_state();
        for node in state.nodes.values().flatten() {
            keys.extend(node.properties.iter().map(|(key, _)| key.to_string()));
        }
        for edge in state.edges.values().flatten() {
            keys.extend(edge.properties.iter().map(|(key, _)| key.to_string()));
        }
        keys.into_iter().collect()
    }
}

/// Text search reads the committed snapshot: what the statement itself
/// writes is not searchable until it commits.
impl GraphStoreSearch for OverlayStore {
    fn has_text_index(&self, label: &str, property: &str) -> bool {
        self.base.has_text_index(label, property)
    }

    fn score_text(&self, node_id: NodeId, label: &str, property: &str, query: &str) -> Option<f64> {
        self.base.score_text(node_id, label, property, query)
    }

    fn text_search(
        &self,
        label: &str,
        property: &str,
        query: &str,
        k: usize,
    ) -> Vec<(NodeId, f64)> {
        self.base.text_search(label, property, query, k)
    }

    fn text_search_with_threshold(
        &self,
        label: &str,
        property: &str,
        query: &str,
        threshold: f64,
    ) -> Vec<(NodeId, f64)> {
        self.base
            .text_search_with_threshold(label, property, query, threshold)
    }
}

/// Grafeo writes through these (failures surface after execution through
/// [`OverlayStore::check`]: a refused creation returns an id that resolves
/// to nothing).
impl GraphStoreMut for OverlayStore {
    fn create_node(&self, labels: &[&str]) -> NodeId {
        let mut state = self.write_state();
        if !self.admit_node(&mut state)
            || !self
                .budget
                .admit(256usize.saturating_add(labels.iter().map(|label| label.len()).sum()))
        {
            return NodeId::new(u64::MAX);
        }
        let id = NodeId::new(Self::next_id(&mut state));
        state
            .nodes
            .insert(id, Some(Node::with_labels(id, labels.iter().copied())));
        state.created_nodes.push(id);
        id
    }
    fn create_node_versioned(&self, labels: &[&str], _: EpochId, _: TransactionId) -> NodeId {
        self.create_node(labels)
    }
    fn create_edge(&self, src: NodeId, dst: NodeId, edge_type: &str) -> EdgeId {
        if self.current_node(src).is_none()
            || self.current_node(dst).is_none()
            || !self.budget.admit(256usize.saturating_add(edge_type.len()))
        {
            return EdgeId::new(u64::MAX);
        }
        let mut state = self.write_state();
        let id = EdgeId::new(Self::next_id(&mut state));
        state
            .edges
            .insert(id, Some(Edge::new(id, src, dst, edge_type)));
        state.created_edges.push(id);
        state.added_out.entry(src).or_default().push((dst, id));
        state.added_in.entry(dst).or_default().push((src, id));
        id
    }
    fn create_edge_versioned(
        &self,
        src: NodeId,
        dst: NodeId,
        edge_type: &str,
        _: EpochId,
        _: TransactionId,
    ) -> EdgeId {
        self.create_edge(src, dst, edge_type)
    }
    fn batch_create_edges(&self, edges: &[(NodeId, NodeId, &str)]) -> Vec<EdgeId> {
        edges
            .iter()
            .map(|(src, dst, kind)| self.create_edge(*src, *dst, kind))
            .collect()
    }
    fn delete_node(&self, id: NodeId) -> bool {
        let mut state = self.write_state();
        if !self.touch_node(&mut state, id) {
            return false;
        }
        state.nodes.insert(id, None);
        true
    }
    fn delete_node_versioned(&self, id: NodeId, _: EpochId, _: TransactionId) -> bool {
        self.delete_node(id)
    }
    fn delete_node_edges(&self, node_id: NodeId) {
        for (_, edge) in self.edges_from(node_id, Direction::Both) {
            self.delete_edge(edge);
        }
    }
    fn delete_edge(&self, id: EdgeId) -> bool {
        let mut state = self.write_state();
        if id.as_u64() >= CREATED_BASE {
            let Some(Some(edge)) = state.edges.insert(id, None) else {
                return false;
            };
            let state = &mut *state;
            for (map, node) in [
                (&mut state.added_out, edge.src),
                (&mut state.added_in, edge.dst),
            ] {
                if let Some(list) = map.get_mut(&node) {
                    list.retain(|(_, edge)| *edge != id);
                }
            }
            return true;
        }
        if !self.touch_edge(&mut state, id) {
            return false;
        }
        state.edges.insert(id, None);
        true
    }
    fn delete_edge_versioned(&self, id: EdgeId, _: EpochId, _: TransactionId) -> bool {
        self.delete_edge(id)
    }
    /// Setting a property to null removes it (Cypher and GQL semantics).
    fn set_node_property(&self, id: NodeId, key: &str, value: Value) {
        if !self.budget.admit(property_bytes(key, &value)) {
            return;
        }
        let mut state = self.write_state();
        if self.touch_node(&mut state, id) {
            if let Some(Some(node)) = state.nodes.get_mut(&id) {
                if value.is_null() {
                    node.remove_property(key);
                } else {
                    node.set_property(key, value);
                }
            }
        }
    }
    fn set_edge_property(&self, id: EdgeId, key: &str, value: Value) {
        if !self.budget.admit(property_bytes(key, &value)) {
            return;
        }
        let mut state = self.write_state();
        if id.as_u64() >= CREATED_BASE || self.touch_edge(&mut state, id) {
            if let Some(Some(edge)) = state.edges.get_mut(&id) {
                if value.is_null() {
                    edge.remove_property(key);
                } else {
                    edge.set_property(key, value);
                }
            }
        }
    }
    fn remove_node_property(&self, id: NodeId, key: &str) -> Option<Value> {
        let mut state = self.write_state();
        if !self.touch_node(&mut state, id) {
            return None;
        }
        state.nodes.get_mut(&id)?.as_mut()?.remove_property(key)
    }
    fn remove_edge_property(&self, id: EdgeId, key: &str) -> Option<Value> {
        let mut state = self.write_state();
        if id.as_u64() < CREATED_BASE && !self.touch_edge(&mut state, id) {
            return None;
        }
        state.edges.get_mut(&id)?.as_mut()?.remove_property(key)
    }
    fn add_label(&self, node_id: NodeId, label: &str) -> bool {
        if !self.budget.admit(64usize.saturating_add(label.len())) {
            return false;
        }
        let mut state = self.write_state();
        if !self.touch_node(&mut state, node_id) {
            return false;
        }
        let Some(Some(node)) = state.nodes.get_mut(&node_id) else {
            return false;
        };
        if node.has_label(label) {
            return false;
        }
        node.add_label(label);
        true
    }
    fn remove_label(&self, node_id: NodeId, label: &str) -> bool {
        let mut state = self.write_state();
        if !self.touch_node(&mut state, node_id) {
            return false;
        }
        state
            .nodes
            .get_mut(&node_id)
            .and_then(Option::as_mut)
            .is_some_and(|node| node.remove_label(label))
    }
}
