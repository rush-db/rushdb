//! Full-text indexes for `RushDB`.
//!
//! A text index is a declared index (`IndexKind::Text`) on one record label
//! and one property — or every string property (`*`). Its tantivy index is
//! persistent, one directory per index under `data_dir/text/<tenant>/<id>/`
//! (in RAM for in-memory stores), and maintained incrementally:
//!
//! - every commit that touches a covered record leaves a `TEXT_PENDING`
//!   row (written by the expander, see `rushdb_lmpg::index`);
//! - [`TextIndexes::apply`] reads pending rows at a snapshot, re-reads each
//!   record's current text, updates tantivy (delete the owner's document,
//!   add the current one), commits tantivy, then commits `IndexApplied`, which
//!   removes exactly the rows it consumed (a change after the snapshot stays
//!   pending). Re-applying after a crash between the two commits is
//!   idempotent;
//! - an `eventual` index is searched as applied and reports its backlog; an
//!   `immediate` index applies its pending rows before searching
//!   (read-your-writes).
//!
//! Nothing here pins the WAL, and a tenant without changes never commits.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use rushdb_lmpg::index::{self, pending};
use rushdb_storage::ReadView;
use rushdb_types::ids::TenantId;
use rushdb_types::index::{IndexDef, IndexKind, IndexState, TextConsistency, TEXT_ALL_PROPERTIES};

/// Owners applied per [`TextIndexes::apply`] step.
pub const APPLY_BATCH: usize = 512;

/// Pending rows an `immediate` search applies before giving up (bounds a
/// search behind a write storm; the response then reports the backlog).
const IMMEDIATE_APPLY_LIMIT: usize = 64 * APPLY_BATCH;

/// Open tantivy indexes kept per process over a persistent store (an
/// in-memory store keeps every index open: it holds the index's only copy).
const MAX_OPEN_INDEXES: usize = 256;

/// How a query string is read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextSyntax {
    /// Plain words: every character is text, and the query's terms are the
    /// tokens the index's own analysis produces. The default of `query.text`
    /// and the hybrid text leg, because callers send colons, apostrophes
    /// and the words `and`/`or`/`not`.
    #[default]
    Terms,
    /// Tantivy's query syntax: `+`, `-`, quotes, parentheses and
    /// upper-case `AND`/`OR`/`NOT`. A query that does not parse is refused
    /// as [`TextSearchError::InvalidQuery`].
    Query,
}

/// Lexical search options.
#[derive(Debug, Clone, Copy, Default)]
pub struct LexicalOptions {
    /// Every query term must match (AND); otherwise any term may (OR).
    pub all_terms: bool,
    /// Match the whole query as a phrase (term adjacency).
    pub phrase: bool,
    /// How the query string is read.
    pub syntax: TextSyntax,
}

/// Why a text search could not run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TextSearchError {
    /// No index of that name.
    #[error("unknown text index `{0}`")]
    UnknownIndex(String),
    /// The name belongs to an index of another kind.
    #[error("`{0}` is not a text index")]
    NotATextIndex(String),
    /// The index exists but is not `ready` yet; the search can be retried.
    #[error("text index `{index}` is {state}")]
    NotReady {
        /// The index name.
        index: String,
        /// Its current state.
        state: String,
    },
    /// The query string does not parse under [`TextSyntax::Query`], or
    /// names a field other than the indexed text.
    #[error("text query cannot be parsed: {0}")]
    InvalidQuery(String),
    /// A storage or index fault; the detail is for the log.
    #[error("{0}")]
    Internal(String),
}

impl From<String> for TextSearchError {
    fn from(message: String) -> Self {
        Self::Internal(message)
    }
}

/// One BM25-ranked owner with its raw score.
#[derive(Debug, Clone)]
pub struct ScoredDoc {
    /// Record ordinal within the tenant.
    pub owner_ord: u64,
    /// Raw BM25 score; only comparable within one result set.
    pub score: f64,
}

/// One text search.
#[derive(Debug, Clone, Copy)]
pub struct TextQuery<'a> {
    /// Text index name.
    pub index: &'a str,
    /// Query string.
    pub query: &'a str,
    /// Matching options.
    pub opts: LexicalOptions,
    /// Hits to return; [`ALL_HITS`] returns every match.
    pub top_k: usize,
}

/// [`TextQuery::top_k`] that returns every match.
pub const ALL_HITS: usize = usize::MAX;

/// A search result and how far the index trails the searched snapshot.
#[derive(Debug, Clone)]
pub struct TextHits {
    /// Ranked owners.
    pub hits: Vec<ScoredDoc>,
    /// Owners matching the query (within the candidates, when given), of
    /// which `hits` are the best `top_k`.
    pub matches: u64,
    /// Changes not yet applied when the search ran (always 0 for an
    /// `immediate` index unless a write storm exceeded its apply bound).
    pub pending: u64,
}

struct OpenIndex {
    index: tantivy::Index,
    reader: tantivy::IndexReader,
    owner_field: tantivy::schema::Field,
    text_field: tantivy::schema::Field,
    /// One applier at a time per index (tantivy allows one writer).
    apply: Mutex<()>,
}

/// The process's text indexes over one store.
pub struct TextIndexes {
    store: Arc<rushdb_storage::Store>,
    open: Mutex<HashMap<(TenantId, u32), Arc<OpenIndex>>>,
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

impl TextIndexes {
    /// Text indexes over `store`.
    #[must_use]
    pub fn new(store: Arc<rushdb_storage::Store>) -> Self {
        Self {
            store,
            open: Mutex::new(HashMap::new()),
        }
    }

    /// Directory of one text index, for persistent stores.
    fn dir(&self, tenant: TenantId, id: u32) -> Option<std::path::PathBuf> {
        self.store.data_dir().map(|data| {
            data.join("text")
                .join(rushdb_types::keys::tenant_hex(tenant))
                .join(id.to_string())
        })
    }

    fn open_index(&self, tenant: TenantId, id: u32) -> Result<Arc<OpenIndex>, String> {
        use tantivy::schema::{Schema, FAST, INDEXED, STORED, TEXT};
        let mut open = self.open.lock().map_err(|_| "text index cache poisoned")?;
        if let Some(found) = open.get(&(tenant, id)) {
            return Ok(Arc::clone(found));
        }
        let mut schema = Schema::builder();
        let owner_field = schema.add_u64_field("owner", INDEXED | STORED | FAST);
        let text_field = schema.add_text_field("text", TEXT);
        let schema = schema.build();
        let index = match self.dir(tenant, id) {
            Some(dir) => {
                std::fs::create_dir_all(&dir).map_err(err)?;
                let directory = tantivy::directory::MmapDirectory::open(&dir).map_err(err)?;
                tantivy::Index::open_or_create(directory, schema).map_err(err)?
            }
            None => tantivy::Index::create_in_ram(schema),
        };
        let reader = index
            .reader_builder()
            .reload_policy(tantivy::ReloadPolicy::Manual)
            .try_into()
            .map_err(err)?;
        let opened = Arc::new(OpenIndex {
            index,
            reader,
            owner_field,
            text_field,
            apply: Mutex::new(()),
        });
        // Idle on-disk indexes reopen from their files; an in-memory index
        // is its only copy, so only a persistent store evicts.
        if open.len() >= MAX_OPEN_INDEXES && self.store.data_dir().is_some() {
            open.retain(|_, index| Arc::strong_count(index) > 1);
        }
        open.insert((tenant, id), Arc::clone(&opened));
        Ok(opened)
    }

    /// Text indexes open in this process (bounded by the open-index cache).
    #[must_use]
    pub fn open_indexes(&self) -> usize {
        self.open.lock().map_or(0, |open| open.len())
    }

    /// Forget and delete the files of a text index whose definition is gone.
    ///
    /// # Errors
    /// Returns an error if the directory cannot be removed.
    pub fn remove(&self, tenant: TenantId, id: u32) -> Result<(), String> {
        self.open
            .lock()
            .map_err(|_| "text index cache poisoned")?
            .remove(&(tenant, id));
        match self.dir(tenant, id) {
            Some(dir) if dir.exists() => std::fs::remove_dir_all(dir).map_err(err),
            _ => Ok(()),
        }
    }

    /// Forget and delete every text index of a purged tenant.
    ///
    /// # Errors
    /// Returns an error if the tenant's directory cannot be removed.
    pub fn remove_tenant(&self, tenant: TenantId) -> Result<(), String> {
        self.open
            .lock()
            .map_err(|_| "text index cache poisoned")?
            .retain(|(owner, _), _| *owner != tenant);
        match self.store.data_dir() {
            Some(data) => {
                let dir = data
                    .join("text")
                    .join(rushdb_types::keys::tenant_hex(tenant));
                if dir.exists() {
                    std::fs::remove_dir_all(dir).map_err(err)?;
                }
                Ok(())
            }
            None => Ok(()),
        }
    }

    /// Apply up to `max_owners` pending changes of text index `def`: index
    /// each owner's current text, commit tantivy, then commit `IndexApplied`.
    /// Returns the owners applied (0 when nothing is pending).
    ///
    /// # Errors
    /// Returns an error if a read, the tantivy commit, or the store commit
    /// fails; pending rows then stay for the next attempt.
    pub fn apply(
        &self,
        lmpg: &rushdb_lmpg::Lmpg,
        def: &IndexDef,
        max_owners: usize,
    ) -> Result<usize, String> {
        let tenant = lmpg.tenant;
        let opened = self.open_index(tenant, def.id)?;
        let _single = opened.apply.lock().map_err(|_| "text applier poisoned")?;
        let snap = lmpg.store.read_view().map_err(err)?;
        let owners = pending(&snap, tenant, def.id, max_owners).map_err(err)?;
        if owners.is_empty() {
            return Ok(0);
        }
        let mut writer: tantivy::IndexWriter = opened.index.writer(15_000_000).map_err(err)?;
        let mut session = rushdb_lmpg::reconstruct::ReconstructSession::new(&snap, tenant);
        // An index on one property decodes that property alone; `*` needs
        // every string of the record.
        let selected: Option<HashSet<String>> = (def.property != TEXT_ALL_PROPERTIES).then(|| {
            [
                def.property.clone(),
                rushdb_types::value::normalize_name(&def.property),
            ]
            .into_iter()
            .collect()
        });
        for &owner in &owners {
            writer.delete_term(tantivy::Term::from_field_u64(opened.owner_field, owner));
            let covered = def.state.covers(owner)
                && rushdb_lmpg::reader::try_access::record_by_ord(&snap, tenant, owner)
                    .map_err(err)?
                    .is_some_and(|header| header.label == def.label);
            if !covered {
                continue;
            }
            let record = match &selected {
                Some(selected) => session.reconstruct_record_selected(owner, selected),
                None => session.reconstruct_record(owner),
            }
            .map_err(err)?;
            let Some(record) = record else {
                continue;
            };
            let text = record_text(&record, &def.property);
            if !text.is_empty() {
                writer
                    .add_document(tantivy::doc!(
                        opened.owner_field => owner,
                        opened.text_field => text
                    ))
                    .map_err(err)?;
            }
        }
        writer.commit().map_err(err)?;
        opened.reader.reload().map_err(err)?;
        let applied = owners.len();
        lmpg.index_applied(def.id, snap.commit_index(), owners)
            .map_err(|e| e.to_string())?;
        Ok(applied)
    }

    /// Apply the pending changes of every text index of `lmpg`'s tenant, a
    /// batch at a time, until done or `budget` elapses. Returns the owners
    /// applied.
    ///
    /// # Errors
    /// Returns the first apply error.
    pub fn apply_tenant(
        &self,
        lmpg: &rushdb_lmpg::Lmpg,
        budget: std::time::Duration,
    ) -> Result<usize, String> {
        let started = std::time::Instant::now();
        let view = lmpg.store.read_view().map_err(err)?;
        let defs = index::definitions(&view, lmpg.tenant).map_err(err)?;
        drop(view);
        let mut applied = 0;
        for def in defs {
            if !matches!(def.kind, IndexKind::Text { .. })
                || matches!(def.state, IndexState::Dropping { .. })
            {
                continue;
            }
            loop {
                let step = self.apply(lmpg, &def, APPLY_BATCH)?;
                applied += step;
                if step < APPLY_BATCH || started.elapsed() >= budget {
                    break;
                }
            }
            if started.elapsed() >= budget {
                break;
            }
        }
        Ok(applied)
    }

    /// BM25 top-k for `request` at `view`, restricted to
    /// `candidates` when given. An `immediate` index first applies its
    /// pending changes (bounded); an `eventual` one reports its backlog.
    ///
    /// # Errors
    /// Returns [`TextSearchError`]: an unknown name, an index of another
    /// kind, an index not yet ready, a query that does not parse, or an
    /// index or storage fault.
    pub fn search(
        &self,
        tenant: TenantId,
        view: &ReadView,
        request: TextQuery<'_>,
        candidates: Option<Arc<HashSet<u64>>>,
    ) -> Result<TextHits, TextSearchError> {
        let TextQuery {
            index: name,
            query,
            opts,
            top_k,
        } = request;
        let lmpg = rushdb_lmpg::Lmpg::new(Arc::clone(&self.store), tenant);
        let def = index::by_name(view, tenant, name)
            .map_err(err)?
            .ok_or_else(|| TextSearchError::UnknownIndex(name.to_owned()))?;
        let IndexKind::Text { consistency } = def.kind else {
            return Err(TextSearchError::NotATextIndex(name.to_owned()));
        };
        if def.state != IndexState::Ready {
            return Err(TextSearchError::NotReady {
                index: name.to_owned(),
                state: def.state.label().to_owned(),
            });
        }
        if consistency == TextConsistency::Immediate {
            let mut applied = 0;
            while applied < IMMEDIATE_APPLY_LIMIT {
                let step = self.apply(&lmpg, &def, APPLY_BATCH)?;
                if step == 0 {
                    break;
                }
                applied += step;
            }
        }
        let pending = {
            let now = lmpg.store.read_view().map_err(err)?;
            pending(&now, tenant, def.id, IMMEDIATE_APPLY_LIMIT)
                .map_err(err)?
                .len() as u64
        };
        if query.trim().is_empty() {
            return Ok(TextHits {
                hits: Vec::new(),
                matches: 0,
                pending,
            });
        }
        let opened = self.open_index(tenant, def.id)?;
        let (hits, matches) = ranked(&opened, query, opts, top_k, candidates)?;
        Ok(TextHits {
            hits,
            matches: u64::try_from(matches).unwrap_or(u64::MAX),
            pending,
        })
    }

    /// The name of the `ready` text index on exactly `label` and `property`
    /// (`*` names the index over every string of the record), if any.
    ///
    /// # Errors
    /// Returns an error if the index catalog cannot be read.
    pub fn index_for<R: rushdb_lmpg::reader::TryRead + ?Sized>(
        view: &R,
        tenant: TenantId,
        label: &str,
        property: &str,
    ) -> Result<Option<String>, rushdb_storage::StorageError> {
        let property = if property == TEXT_ALL_PROPERTIES {
            TEXT_ALL_PROPERTIES.to_string()
        } else {
            rushdb_types::value::normalize_name(property)
        };
        Ok(index::definitions(view, tenant)?
            .into_iter()
            .find(|def| {
                matches!(def.kind, IndexKind::Text { .. })
                    && def.state == IndexState::Ready
                    && def.label == label
                    && def.property == property
            })
            .map(|def| def.name))
    }
}

/// The query `text` denotes under `opts.syntax`.
fn build_query(
    opened: &OpenIndex,
    text: &str,
    opts: LexicalOptions,
) -> Result<Box<dyn tantivy::query::Query>, TextSearchError> {
    use tantivy::query::{
        BooleanQuery, EmptyQuery, Occur, PhraseQuery, Query, QueryParser, TermQuery,
    };
    use tantivy::schema::IndexRecordOption;
    use tantivy::tokenizer::TokenStream;
    use tantivy::Term;
    match opts.syntax {
        TextSyntax::Query => {
            let mut parser = QueryParser::for_index(&opened.index, vec![opened.text_field]);
            if opts.all_terms {
                parser.set_conjunction_by_default();
            }
            let expression = if opts.phrase {
                format!("\"{}\"", text.replace('"', "\\\""))
            } else {
                text.to_string()
            };
            let parsed = parser
                .parse_query(&expression)
                .map_err(|error| TextSearchError::InvalidQuery(error.to_string()))?;
            // Only the indexed text is searchable: `owner:` would name the
            // index's internal id column.
            let mut foreign = false;
            parsed.query_terms(&mut |term, _| {
                if term.field() != opened.text_field {
                    foreign = true;
                }
            });
            if foreign {
                return Err(TextSearchError::InvalidQuery(
                    "only the indexed text can be searched; a `field:` prefix is not accepted"
                        .into(),
                ));
            }
            Ok(parsed)
        }
        TextSyntax::Terms => {
            // The index's own analysis: what it did to the documents.
            let mut analyzer = opened
                .index
                .tokenizer_for_field(opened.text_field)
                .map_err(err)?;
            let mut stream = analyzer.token_stream(text);
            let mut terms = Vec::new();
            while stream.advance() {
                terms.push(Term::from_field_text(
                    opened.text_field,
                    &stream.token().text,
                ));
            }
            let occur = if opts.all_terms {
                Occur::Must
            } else {
                Occur::Should
            };
            Ok(match (terms.len(), opts.phrase) {
                (0, _) => Box::new(EmptyQuery),
                (1, _) => Box::new(TermQuery::new(
                    terms.remove(0),
                    IndexRecordOption::WithFreqs,
                )),
                (_, true) => Box::new(PhraseQuery::new(terms)),
                (_, false) => Box::new(BooleanQuery::new(
                    terms
                        .into_iter()
                        .map(|term| {
                            let query: Box<dyn Query> =
                                Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs));
                            (occur, query)
                        })
                        .collect(),
                )),
            })
        }
    }
}

/// The best `top_k` matches of `query` in `opened` (all of them for
/// [`ALL_HITS`]) among `candidates`, and how many matched.
fn ranked(
    opened: &OpenIndex,
    query: &str,
    opts: LexicalOptions,
    top_k: usize,
    candidates: Option<Arc<HashSet<u64>>>,
) -> Result<(Vec<ScoredDoc>, usize), TextSearchError> {
    use tantivy::collector::{Count, FilterCollector, TopDocs};
    use tantivy::schema::{TantivyDocument, Value as _};
    let searcher = opened.reader.searcher();
    let parsed = build_query(opened, query, opts)?;
    let accepts = move |owner: u64| candidates.as_ref().is_none_or(|c| c.contains(&owner));
    let matches = searcher
        .search(
            &*parsed,
            &FilterCollector::new("owner".to_string(), accepts.clone(), Count),
        )
        .map_err(err)?;
    // Never size the collector past what matched (`ALL_HITS` included).
    let limit = top_k.min(matches);
    let found = if limit == 0 {
        Vec::new()
    } else {
        searcher
            .search(
                &*parsed,
                &FilterCollector::new(
                    "owner".to_string(),
                    accepts,
                    TopDocs::with_limit(limit).order_by_score(),
                ),
            )
            .map_err(err)?
    };
    let mut hits = Vec::with_capacity(found.len());
    for (score, address) in found {
        let doc = searcher.doc::<TantivyDocument>(address).map_err(err)?;
        if let Some(owner) = doc
            .get_first(opened.owner_field)
            .and_then(|v| v.as_value().as_u64())
        {
            hits.push(ScoredDoc {
                owner_ord: owner,
                score: f64::from(score),
            });
        }
    }
    // Deterministic ties: score, then owner.
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.owner_ord.cmp(&b.owner_ord))
    });
    Ok((hits, matches))
}

/// The text a record contributes: every string value of `property`
/// (normalized), or of the whole record for `*`.
fn record_text(record: &serde_json::Value, property: &str) -> String {
    let mut values = Vec::new();
    if property == TEXT_ALL_PROPERTIES {
        collect_text_values(record, &mut values);
    } else if let serde_json::Value::Object(fields) = record {
        for (key, value) in fields {
            if rushdb_types::value::normalize_name(key) == property {
                collect_text_values(value, &mut values);
            }
        }
    }
    values.join(" ")
}

fn collect_text_values(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(value) => out.push(value.clone()),
        serde_json::Value::Array(values) => values
            .iter()
            .for_each(|value| collect_text_values(value, out)),
        serde_json::Value::Object(values) => values
            .iter()
            .filter(|(key, _)| !rushdb_types::value::is_reserved_name(key))
            .for_each(|(_, value)| collect_text_values(value, out)),
        _ => {}
    }
}
