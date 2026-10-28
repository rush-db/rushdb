//! `text_search(index, query [, limit [, options]])`: the BM25 hits of a text
//! index as a table `(id TEXT, score DOUBLE)`, best first, searched at the
//! statement's pinned snapshot. Join it to a label table on `__id`:
//!
//! ```sql
//! SELECT d.title, t.score
//! FROM text_search('docs', 'quick fox', 20) AS t JOIN doc d ON d.__id = t.id
//! ORDER BY t.score DESC
//! ```
//!
//! `limit` defaults to 10; `options` is JSON text such as
//! `'{"allTerms": true}'` or `'{"phrase": true}'`. Arguments are literals
//! (the search runs while the statement is planned).

use std::sync::Arc;

use arrow::array::{Float64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use datafusion::catalog::{TableFunctionArgs, TableFunctionImpl, TableProvider};
use datafusion::common::{DataFusionError, Result, ScalarValue};
use datafusion::datasource::MemTable;
use datafusion::execution::context::SessionContext;
use datafusion::logical_expr::Expr;
use rushdb_lmpg::reader::try_access as access;
use rushdb_lmpg::Lmpg;
use rushdb_storage::ReadView;
use rushdb_textsearch::{LexicalOptions, TextIndexes, TextQuery};
use rushdb_types::ids::TenantId;

/// Hits returned when the call names no limit.
const DEFAULT_LIMIT: usize = 10;

/// Register `text_search` on `context`, searching `text` at `snapshot`.
/// Without text indexes (`None`) the function exists but every call fails,
/// so statements still plan the same way everywhere.
pub(crate) fn install(
    context: &SessionContext,
    lmpg: &Lmpg,
    snapshot: &Arc<ReadView>,
    text: Option<&Arc<TextIndexes>>,
) {
    context.register_udtf(
        "text_search",
        Arc::new(TextSearchFunction {
            text: text.cloned(),
            tenant: lmpg.tenant,
            snapshot: Arc::clone(snapshot),
        }),
    );
}

/// A `text_search` call that was invalid or whose search failed.
#[derive(Debug)]
pub(crate) struct TextSearchError(String);

impl std::fmt::Display for TextSearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TextSearchError {}

fn failed<T>(message: String) -> Result<T> {
    Err(DataFusionError::External(Box::new(TextSearchError(
        message,
    ))))
}

struct TextSearchFunction {
    text: Option<Arc<TextIndexes>>,
    tenant: TenantId,
    snapshot: Arc<ReadView>,
}

impl std::fmt::Debug for TextSearchFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("text_search")
    }
}

impl TableFunctionImpl for TextSearchFunction {
    fn call_with_args(&self, args: TableFunctionArgs) -> Result<Arc<dyn TableProvider>> {
        let exprs = args.exprs();
        if !(2..=4).contains(&exprs.len()) {
            return failed(
                "text_search(index, query [, limit [, options]]) takes 2 to 4 arguments".into(),
            );
        }
        let index = text_argument(&exprs[0], "index")?;
        let query = text_argument(&exprs[1], "query")?;
        let limit = match exprs.get(2) {
            None => DEFAULT_LIMIT,
            Some(expr) => limit_argument(expr)?,
        };
        let opts = match exprs.get(3) {
            None => LexicalOptions::default(),
            Some(expr) => options_argument(&text_argument(expr, "options")?)?,
        };
        let Some(text) = &self.text else {
            return failed("text_search is unavailable in this context".into());
        };
        let found = text
            .search(
                self.tenant,
                &self.snapshot,
                TextQuery {
                    index: &index,
                    query: &query,
                    opts,
                    top_k: limit,
                },
                None,
            )
            .or_else(failed)?;
        let mut ids = Vec::with_capacity(found.hits.len());
        let mut scores = Vec::with_capacity(found.hits.len());
        for hit in found.hits {
            let header = access::record_by_ord(&self.snapshot, self.tenant, hit.owner_ord)
                .map_err(|error| DataFusionError::External(Box::new(error)))?;
            if let Some(header) = header {
                ids.push(uuid::Uuid::from_bytes(header.record_id).to_string());
                scores.push(hit.score);
            }
        }
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("score", DataType::Float64, false),
        ]));
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![
                Arc::new(StringArray::from(ids)),
                Arc::new(Float64Array::from(scores)),
            ],
        )?;
        Ok(Arc::new(MemTable::try_new(schema, vec![vec![batch]])?))
    }
}

fn text_argument(expr: &Expr, name: &str) -> Result<String> {
    match expr {
        Expr::Literal(ScalarValue::Utf8(Some(value)) | ScalarValue::LargeUtf8(Some(value)), _) => {
            Ok(value.clone())
        }
        _ => failed(format!("text_search `{name}` must be a text literal")),
    }
}

fn limit_argument(expr: &Expr) -> Result<usize> {
    let value = match expr {
        Expr::Literal(ScalarValue::Int64(Some(value)), _) => *value,
        Expr::Literal(ScalarValue::Int32(Some(value)), _) => i64::from(*value),
        _ => return failed("text_search `limit` must be an integer literal".into()),
    };
    usize::try_from(value).or_else(|_| failed("text_search `limit` must not be negative".into()))
}

fn options_argument(options: &str) -> Result<LexicalOptions> {
    let parsed: serde_json::Value = serde_json::from_str(options)
        .or_else(|error| failed(format!("text_search `options` is not JSON: {error}")))?;
    let Some(fields) = parsed.as_object() else {
        return failed("text_search `options` must be a JSON object".into());
    };
    let mut opts = LexicalOptions::default();
    for (name, value) in fields {
        let Some(flag) = value.as_bool() else {
            return failed(format!("text_search option `{name}` must be true or false"));
        };
        match name.as_str() {
            "allTerms" => opts.all_terms = flag,
            "phrase" => opts.phrase = flag,
            _ => {
                return failed(format!(
                    "unknown text_search option `{name}` (allTerms, phrase)"
                ))
            }
        }
    }
    Ok(opts)
}
