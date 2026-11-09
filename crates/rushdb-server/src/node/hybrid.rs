//! Hybrid search: one candidate set (a predicate's matches, or a label's
//! members) ranked by a vector leg, a text leg or both, fused by reciprocal
//! rank.

use super::{access, append_bounded_json_response, Arc, OwnerKind, RequestScope};

/// Why a hybrid search did not run.
#[derive(Debug)]
pub(crate) enum HybridError {
    /// The enforced policy denies a label the query names.
    Policy,
    /// The text leg's typed failure.
    Text(rushdb_textsearch::TextSearchError),
    /// Anything else, as the engine phrased it.
    Other(String),
}

impl From<String> for HybridError {
    fn from(message: String) -> Self {
        Self::Other(message)
    }
}

impl From<&str> for HybridError {
    fn from(message: &str) -> Self {
        Self::Other(message.to_owned())
    }
}

impl From<rushdb_textsearch::TextSearchError> for HybridError {
    fn from(error: rushdb_textsearch::TextSearchError) -> Self {
        Self::Text(error)
    }
}

impl std::fmt::Display for HybridError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Policy => f.write_str("hybrid query denied by policy"),
            Self::Text(error) => error.fmt(f),
            Self::Other(message) => f.write_str(message),
        }
    }
}

impl RequestScope<'_> {
    /// Execute [`Self::hybrid_search_in_view`] against a caller-owned
    /// execution context's pinned view and remaining response-byte budget.
    ///
    /// # Errors
    ///
    /// Returns why the search did not run.
    pub(crate) fn hybrid_search_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        query: &rushdb_protocol::HybridQuery,
    ) -> Result<serde_json::Value, HybridError> {
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        let result = self.hybrid_search_in_view(query, context.read_view())?;
        context
            .check_deadline()
            .map_err(|error| error.to_string())?;
        Ok(result)
    }

    /// Combined graph + vector + lexical query (document 26's hybrid
    /// pattern), against a caller-owned pinned view and work budget.
    ///
    /// A `predicate` is resolved exactly once, via the same cardinality-based
    /// access-path planner `find_records` uses (`matching_ordinals`), into
    /// one candidate owner-ordinal set. Both legs search only inside that
    /// set — candidate resolution happens before ranking for both the ANN
    /// graph and the lexical index, never as a post-filter that could leak a
    /// hidden owner into a fused top-k slot. Without a predicate the legs
    /// check label membership on their hits instead (see
    /// [`Self::hybrid_text_leg`]), so an unfiltered query does not read the
    /// whole label. The legs run concurrently, and their two independently
    /// correct rankings are then combined by deterministic reciprocal-rank
    /// fusion; raw scores never cross providers.
    ///
    /// Every fused hit is reconstructed through an
    /// [`rushdb_lmpg::AuthorizedView`] before being emitted — the same
    /// policy gate [`Self::search_vectors_in_view`] applies, and a stronger
    /// one than the bare lexical path had, so this is not added to
    /// `protocol.rs`'s `PolicyEnforced` fail-closed list.
    ///
    /// # Errors
    ///
    /// Returns an error message when the operation fails, including when
    /// neither leg is set, a named vector index/text profile is unknown, or
    /// `PolicyEnforced` denies every requested label.
    #[expect(
        clippy::too_many_lines,
        reason = "one cohesive candidate-resolution -> dual-leg search -> fusion -> \
                  reconstruction pipeline, the same shape as search_vectors_in_view; \
                  splitting it would scatter the shared candidate/budget borrows"
    )]
    pub(super) fn hybrid_search_in_view(
        &self,
        query: &rushdb_protocol::HybridQuery,
        snap: &Arc<rushdb_storage::ReadView>,
    ) -> Result<serde_json::Value, HybridError> {
        if query.vector.is_none() && query.text.is_none() {
            return Err("hybrid query requires at least one of `vector`/`text`".into());
        }
        validate(query)?;
        if let Some(active) = rushdb_authz::load_active_policy_in_view(snap, self.tenant())
            .map_err(|error| error.to_string())?
        {
            if active.mode == rushdb_authz::AuthorizationMode::PolicyEnforced
                && !(if query.labels.is_empty() {
                    active.policy.allows_every_label()
                } else {
                    query
                        .labels
                        .iter()
                        .all(|label| active.policy.allows_label(label))
                })
            {
                return Err(HybridError::Policy);
            }
        }
        let limit = usize::try_from(query.limit).unwrap_or(usize::MAX).max(1);
        let offset = usize::try_from(query.offset).unwrap_or(usize::MAX);
        let lmpg = self.lmpg();

        // A predicate is resolved once into the owner set both legs search
        // inside. Without one, neither leg needs it: the vector leg takes the
        // unfiltered route of a plain vector search (label membership checked
        // on the hits it keeps), and the text leg searches its index whole and
        // checks its hits' label — so an unfiltered query does not grow with
        // the label.
        let unfiltered = query.predicate.is_null()
            || query
                .predicate
                .as_object()
                .is_some_and(serde_json::Map::is_empty);
        let resolve_owners = || -> Result<Arc<roaring::RoaringTreemap>, HybridError> {
            let mut filter_document = serde_json::json!({ "labels": query.labels });
            if !unfiltered {
                filter_document["where"] = query.predicate.clone();
            }
            let parsed = rushdb_query::document::parse_document(&filter_document)
                .map_err(|e| e.to_string())?;
            // Ordinals straight from the cardinality planner, ascending, not
            // from reconstructing and re-decoding records.
            let ords =
                rushdb_query::matching_ordinals(&lmpg, &parsed.search, &|| false, Arc::clone(snap))
                    .map_err(|e| e.to_string())?;
            Ok(Arc::new(ords.into_iter().collect()))
        };
        let owners = if unfiltered {
            None
        } else {
            Some(resolve_owners()?)
        };

        // Over-fetch before fusion/truncation so RRF has enough of each
        // ranking to actually combine rather than just echoing whichever leg
        // happened to return first. A tunable, not a correctness bound: RRF
        // itself is well-defined for any k. Sized from `offset + limit`, not
        // bare `limit`, so a deep page's over-fetch still covers the whole
        // page instead of only ever gathering enough for page one.
        let page_end = offset.saturating_add(limit);
        let default_prefetch = page_end.saturating_mul(4).max(page_end);
        let prefetch = |limit: Option<u64>| {
            limit.map_or(default_prefetch, |n| {
                usize::try_from(n).unwrap_or(usize::MAX)
            })
        };

        // The two legs run concurrently on the shared thread budget (one
        // after the other when it lends no thread); each leg's result does
        // not depend on the other.
        let legs: Vec<LegKind> = [
            query.vector.as_ref().map(|_| LegKind::Vector),
            query.text.as_ref().map(|_| LegKind::Text),
        ]
        .into_iter()
        .flatten()
        .collect();
        let mut vector_leg = None;
        let mut text_leg = None;
        for out in rushdb_storage::parallel::run_parts(legs.len(), |part| match legs[part] {
            LegKind::Vector => self
                .hybrid_vector_leg(query, snap, owners.as_ref(), &prefetch)
                .map(LegOut::Vector),
            LegKind::Text => self
                .hybrid_text_leg(query, snap, owners.as_ref(), &prefetch, &resolve_owners)
                .map(LegOut::Text),
        })? {
            match out {
                LegOut::Vector(found) => vector_leg = Some(found),
                LegOut::Text(found) => text_leg = Some(found),
            }
        }
        let (vec_hits, vector_report) = match vector_leg {
            Some((hits, report)) => (hits, Some(report)),
            None => (Vec::new(), None),
        };
        let (lex_hits, text_report) = match text_leg {
            Some(found) => (
                found.hits,
                Some(serde_json::json!({
                    "matches": found.matches,
                    "pending": found.pending,
                })),
            ),
            None => (Vec::new(), None),
        };
        // Each leg's own score per record; one leg alone ranks by its score,
        // since a fused rank score says nothing on its own.
        let vector_scores: std::collections::HashMap<u64, f32> = vec_hits
            .iter()
            .map(|hit| (hit.source.owner_ord, hit.score))
            .collect();
        let text_scores: std::collections::HashMap<u64, f64> = lex_hits
            .iter()
            .map(|hit| (hit.owner_ord, hit.score))
            .collect();
        let both_legs = query.vector.is_some() && query.text.is_some();

        let vector_ranked: Vec<(u64, f64)> = vec_hits
            .iter()
            .map(|hit| (hit.source.owner_ord, f64::from(hit.score)))
            .collect();
        let text_ranked: Vec<(u64, f64)> = lex_hits
            .iter()
            .map(|hit| (hit.owner_ord, hit.score))
            .collect();
        // A weight is a leg's share of a fused score; one leg alone ranks by
        // its own order whatever its weight.
        let weight = |weight: f64| if both_legs { weight } else { 1.0 };
        let mut legs = Vec::with_capacity(2);
        if let Some(leg) = &query.vector {
            legs.push(super::fusion::Leg {
                name: "vector",
                weight: weight(leg.weight),
                hits: &vector_ranked,
            });
        }
        if let Some(leg) = &query.text {
            legs.push(super::fusion::Leg {
                name: "lexical",
                weight: weight(leg.weight),
                hits: &text_ranked,
            });
        }
        let fused = super::fusion::fuse(&legs, query.fusion, query.rrf_k, page_end);

        let mut data = Vec::new();
        let mut encoded_bytes = 0usize;
        let proptypes = self.proptypes_cached(snap)?;
        let active = rushdb_authz::load_active_policy_in_view(snap.as_ref(), self.tenant())
            .map_err(|error| error.to_string())?;
        let authz = rushdb_lmpg::AuthorizedView::new(snap.as_ref(), self.tenant(), active.as_ref());
        for hit in fused.into_iter().skip(offset) {
            let Some(hdr) = access::record_by_ord(snap, self.tenant(), hit.owner_ord)
                .map_err(|e| e.to_string())?
            else {
                continue;
            };
            let Some(rec) = authz
                .reconstruct_record(hdr.ordinal)
                .map_err(|e| e.to_string())?
            else {
                continue;
            };
            let rec = Self::canonical_record_with(&rec, &proptypes);
            let mut obj = rec.as_object().cloned().unwrap_or_default();
            let vector_score = vector_scores.get(&hit.owner_ord).copied();
            let text_score = text_scores.get(&hit.owner_ord).copied();
            let score = if both_legs {
                hit.score
            } else {
                text_score
                    .or(vector_score.map(f64::from))
                    .unwrap_or(hit.score)
            };
            obj.insert("score".to_string(), serde_json::json!(score));
            if let Some(vector_score) = vector_score {
                obj.insert("vectorScore".to_string(), serde_json::json!(vector_score));
            }
            if let Some(text_score) = text_score {
                obj.insert("textScore".to_string(), serde_json::json!(text_score));
            }
            obj.insert(
                "provenance".to_string(),
                serde_json::json!(hit
                    .provenance
                    .iter()
                    .map(|(p, r)| format!("{p}:{r}"))
                    .collect::<Vec<_>>()),
            );
            append_bounded_json_response(
                &mut data,
                &mut encoded_bytes,
                serde_json::Value::Object(obj),
                self.max_response_bytes(),
            )?;
        }
        let mut result = serde_json::json!({ "data": data });
        if let Some(report) = vector_report {
            result["vectorSearch"] = report;
        }
        if let Some(report) = text_report {
            result["textSearch"] = report;
        }
        if both_legs {
            result["fusion"] = serde_json::to_value(query.fusion).map_err(|e| e.to_string())?;
        }
        Ok(result)
    }
}

/// The legs of a hybrid query.
#[derive(Clone, Copy)]
enum LegKind {
    Vector,
    Text,
}

/// One leg's result.
enum LegOut {
    Vector((Vec<rushdb_vector::ScoredSource>, serde_json::Value)),
    Text(rushdb_textsearch::TextHits),
}

impl RequestScope<'_> {
    /// The vector leg: inside `owners` when the query has a predicate, else
    /// the unfiltered route of [`Self::search_vectors_in_view`].
    fn hybrid_vector_leg(
        &self,
        query: &rushdb_protocol::HybridQuery,
        snap: &Arc<rushdb_storage::ReadView>,
        owners: Option<&Arc<roaring::RoaringTreemap>>,
        prefetch: &(dyn Fn(Option<u64>) -> usize + Sync),
    ) -> Result<(Vec<rushdb_vector::ScoredSource>, serde_json::Value), HybridError> {
        let Some(leg) = &query.vector else {
            return Ok((Vec::new(), serde_json::Value::Null));
        };
        if leg.vector.is_empty() {
            return Err("hybrid vector leg requires a non-empty vector".into());
        }
        let label = query.labels.first().cloned().unwrap_or_default();
        let alias = match leg.index.as_deref() {
            Some(index) => index.to_string(),
            None if self.require_vector_index_name => {
                return Err("hybrid vector leg requires an explicit `index` \
                     (resources.require-vector-index-name is on)"
                    .into());
            }
            None => rushdb_lmpg::import::external_space_alias(
                &label,
                &leg.property,
                &leg.similarity,
                leg.vector.len(),
            ),
        };
        let vectors = self.vector_engine()?;
        let space = vectors
            .lookup_space_in_view(snap, &alias)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| {
                format!(
                    "embedding index not found for label `{label}` and property `{}`",
                    leg.property
                )
            })?;
        let member =
            super::vectors::MemberCheck::new(snap, self.tenant(), OwnerKind::Record, &query.labels);
        let check = |source| member.admits(source);
        let candidates = match owners {
            Some(ords) => rushdb_vector::Candidates::Owners {
                kind: OwnerKind::Record,
                ords,
            },
            None => rushdb_vector::Candidates::Check(&check),
        };
        let found = self.run_vector_search(
            &vectors,
            snap,
            &space,
            &super::vector_mode::VectorSearch {
                query: &leg.vector,
                k: prefetch(leg.prefetch_limit),
                mode: leg.mode,
                ef: leg.ef,
                min_score: leg.score_threshold,
                candidates,
            },
        )?;
        member.finish()?;
        Ok(found)
    }

    /// The text leg: inside `owners` when the query has a predicate. Without
    /// one, the index is searched whole when it covers a requested label (or
    /// any label is accepted), and a hit whose record is no longer a member
    /// of the index's label sends the leg back to the owner set — so the
    /// result is the owner set's either way.
    fn hybrid_text_leg(
        &self,
        query: &rushdb_protocol::HybridQuery,
        snap: &Arc<rushdb_storage::ReadView>,
        owners: Option<&Arc<roaring::RoaringTreemap>>,
        prefetch: &(dyn Fn(Option<u64>) -> usize + Sync),
        resolve_owners: &(dyn Fn() -> Result<Arc<roaring::RoaringTreemap>, HybridError> + Sync),
    ) -> Result<rushdb_textsearch::TextHits, HybridError> {
        let Some(leg) = &query.text else {
            return Ok(rushdb_textsearch::TextHits {
                hits: Vec::new(),
                matches: 0,
                pending: 0,
            });
        };
        let search = |candidates: Option<Arc<roaring::RoaringTreemap>>| {
            self.text.search(
                self.tenant(),
                snap,
                rushdb_textsearch::TextQuery {
                    index: &leg.index,
                    query: &leg.query,
                    opts: crate::protocol::queries::lexical_options(
                        leg.all_terms,
                        leg.phrase,
                        leg.syntax,
                    ),
                    top_k: prefetch(leg.prefetch_limit),
                },
                candidates,
            )
        };
        if let Some(owners) = owners {
            return Ok(search(Some(Arc::clone(owners)))?);
        }
        let covered_label = rushdb_lmpg::index::by_name(snap.as_ref(), self.tenant(), &leg.index)
            .map_err(|e| e.to_string())?
            .map(|def| def.label)
            .filter(|label| query.labels.is_empty() || query.labels.contains(label));
        let Some(label) = covered_label else {
            // Another label's index (or none: the search reports it).
            return Ok(search(Some(resolve_owners()?))?);
        };
        let found = search(None)?;
        let mut members = true;
        for hit in &found.hits {
            let key = rushdb_types::keys::label_member_key(self.tenant(), &label, hit.owner_ord);
            if snap.get(&key).map_err(|e| e.to_string())?.is_none() {
                members = false;
                break;
            }
        }
        if members {
            Ok(found)
        } else {
            Ok(search(Some(resolve_owners()?))?)
        }
    }
}

/// Candidates a leg may rank before fusion, at most.
const MAX_PREFETCH_LIMIT: u64 = 10_000;

/// The fusion parameters a hybrid query may use.
fn validate(query: &rushdb_protocol::HybridQuery) -> Result<(), String> {
    let legs = [
        query
            .vector
            .as_ref()
            .map(|leg| ("vector", leg.weight, leg.prefetch_limit)),
        query
            .text
            .as_ref()
            .map(|leg| ("text", leg.weight, leg.prefetch_limit)),
    ];
    for (name, weight, prefetch_limit) in legs.into_iter().flatten() {
        if !weight.is_finite() || weight < 0.0 {
            return Err(format!(
                "invalid hybrid {name} leg: `weight` must be a finite number ≥ 0"
            ));
        }
        if prefetch_limit.is_some_and(|n| n == 0 || n > MAX_PREFETCH_LIMIT) {
            return Err(format!(
                "invalid hybrid {name} leg: `prefetchLimit` must be between 1 and {MAX_PREFETCH_LIMIT}"
            ));
        }
    }
    if let (Some(vector), Some(text)) = (&query.vector, &query.text) {
        if vector.weight == 0.0 && text.weight == 0.0 {
            return Err("invalid hybrid query: at least one leg `weight` must be above 0".into());
        }
    }
    if !query.rrf_k.is_finite() || query.rrf_k < 0.0 {
        return Err("invalid hybrid query: `rrfK` must be a finite number ≥ 0".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rushdb_protocol::{
        Operation, Query, RequestContext, RequestEnvelope, Response, PROTOCOL_VERSION,
    };

    use crate::protocol::ProtocolExecutor;
    use crate::{Node, ServerConfig};

    /// A node whose `auto` vector searches use the ANN graph past 4 vectors,
    /// with a 2-dimension vector index `emb` and a whole-record text index
    /// `docs` (of `consistency`) on `Doc`.
    fn node(consistency: &str) -> Node {
        let mut cfg = ServerConfig::default();
        cfg.resources.vector_exact_max_pool = 4;
        let node = Node::bootstrap(&mut cfg).unwrap();
        node.scope()
            .create_vector_index(&rushdb_protocol::VectorIndexDefinition {
                label: "Doc".into(),
                property: "embedding".into(),
                similarity: "cosine".into(),
                dimensions: 2,
                name: Some("emb".into()),
                model: None,
                owner: None,
            })
            .unwrap();
        node.scope()
            .create_index(&rushdb_protocol::IndexDefinition {
                name: "docs".into(),
                label: "Doc".into(),
                property: "*".into(),
                kind: "text".into(),
                consistency: Some(consistency.into()),
                dimensions: None,
            })
            .unwrap();
        while node.step_index_builds(std::time::Duration::from_secs(5)) > 0 {}
        node
    }

    /// `count` `Doc` records around the unit circle in a fixed scattered
    /// order, every one matching the text `alpha`; returns their ids.
    #[expect(
        clippy::cast_precision_loss,
        reason = "a small record index mapped into f32 radians"
    )]
    fn fill(node: &Node, count: usize) -> Vec<String> {
        (0..count)
            .map(|i| {
                let angle = ((i * 2_654_435_761) % count) as f32 * 6.2 / count as f32;
                let words = if i % 3 == 0 {
                    "alpha alpha beta"
                } else {
                    "alpha"
                };
                node.scope()
                    .create_record(
                        "Doc",
                        &serde_json::json!({ "body": words, "kind": "doc" }),
                        &[serde_json::json!({
                            "propertyName": "embedding",
                            "vector": [angle.cos(), angle.sin()],
                            "index": "emb",
                        })],
                        &serde_json::json!({}),
                    )
                    .unwrap()["id"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    fn hybrid(
        node: &Node,
        query: serde_json::Value,
    ) -> (Vec<serde_json::Value>, serde_json::Value) {
        let query = serde_json::from_value(query).unwrap();
        let response = ProtocolExecutor::new(node, None)
            .execute(RequestEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: None,
                context: RequestContext::default(),
                operation: Operation::Query {
                    query: Query::Hybrid { query },
                },
            })
            .unwrap()
            .response;
        let Response::Records {
            records,
            vector_search,
            text_search,
            ..
        } = response
        else {
            panic!("expected records, got {response:?}")
        };
        (
            records,
            serde_json::json!({
                "vector": vector_search,
                "text": text_search,
            }),
        )
    }

    fn both_legs(predicate: &serde_json::Value, mode: &str) -> serde_json::Value {
        serde_json::json!({
            "labels": ["Doc"],
            "predicate": predicate,
            "limit": 5,
            "vector": {
                "property": "embedding",
                "vector": [1.0, 0.0],
                "index": "emb",
                "mode": mode,
            },
            "text": { "index": "docs", "query": "alpha" },
        })
    }

    /// Run an unfiltered hybrid query until its vector leg answers on the
    /// resident ANN graph.
    fn warm(node: &Node) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while hybrid(node, both_legs(&serde_json::json!({}), "auto")).1["vector"]["mode"] != "ann" {
            assert!(
                std::time::Instant::now() < deadline,
                "the graph never warmed"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// Without a predicate neither leg resolves the label: the same query
    /// reads the same rows over 10x the records.
    #[test]
    fn an_unfiltered_hybrid_query_reads_the_same_at_n_and_ten_n_records() {
        let reads = |count: usize| {
            let node = node("immediate");
            fill(&node, count);
            warm(&node);
            // Applying the text index deleted its pending rows; collect them
            // so the scan for new ones does not walk their history.
            node.store().gc(u64::MAX).unwrap();
            rushdb_storage::read_accounting::enable(true);
            let _ = rushdb_storage::read_accounting::take_local();
            let (records, report) = hybrid(&node, both_legs(&serde_json::json!({}), "ann"));
            let reads: u64 = rushdb_storage::read_accounting::take_local().values().sum();
            assert_eq!(records.len(), 5);
            assert_eq!(report["vector"]["mode"], "ann");
            reads
        };
        let (small, large) = (reads(300), reads(3_000));
        assert!(small > 0);
        assert_eq!(small, large, "reads grew with the label");
    }

    /// The unfiltered route ranks exactly as the owner set of every label
    /// member does (a predicate every record matches), on both paths.
    #[test]
    fn an_unfiltered_hybrid_query_ranks_as_the_owner_set_does() {
        let node = node("immediate");
        fill(&node, 400);
        warm(&node);
        for mode in ["exact", "ann"] {
            let (unfiltered, _) = hybrid(&node, both_legs(&serde_json::json!({}), mode));
            let (owner_set, _) = hybrid(
                &node,
                both_legs(&serde_json::json!({ "kind": "doc" }), mode),
            );
            assert_eq!(unfiltered.len(), 5);
            assert_eq!(unfiltered, owner_set, "{mode}");
        }
    }

    /// A text hit whose record left the label (deleted, not yet applied to
    /// an `eventual` index) sends the unfiltered text leg back to the owner
    /// set: the page is still full and never holds the deleted record.
    #[test]
    fn a_stale_text_hit_falls_back_to_the_owner_set() {
        let node = node("eventual");
        let ids = fill(&node, 30);
        // Apply the inserts, then delete a top text match.
        node.apply_text_indexes(std::time::Duration::from_secs(5));
        node.scope().delete_record(&ids[0]).unwrap();
        let text_only = |predicate: serde_json::Value| {
            hybrid(
                &node,
                serde_json::json!({
                    "labels": ["Doc"],
                    "predicate": predicate,
                    "limit": 3,
                    "text": { "index": "docs", "query": "beta", "prefetchLimit": 3 },
                }),
            )
        };
        let (unfiltered, report) = text_only(serde_json::json!({}));
        let (owner_set, _) = text_only(serde_json::json!({ "kind": "doc" }));
        assert!(report["text"]["pending"].as_u64().unwrap() > 0, "{report}");
        assert_eq!(unfiltered.len(), 3);
        assert!(unfiltered
            .iter()
            .all(|record| record["__id"] != ids[0].as_str()));
        assert_eq!(unfiltered, owner_set);
    }
}
