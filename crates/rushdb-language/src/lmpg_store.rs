//! Read-only Grafeo storage contract backed directly by one LMPG tenant.
//!
//! Records are reconstructed from their occurrences on demand, with the
//! property families cached for the whole statement. Equality lookups go
//! through declared ordered indexes when every label holding the property
//! has one (`has_property_index`); otherwise Grafeo scans the label.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, Mutex};

use grafeo_common::types::{EdgeId, EpochId, NodeId, PropertyKey, TransactionId, Value};
use grafeo_common::utils::hash::FxHashMap;
use grafeo_core::graph::lpg::{CompareOp, Edge, Node};
use grafeo_core::graph::{Direction, GraphStore, GraphStoreSearch};
use grafeo_core::statistics::Statistics;
use rushdb_lmpg::reader::try_access as access;
use rushdb_lmpg::reconstruct::{reconstruct_relationship, FamilyCache, ReconstructSession};
use rushdb_storage::{OverlayView, StorageError};
use rushdb_types::ids::TenantId;
use rushdb_types::index::IndexDef;
use rushdb_types::keys::{key, P_REL_BY_ORD};
use rushdb_types::model::LogicalType;
use rushdb_types::value::{scalar_cmp, Scalar};
use serde_json::Value as Json;

use crate::json_to_value_checked;

/// Read-only Grafeo storage contract backed directly by one LMPG tenant.
/// Every match of one text search, by record ordinal, keyed by (index,
/// query).
type TextMatches = HashMap<(String, String), Arc<HashMap<u64, f64>>>;

pub(crate) struct LmpgGraphStore {
    view: Arc<OverlayView>,
    tenant: TenantId,
    failure: Mutex<Option<StorageError>>,
    /// Families resolved so far by this statement's reconstructions.
    families: Mutex<FamilyCache>,
    /// Per property: the ordered indexes answering equality on it (one per
    /// label holding it), or `None` when some label has none.
    equality_indexes: Mutex<HashMap<String, Option<Vec<IndexDef>>>>,
    /// The node's text indexes; `text_match`/`text_score` search them.
    text: Option<Arc<rushdb_textsearch::TextIndexes>>,
    /// Every match of one (index, query), by record ordinal: per-row
    /// scoring reads it instead of searching again.
    text_matches: Mutex<TextMatches>,
    /// A failed text search (unknown option, unparsable query), reported
    /// like a storage failure at the statement boundary.
    text_failure: Mutex<Option<String>>,
}

impl LmpgGraphStore {
    /// `(records, relationships)` from the label and relationship catalogs.
    fn catalog_totals(&self) -> (u64, u64) {
        let nodes = self
            .read(|| access::label_counts(&self.view, self.tenant))
            .iter()
            .map(|(_, count)| count)
            .sum();
        let edges = self
            .read(|| access::relationship_catalog(&self.view, self.tenant))
            .iter()
            .map(|(_, _, _, count)| count)
            .sum();
        (nodes, edges)
    }

    pub(crate) fn new(
        view: Arc<OverlayView>,
        tenant: TenantId,
        text: Option<Arc<rushdb_textsearch::TextIndexes>>,
    ) -> Self {
        Self {
            view,
            tenant,
            failure: Mutex::new(None),
            families: Mutex::new(FamilyCache::default()),
            equality_indexes: Mutex::new(HashMap::new()),
            text,
            text_matches: Mutex::new(HashMap::new()),
            text_failure: Mutex::new(None),
        }
    }

    /// The first text search failure of the statement, if any.
    pub(crate) fn text_check(&self) -> Result<(), crate::LanguageError> {
        match self
            .text_failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            Some(error) => Err(crate::LanguageError::TextSearch(error.clone())),
            None => Ok(()),
        }
    }

    /// The text index answering `text_match(n.<property>, …)` on `label`:
    /// the `ready` index on exactly that property (`*` for the index over
    /// every string of the record).
    fn text_index(&self, label: &str, property: &str) -> Option<String> {
        self.text.as_ref()?;
        self.read(|| {
            rushdb_textsearch::TextIndexes::index_for(
                self.view.as_ref(),
                self.tenant,
                label,
                property,
            )
        })
    }

    /// Record the statement's first text search failure and stop its work.
    fn text_failed(&self, error: String) {
        let mut failure = self
            .text_failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if failure.is_none() {
            *failure = Some(error);
        }
        self.view.cancellation().cancel();
    }

    /// Run one search of `index`, recording a failure instead of returning it.
    fn text_hits(&self, index: &str, query: &str, top_k: usize) -> Vec<(u64, f64)> {
        let Some(text) = &self.text else {
            return Vec::new();
        };
        if self.text_check().is_err() {
            return Vec::new();
        }
        let found = text.search(
            self.tenant,
            self.view.base(),
            rushdb_textsearch::TextQuery {
                index,
                query,
                opts: rushdb_textsearch::LexicalOptions::default(),
                top_k,
            },
            None,
        );
        match found {
            Ok(found) => found
                .hits
                .into_iter()
                .map(|hit| (hit.owner_ord, hit.score))
                .collect(),
            Err(error) => {
                self.text_failed(error);
                Vec::new()
            }
        }
    }

    /// Every match of `query` in `index`, searched once per statement.
    fn text_matches(&self, index: &str, query: &str) -> Arc<HashMap<u64, f64>> {
        let key = (index.to_owned(), query.to_owned());
        if let Some(found) = self
            .text_matches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&key)
        {
            return Arc::clone(found);
        }
        let found: Arc<HashMap<u64, f64>> = Arc::new(
            self.text_hits(index, query, rushdb_textsearch::ALL_HITS)
                .into_iter()
                .collect(),
        );
        self.text_matches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key, Arc::clone(&found));
        found
    }

    pub(crate) fn view(&self) -> &Arc<OverlayView> {
        &self.view
    }

    pub(crate) fn tenant(&self) -> TenantId {
        self.tenant
    }

    /// Run `reconstruct` in a session that keeps the statement's family
    /// cache.
    fn with_session<T>(
        &self,
        reconstruct: impl FnOnce(&mut ReconstructSession<'_, OverlayView>) -> Result<T, StorageError>,
    ) -> Result<T, StorageError> {
        let cache = std::mem::take(
            &mut *self
                .families
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        let mut session = ReconstructSession::with_cache(self.view.as_ref(), self.tenant, cache);
        let result = reconstruct(&mut session);
        *self
            .families
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = session.into_cache();
        result
    }

    pub(crate) fn check(&self) -> Result<(), StorageError> {
        match self
            .failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    // Grafeo 0.5.42's GraphStore methods cannot return storage errors. Poison
    // the whole statement on first failure, stop further storage work, and
    // reject its result at every engine boundary. This adapter is private so
    // callers cannot accidentally accept its terminal placeholder values.
    pub(crate) fn read<T: Default>(&self, read: impl FnOnce() -> Result<T, StorageError>) -> T {
        if self.check().is_err() {
            return T::default();
        }
        match read() {
            Ok(value) => value,
            Err(error) => {
                let mut failure = self
                    .failure
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if failure.is_none() {
                    *failure = Some(error);
                }
                self.view.cancellation().cancel();
                T::default()
            }
        }
    }

    fn node(&self, id: NodeId) -> Option<Node> {
        self.read(|| self.try_node(id))
    }

    pub(crate) fn try_node(&self, id: NodeId) -> Result<Option<Node>, StorageError> {
        let Some(record) = self.with_session(|session| session.reconstruct_record(id.as_u64()))?
        else {
            return Ok(None);
        };
        node_of(id, &record)
    }

    /// One property of a record, decoding only that property's occurrences.
    fn try_node_property(&self, id: NodeId, name: &str) -> Result<Option<Value>, StorageError> {
        let selected = HashSet::from([name.to_owned()]);
        let Some(record) = self
            .with_session(|session| session.reconstruct_record_selected(id.as_u64(), &selected))?
        else {
            return Ok(None);
        };
        record
            .get(name)
            .map(|value| json_to_value_checked(value).map_err(|()| StorageError::CorruptLog))
            .transpose()
    }

    /// The ordered indexes that answer equality on `property` for every
    /// label holding it, or `None`. Only types whose equality the index
    /// encodes exactly qualify (not datetimes, which Grafeo compares as
    /// strings).
    fn equality_indexes(&self, property: &str) -> Option<Vec<IndexDef>> {
        if let Some(known) = self
            .equality_indexes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(property)
        {
            return known.clone();
        }
        let found = self.read(|| {
            let norm = rushdb_types::value::normalize_name(property);
            if norm.is_empty() || rushdb_types::value::is_reserved_name(&norm) {
                return Ok(None);
            }
            let typed =
                rushdb_lmpg::index::record_property_types(self.view.as_ref(), self.tenant, &norm)?;
            if typed.iter().any(|(ty, _)| *ty == LogicalType::DateTime) {
                return Ok(None);
            }
            let mut labels = BTreeSet::new();
            for (_, pid) in &typed {
                for (label, count) in
                    access::prop_support_label_counts(self.view.as_ref(), self.tenant, *pid)?
                {
                    if count > 0 {
                        labels.insert(label);
                    }
                }
            }
            let mut defs = Vec::with_capacity(labels.len());
            for label in labels {
                match rushdb_lmpg::index::ready_ordered(
                    self.view.as_ref(),
                    self.tenant,
                    &label,
                    &norm,
                )? {
                    Some(def) => defs.push(def),
                    None => return Ok(None),
                }
            }
            Ok(Some(defs))
        });
        self.equality_indexes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(property.to_owned(), found.clone());
        found
    }

    /// Owners whose `property` may equal `value`, from its ordered indexes,
    /// or `None` when they cannot answer (no index, or a value the index
    /// does not encode). A superset: the caller checks each owner.
    fn indexed_candidates(&self, property: &str, value: &Value) -> Option<BTreeSet<u64>> {
        let defs = self.equality_indexes(property)?;
        let probes = equality_probes(value)?;
        Some(self.read(|| {
            let mut owners = BTreeSet::new();
            for (start, end) in &probes {
                for def in &defs {
                    let rows = rushdb_lmpg::index::entries(
                        self.view.as_ref(),
                        self.tenant,
                        def.id,
                        start,
                        end,
                    )?;
                    owners.extend(rows.into_iter().map(|(_, owner)| owner));
                }
            }
            Ok(owners)
        }))
    }

    /// Records matching every `conditions` equality, exactly as Grafeo
    /// compares values.
    fn matching_nodes(&self, conditions: &[(&str, Value)]) -> Vec<NodeId> {
        if conditions.iter().any(|(_, value)| value.is_null()) {
            return Vec::new();
        }
        let candidates = conditions
            .iter()
            .find_map(|(property, value)| self.indexed_candidates(property, value))
            .map_or_else(
                || self.node_ids(),
                |owners| owners.into_iter().map(NodeId::new).collect(),
            );
        candidates
            .into_iter()
            .filter(|id| {
                conditions.iter().all(|(property, expected)| {
                    self.read(|| self.try_node_property(*id, property))
                        .is_some_and(|actual| values_equal(&actual, expected))
                })
            })
            .collect()
    }
}

/// A Grafeo node from a reconstructed record document.
pub(crate) fn node_of(id: NodeId, record: &Json) -> Result<Option<Node>, StorageError> {
    let object = record.as_object().ok_or(StorageError::CorruptLog)?;
    let label = object
        .get(rushdb_types::value::LABEL_FIELD)
        .and_then(Json::as_str)
        .ok_or(StorageError::CorruptLog)?;
    let mut node = Node::with_labels(id, [label]);
    for (key, value) in object {
        if !rushdb_types::value::is_reserved_name(key) {
            node.set_property(
                key.as_str(),
                json_to_value_checked(value).map_err(|()| StorageError::CorruptLog)?,
            );
        }
    }
    Ok(Some(node))
}

impl LmpgGraphStore {
    fn edge(&self, id: EdgeId) -> Option<Edge> {
        self.read(|| self.try_edge(id))
    }

    pub(crate) fn try_edge(&self, id: EdgeId) -> Result<Option<Edge>, StorageError> {
        let Some(header) = access::relationship_by_ord(&self.view, self.tenant, id.as_u64())?
        else {
            return Ok(None);
        };
        let raw = reconstruct_relationship(&self.view, self.tenant, id.as_u64())?
            .ok_or(StorageError::CorruptLog)?;
        let mut edge = Edge::new(
            id,
            NodeId::new(header.source_ord),
            NodeId::new(header.target_ord),
            header.rel_type,
        );
        if let Some(object) = raw.as_object() {
            for (key, value) in object {
                if !rushdb_types::value::is_reserved_name(key) {
                    edge.set_property(
                        key.as_str(),
                        json_to_value_checked(value).map_err(|()| StorageError::CorruptLog)?,
                    );
                }
            }
        }
        Ok(Some(edge))
    }

    fn edge_ids(&self) -> Vec<EdgeId> {
        self.read(|| self.try_edge_ids())
    }

    pub(crate) fn try_node_ids(&self) -> Result<Vec<NodeId>, StorageError> {
        let prefix = rushdb_types::keys::record_hdr_prefix(self.tenant);
        self.view
            .range_prefix(&prefix)?
            .into_iter()
            .map(|(key, _)| {
                rushdb_types::keys::parse_record_hdr_key(&key)
                    .map(NodeId::new)
                    .ok_or(StorageError::CorruptLog)
            })
            .collect()
    }

    pub(crate) fn try_edge_ids(&self) -> Result<Vec<EdgeId>, StorageError> {
        Ok(self
            .ordinals(P_REL_BY_ORD)?
            .into_iter()
            .map(EdgeId::new)
            .collect())
    }

    fn ordinals(&self, table: &[u8]) -> Result<Vec<u64>, StorageError> {
        let prefix = key(self.tenant, &[table]);
        self.view
            .range_prefix(&prefix)?
            .into_iter()
            .map(|(key, _)| {
                if key.len() != prefix.len() + 9 || key[prefix.len()] != b'/' {
                    return Err(StorageError::CorruptLog);
                }
                let ordinal = key[prefix.len() + 1..]
                    .try_into()
                    .map_err(|_| StorageError::CorruptLog)?;
                Ok(u64::from_be_bytes(ordinal))
            })
            .collect()
    }

    pub(crate) fn property_matches_range(
        value: &Value,
        min: Option<&Value>,
        max: Option<&Value>,
        min_inclusive: bool,
        max_inclusive: bool,
    ) -> bool {
        if let Some(minimum) = min {
            match compare_values(value, minimum) {
                Some(Ordering::Equal) if !min_inclusive => return false,
                Some(Ordering::Less) | None => return false,
                _ => {}
            }
        }
        if let Some(maximum) = max {
            match compare_values(value, maximum) {
                Some(Ordering::Equal) if !max_inclusive => return false,
                Some(Ordering::Greater) | None => return false,
                _ => {}
            }
        }
        true
    }
}

impl GraphStore for LmpgGraphStore {
    fn get_node(&self, id: NodeId) -> Option<Node> {
        self.node(id)
    }

    fn get_edge(&self, id: EdgeId) -> Option<Edge> {
        self.edge(id)
    }

    fn get_node_versioned(
        &self,
        id: NodeId,
        _epoch: EpochId,
        _transaction_id: TransactionId,
    ) -> Option<Node> {
        self.node(id)
    }

    fn get_edge_versioned(
        &self,
        id: EdgeId,
        _epoch: EpochId,
        _transaction_id: TransactionId,
    ) -> Option<Edge> {
        self.edge(id)
    }

    fn get_node_at_epoch(&self, id: NodeId, _epoch: EpochId) -> Option<Node> {
        self.node(id)
    }

    fn get_edge_at_epoch(&self, id: EdgeId, _epoch: EpochId) -> Option<Edge> {
        self.edge(id)
    }

    fn get_node_property(&self, id: NodeId, key: &PropertyKey) -> Option<Value> {
        self.read(|| self.try_node_property(id, key.as_str()))
    }

    fn get_edge_property(&self, id: EdgeId, key: &PropertyKey) -> Option<Value> {
        self.edge(id)?.get_property(key.as_str()).cloned()
    }

    fn get_node_property_batch(&self, ids: &[NodeId], key: &PropertyKey) -> Vec<Option<Value>> {
        ids.iter()
            .map(|id| self.get_node_property(*id, key))
            .collect()
    }

    fn get_nodes_properties_batch(&self, ids: &[NodeId]) -> Vec<FxHashMap<PropertyKey, Value>> {
        ids.iter()
            .map(|id| {
                self.node(*id)
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
                    .filter_map(|key| {
                        self.get_node_property(*id, key)
                            .map(|value| (key.clone(), value))
                    })
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
                    .filter_map(|key| {
                        self.get_edge_property(*id, key)
                            .map(|value| (key.clone(), value))
                    })
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
        self.read(|| {
            let outgoing = || access::adj_out(&self.view, self.tenant, node.as_u64(), None);
            let incoming = || access::adj_in(&self.view, self.tenant, node.as_u64(), None);
            let mut edges = match direction {
                Direction::Outgoing => outgoing()?,
                Direction::Incoming => incoming()?,
                Direction::Both => {
                    let mut result = outgoing()?;
                    result.extend(incoming()?);
                    result
                }
                _ => Vec::new(),
            }
            .into_iter()
            .map(|(_, other, edge)| (NodeId::new(other), EdgeId::new(edge)))
            .collect::<Vec<_>>();
            edges.sort_unstable();
            edges.dedup();
            Ok(edges)
        })
    }

    fn out_degree(&self, node: NodeId) -> usize {
        self.read(|| Ok(access::adj_out(&self.view, self.tenant, node.as_u64(), None)?.len()))
    }

    fn in_degree(&self, node: NodeId) -> usize {
        self.read(|| Ok(access::adj_in(&self.view, self.tenant, node.as_u64(), None)?.len()))
    }

    fn has_backward_adjacency(&self) -> bool {
        true
    }

    fn node_ids(&self) -> Vec<NodeId> {
        self.read(|| self.try_node_ids())
    }

    fn nodes_by_label(&self, label: &str) -> Vec<NodeId> {
        self.read(|| {
            Ok(access::label_members(&self.view, self.tenant, label)?
                .into_iter()
                .map(NodeId::new)
                .collect())
        })
    }

    // Counts come from the on-disk catalogs at the view's snapshot (one
    // counter row per label / relationship group), never an id walk.
    fn node_count(&self) -> usize {
        usize::try_from(self.catalog_totals().0).unwrap_or(usize::MAX)
    }

    fn nodes_by_label_count(&self, label: &str) -> usize {
        self.read(|| rushdb_lmpg::derived::label_count(&self.view, self.tenant, label))
            .try_into()
            .unwrap_or(usize::MAX)
    }

    fn edge_count(&self) -> usize {
        usize::try_from(self.catalog_totals().1).unwrap_or(usize::MAX)
    }

    fn edge_type(&self, id: EdgeId) -> Option<grafeo_common::types::ArcStr> {
        self.read(|| {
            Ok(
                access::relationship_by_ord(&self.view, self.tenant, id.as_u64())?
                    .map(|header| header.rel_type.into()),
            )
        })
    }

    fn has_property_index(&self, property: &str) -> bool {
        self.equality_indexes(property).is_some()
    }

    fn find_nodes_by_property(&self, property: &str, value: &Value) -> Vec<NodeId> {
        self.matching_nodes(&[(property, value.clone())])
    }

    fn find_nodes_by_properties(&self, conditions: &[(&str, Value)]) -> Vec<NodeId> {
        self.matching_nodes(conditions)
    }

    fn find_nodes_in_range(
        &self,
        property: &str,
        min: Option<&Value>,
        max: Option<&Value>,
        min_inclusive: bool,
        max_inclusive: bool,
    ) -> Vec<NodeId> {
        let key = PropertyKey::new(property);
        self.node_ids()
            .into_iter()
            .filter(|id| {
                self.get_node_property(*id, &key).is_some_and(|value| {
                    Self::property_matches_range(&value, min, max, min_inclusive, max_inclusive)
                })
            })
            .collect()
    }

    fn node_property_might_match(
        &self,
        _property: &PropertyKey,
        _op: CompareOp,
        _value: &Value,
    ) -> bool {
        true
    }

    fn edge_property_might_match(
        &self,
        _property: &PropertyKey,
        _op: CompareOp,
        _value: &Value,
    ) -> bool {
        true
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "planner statistics are approximate average degrees"
    )]
    fn statistics(&self) -> Arc<Statistics> {
        let labels = self.read(|| access::label_counts(&self.view, self.tenant));
        let rels = self.read(|| access::relationship_catalog(&self.view, self.tenant));
        let total_nodes: u64 = labels.iter().map(|(_, count)| count).sum();
        let mut statistics = Statistics {
            total_nodes,
            total_edges: rels.iter().map(|(_, _, _, count)| count).sum(),
            ..Statistics::default()
        };
        for (label, count) in labels {
            statistics
                .labels
                .insert(label, grafeo_core::statistics::LabelStatistics::new(count));
        }
        let mut by_type: std::collections::BTreeMap<String, u64> =
            std::collections::BTreeMap::new();
        for (rel_type, _, _, count) in rels {
            *by_type.entry(rel_type).or_default() += count;
        }
        for (rel_type, count) in by_type {
            let degree = if total_nodes == 0 {
                0.0
            } else {
                count as f64 / total_nodes as f64
            };
            statistics.edge_types.insert(
                rel_type,
                grafeo_core::statistics::EdgeTypeStatistics::new(count, degree, degree),
            );
        }
        Arc::new(statistics)
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "planner statistics are approximate cardinalities, not exact counts"
    )]
    fn estimate_label_cardinality(&self, label: &str) -> f64 {
        self.nodes_by_label_count(label) as f64
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "planner statistics are approximate average degrees, not exact counts"
    )]
    fn estimate_avg_degree(&self, edge_type: &str, _outgoing: bool) -> f64 {
        // Every edge has one source and one target, so the average out- and
        // in-degree over all nodes are both `edges of type / nodes`.
        let nodes = self.catalog_totals().0;
        if nodes == 0 {
            return 0.0;
        }
        let edges: u64 = self
            .read(|| access::relationship_catalog(&self.view, self.tenant))
            .iter()
            .filter(|(rel_type, _, _, _)| rel_type == edge_type)
            .map(|(_, _, _, count)| count)
            .sum();
        edges as f64 / nodes as f64
    }

    fn current_epoch(&self) -> EpochId {
        EpochId::new(self.view.commit_index())
    }

    fn all_labels(&self) -> Vec<String> {
        self.read(|| access::labels(&self.view, self.tenant))
    }

    fn all_edge_types(&self) -> Vec<String> {
        self.read(|| access::relationship_catalog(&self.view, self.tenant))
            .into_iter()
            .map(|(rel_type, _, _, _)| rel_type)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn all_property_keys(&self) -> Vec<String> {
        let mut keys = BTreeSet::new();
        for id in self.node_ids() {
            if let Some(node) = self.node(id) {
                keys.extend(node.properties.iter().map(|(key, _)| key.to_string()));
            }
        }
        for id in self.edge_ids() {
            if let Some(edge) = self.edge(id) {
                keys.extend(edge.properties.iter().map(|(key, _)| key.to_string()));
            }
        }
        keys.into_iter().collect()
    }
}

impl GraphStoreSearch for LmpgGraphStore {
    fn has_text_index(&self, label: &str, property: &str) -> bool {
        self.text_index(label, property).is_some()
    }

    /// Per-row scoring, used when the planner cannot push the search into
    /// a scan. A property no ready text index covers fails the statement
    /// rather than matching nothing.
    fn score_text(&self, node_id: NodeId, label: &str, property: &str, query: &str) -> Option<f64> {
        let Some(index) = self.text_index(label, property) else {
            self.text_failed(if self.text.is_some() {
                format!("no ready text index on :{label}({property})")
            } else {
                "text search is unavailable here".to_owned()
            });
            return None;
        };
        Some(
            self.text_matches(&index, query)
                .get(&node_id.as_u64())
                .copied()
                .unwrap_or(0.0),
        )
    }

    fn text_search(
        &self,
        label: &str,
        property: &str,
        query: &str,
        k: usize,
    ) -> Vec<(NodeId, f64)> {
        let Some(index) = self.text_index(label, property) else {
            return Vec::new();
        };
        self.text_hits(&index, query, k)
            .into_iter()
            .map(|(owner, score)| (NodeId::new(owner), score))
            .collect()
    }

    fn text_search_with_threshold(
        &self,
        label: &str,
        property: &str,
        query: &str,
        threshold: f64,
    ) -> Vec<(NodeId, f64)> {
        let Some(index) = self.text_index(label, property) else {
            return Vec::new();
        };
        self.text_hits(&index, query, rushdb_textsearch::ALL_HITS)
            .into_iter()
            .filter(|(_, score)| *score >= threshold)
            .map(|(owner, score)| (NodeId::new(owner), score))
            .collect()
    }
}

/// Index entry ranges `[start, end)` holding every stored value Grafeo
/// considers equal to `value` (see [`values_equal`]), or `None` for a value
/// the index does not encode exactly (lists, maps, temporals, numbers
/// beyond 2^53, where integers and floats stop mapping one to one).
fn equality_probes(value: &Value) -> Option<Vec<(Vec<u8>, Vec<u8>)>> {
    const EXACT: f64 = 9_007_199_254_740_992.0; // 2^53
    let point = |ty: LogicalType, scalar: &Scalar| -> Option<(Vec<u8>, Vec<u8>)> {
        let entry = entry_of(ty, scalar)?;
        let end = rushdb_storage::prefix_range(&entry).1;
        Some((entry, end))
    };
    // Floats within `f64::EPSILON` of `center`, both ends inclusive.
    let floats_near = |center: f64| -> Option<(Vec<u8>, Vec<u8>)> {
        let low = entry_of(
            LogicalType::Float64,
            &Scalar::Float64(center - f64::EPSILON),
        )?;
        let high = entry_of(
            LogicalType::Float64,
            &Scalar::Float64(center + f64::EPSILON),
        )?;
        Some((low, rushdb_storage::prefix_range(&high).1))
    };
    match value {
        Value::Bool(b) => Some(vec![point(LogicalType::Boolean, &Scalar::Boolean(*b))?]),
        Value::String(s) => Some(vec![point(
            LogicalType::Utf8,
            &Scalar::Utf8(s.to_string()),
        )?]),
        Value::Int64(i) => {
            #[expect(clippy::cast_precision_loss, reason = "bounded by 2^53 below")]
            let f = *i as f64;
            if f.abs() >= EXACT {
                return None;
            }
            Some(vec![
                point(LogicalType::Int64, &Scalar::Int64(*i))?,
                floats_near(f)?,
            ])
        }
        Value::Float64(f) if f.is_finite() && f.abs() < EXACT => {
            let mut probes = vec![floats_near(*f)?];
            let nearest = f.round();
            if (nearest - f).abs() < f64::EPSILON {
                #[expect(clippy::cast_possible_truncation, reason = "bounded by 2^53 above")]
                let i = nearest as i64;
                probes.push(point(LogicalType::Int64, &Scalar::Int64(i))?);
            }
            Some(probes)
        }
        _ => None,
    }
}

/// `tag ‖ order-preserving payload`, the ordered index entry of a scalar.
fn entry_of(ty: LogicalType, scalar: &Scalar) -> Option<Vec<u8>> {
    let mut entry = vec![rushdb_types::encoding::logical_tag(ty)];
    entry.extend(rushdb_types::encoding::encode_order_preserving(ty, scalar).ok()?);
    Some(entry)
}

/// Grafeo's equality for property lookups: an integer and a float are equal
/// within `f64::EPSILON`; everything else compares exactly.
pub(crate) fn values_equal(left: &Value, right: &Value) -> bool {
    #[expect(clippy::cast_precision_loss, reason = "Grafeo's own coercion")]
    match (left, right) {
        (Value::Int64(a), Value::Float64(b)) | (Value::Float64(b), Value::Int64(a)) => {
            (*a as f64 - b).abs() < f64::EPSILON
        }
        _ => left == right,
    }
}

pub(crate) fn compare_values(left: &Value, right: &Value) -> Option<Ordering> {
    match (left, right) {
        (Value::Int64(left), Value::Int64(right)) => {
            scalar_cmp(&Scalar::Int64(*left), &Scalar::Int64(*right))
        }
        (Value::Float64(left), Value::Float64(right)) => {
            scalar_cmp(&Scalar::Float64(*left), &Scalar::Float64(*right))
        }
        (Value::Int64(left), Value::Float64(right)) => {
            scalar_cmp(&Scalar::Int64(*left), &Scalar::Float64(*right))
        }
        (Value::Float64(left), Value::Int64(right)) => {
            scalar_cmp(&Scalar::Float64(*left), &Scalar::Int64(*right))
        }
        (Value::String(left), Value::String(right)) => Some(left.cmp(right)),
        (Value::Bool(left), Value::Bool(right)) => Some(left.cmp(right)),
        (Value::Timestamp(left), Value::Timestamp(right)) => Some(left.cmp(right)),
        (Value::Date(left), Value::Date(right)) => Some(left.cmp(right)),
        (Value::Time(left), Value::Time(right)) => Some(left.cmp(right)),
        _ => None,
    }
}
