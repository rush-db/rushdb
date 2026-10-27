//! Hybrid search: one predicate-resolved candidate set ranked by a vector
//! leg, a text leg or both, fused by reciprocal rank.

use super::{access, append_bounded_json_response, Arc, HashSet, OwnerKind, RequestScope};

impl RequestScope<'_> {
    /// Execute [`Self::hybrid_search_in_view`] against a caller-owned
    /// execution context's pinned view and remaining response-byte budget.
    ///
    /// # Errors
    ///
    /// Returns an error message when the operation fails.
    pub(crate) fn hybrid_search_with_execution_context(
        &self,
        context: &mut rushdb_engine::ExecutionContext,
        query: &rushdb_protocol::HybridQuery,
    ) -> Result<serde_json::Value, String> {
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
    /// The `labels`/`predicate` graph filter is resolved exactly once, via
    /// the same cardinality-based access-path planner `find_records` uses
    /// (`matching_ordinals_with_budget`), into one candidate owner-ordinal
    /// set. Both legs search only inside that set — candidate resolution
    /// happens before ranking for both the ANN graph and the lexical index,
    /// never as a post-filter that could leak a hidden owner into a fused
    /// top-k slot. The two independently correct rankings are then combined
    /// by deterministic reciprocal-rank fusion; raw scores never cross
    /// providers.
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
    ) -> Result<serde_json::Value, String> {
        if query.vector.is_none() && query.text.is_none() {
            return Err("hybrid query requires at least one of `vector`/`text`".to_string());
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
                return Err("hybrid query denied by policy".into());
            }
        }
        let limit = usize::try_from(query.limit).unwrap_or(usize::MAX).max(1);
        let offset = usize::try_from(query.offset).unwrap_or(usize::MAX);
        let lmpg = self.lmpg();

        // One predicate resolution shared by both legs. Ordinals come
        // straight from the cardinality planner, not from reconstructing and
        // re-decoding records the way a naive composition would.
        let mut filter_document = serde_json::json!({ "labels": query.labels });
        if !query.predicate.is_null() {
            filter_document["where"] = query.predicate.clone();
        }
        let parsed =
            rushdb_query::document::parse_document(&filter_document).map_err(|e| e.to_string())?;
        let ords =
            rushdb_query::matching_ordinals(&lmpg, &parsed.search, &|| false, Arc::clone(snap))
                .map_err(|e| e.to_string())?;
        let allowed: HashSet<u64> = ords.into_iter().collect();

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

        let mut vector_report = None;
        let vec_hits: Vec<rushdb_vector::ScoredSource> = match &query.vector {
            Some(leg) => {
                if leg.vector.is_empty() {
                    return Err("hybrid vector leg requires a non-empty vector".to_string());
                }
                let label = query.labels.first().cloned().unwrap_or_default();
                let dims = leg.vector.len();
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
                        dims,
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
                let candidate = |source: rushdb_vector::SourceRef| {
                    source.kind == OwnerKind::Record && allowed.contains(&source.owner_ord)
                };
                let (hits, report) = self.run_vector_search(
                    &vectors,
                    snap,
                    &space,
                    &super::vector_mode::VectorSearch {
                        query: &leg.vector,
                        k: prefetch(leg.prefetch_limit),
                        mode: leg.mode,
                        ef: leg.ef,
                        min_score: leg.score_threshold,
                    },
                    Some(&candidate),
                )?;
                vector_report = Some(report);
                hits
            }
            None => Vec::new(),
        };

        let mut text_report = None;
        let lex_hits: Vec<rushdb_textsearch::ScoredDoc> = match &query.text {
            Some(leg) => {
                let found = self.text.search(
                    self.tenant(),
                    snap,
                    rushdb_textsearch::TextQuery {
                        index: &leg.index,
                        query: &leg.query,
                        opts: rushdb_textsearch::LexicalOptions {
                            all_terms: leg.all_terms,
                            phrase: leg.phrase,
                        },
                        top_k: prefetch(leg.prefetch_limit),
                    },
                    Some(std::sync::Arc::new(allowed)),
                )?;
                text_report = Some(serde_json::json!({
                    "matches": found.matches,
                    "pending": found.pending,
                }));
                found.hits
            }
            None => Vec::new(),
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
