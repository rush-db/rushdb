//! A read-only `pg_catalog` and `information_schema` for PostgreSQL clients
//! and BI tools.
//!
//! JDBC's `DatabaseMetaData`, Metabase's sync and similar tools list
//! schemas, tables and columns by querying PostgreSQL's system catalogs and
//! its `information_schema` (with PostgreSQL's columns and type names).
//! Both schemas answer from the session's own catalog: the
//! label tables of `public` and the relationship tables of `rushdb_edges`,
//! exactly as the session sees them (under an enforced policy, only what the
//! policy shows). Each catalog table is built when a statement first names
//! it; object ids are assigned in schema and table name order, so the tables
//! of one statement agree with each other.
//!
//! Catalogs of objects `RushDB` does not have (indexes as PostgreSQL models
//! them, constraints, defaults, descriptions, views) are empty tables with
//! PostgreSQL's columns, so a query that joins them still plans. The
//! catalog functions such tools call (`has_table_privilege`,
//! `format_type`, `current_schemas`, …) answer for that catalog: every
//! listed object is usable by the session, as listing already applied the
//! policy.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use arrow::array::{
    Array, ArrayRef, BooleanArray, Float64Array, Int64Array, ListBuilder, RecordBatch, StringArray,
    StringBuilder,
};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use async_trait::async_trait;
use datafusion::catalog::{SchemaProvider, TableProvider};
use datafusion::common::{DataFusionError, Result as DFResult, ScalarValue};
use datafusion::datasource::MemTable;
use datafusion::execution::context::SessionContext;
use datafusion::logical_expr::{
    ColumnarValue, ScalarFunctionArgs, ScalarUDF, ScalarUDFImpl, Signature, Volatility,
};

use crate::SqlType;

/// `pg_catalog`'s own object id, and `public`'s, as in PostgreSQL.
const PG_CATALOG_OID: i64 = 11;
const PUBLIC_OID: i64 = 2200;
const INFORMATION_SCHEMA_OID: i64 = 13_000;
const EDGES_OID: i64 = 16_384;
/// The first object id of a listed table.
const FIRST_TABLE_OID: i64 = 16_385;
/// The owner of every object (`pg_get_userbyid` answers its name).
const OWNER_OID: i64 = 10;

/// The tables a session lists, built once per statement for both schemas.
pub(crate) struct Listings {
    /// The listed schemas: name, object id and provider.
    sources: Vec<(&'static str, i64, Arc<dyn SchemaProvider>)>,
    built: Mutex<Option<Arc<Listing>>>,
}

/// One listed table.
struct Listed {
    oid: i64,
    namespace: i64,
    schema: &'static str,
    name: String,
    columns: SchemaRef,
    rows: Option<usize>,
}

struct Listing {
    tables: Vec<Listed>,
}

impl Listings {
    fn new(labels: Arc<dyn SchemaProvider>, edges: Arc<dyn SchemaProvider>) -> Self {
        Self {
            sources: vec![
                ("public", PUBLIC_OID, labels),
                ("rushdb_edges", EDGES_OID, edges),
            ],
            built: Mutex::new(None),
        }
    }

    async fn listing(&self) -> DFResult<Arc<Listing>> {
        if let Some(listing) = self.lock()?.as_ref() {
            return Ok(Arc::clone(listing));
        }
        let mut tables = Vec::new();
        for (schema, namespace, provider) in &self.sources {
            let mut names = provider.table_names();
            names.sort();
            for name in names {
                let Some(table) = provider.table(&name).await? else {
                    continue;
                };
                let rows = table
                    .statistics()
                    .and_then(|stats| stats.num_rows.get_value().copied());
                tables.push(Listed {
                    oid: 0,
                    namespace: *namespace,
                    schema,
                    name,
                    columns: table.schema(),
                    rows,
                });
            }
        }
        for (oid, table) in (FIRST_TABLE_OID..).zip(tables.iter_mut()) {
            table.oid = oid;
        }
        let listing = Arc::new(Listing { tables });
        *self.lock()? = Some(Arc::clone(&listing));
        Ok(listing)
    }

    fn lock(&self) -> DFResult<std::sync::MutexGuard<'_, Option<Arc<Listing>>>> {
        self.built
            .lock()
            .map_err(|_| DataFusionError::Execution("catalog listing lock poisoned".into()))
    }
}

/// The schema a statement sees as `pg_catalog`.
pub(crate) struct PgCatalogSchema {
    listings: Arc<Listings>,
}

impl std::fmt::Debug for PgCatalogSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgCatalogSchema").finish_non_exhaustive()
    }
}

const TABLES: [&str; 15] = [
    "pg_namespace",
    "pg_class",
    "pg_attribute",
    "pg_type",
    "pg_tables",
    "pg_stat_user_tables",
    "pg_description",
    "pg_index",
    "pg_constraint",
    "pg_attrdef",
    "pg_am",
    "pg_enum",
    "pg_inherits",
    "pg_views",
    "pg_matviews",
];

fn mem_table(batch: RecordBatch) -> DFResult<Option<Arc<dyn TableProvider>>> {
    Ok(Some(Arc::new(MemTable::try_new(
        batch.schema(),
        vec![vec![batch]],
    )?)))
}

#[async_trait]
impl SchemaProvider for PgCatalogSchema {
    fn table_names(&self) -> Vec<String> {
        TABLES.iter().map(|name| (*name).to_owned()).collect()
    }

    fn table_exist(&self, name: &str) -> bool {
        TABLES.contains(&name)
    }

    async fn table(&self, name: &str) -> DFResult<Option<Arc<dyn TableProvider>>> {
        if !self.table_exist(name) {
            return Ok(None);
        }
        let listing = self.listings.listing().await?;
        mem_table(match name {
            "pg_namespace" => namespaces(),
            "pg_class" => classes(&listing),
            "pg_attribute" => attributes(&listing),
            "pg_type" => types(),
            "pg_tables" => pg_tables(&listing),
            "pg_stat_user_tables" => stat_user_tables(&listing),
            other => empty(other),
        }?)
    }
}

/// The schema a statement sees as `information_schema`, with PostgreSQL's
/// columns.
pub(crate) struct InformationSchema {
    listings: Arc<Listings>,
}

impl std::fmt::Debug for InformationSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InformationSchema").finish_non_exhaustive()
    }
}

const VIEWS: [&str; 8] = [
    "tables",
    "columns",
    "schemata",
    "views",
    "table_constraints",
    "key_column_usage",
    "constraint_column_usage",
    "referential_constraints",
];

/// The catalog name `information_schema` reports.
const CATALOG: &str = "rushdb";

#[async_trait]
impl SchemaProvider for InformationSchema {
    fn table_names(&self) -> Vec<String> {
        VIEWS.iter().map(|name| (*name).to_owned()).collect()
    }

    fn table_exist(&self, name: &str) -> bool {
        VIEWS.contains(&name)
    }

    async fn table(&self, name: &str) -> DFResult<Option<Arc<dyn TableProvider>>> {
        if !self.table_exist(name) {
            return Ok(None);
        }
        let listing = self.listings.listing().await?;
        mem_table(match name {
            "tables" => info_tables(&listing),
            "columns" => info_columns(&listing),
            "schemata" => schemata(),
            other => empty(other),
        }?)
    }
}

fn info_tables(listing: &Listing) -> DFResult<RecordBatch> {
    let tables = &listing.tables;
    let n = tables.len();
    batch(vec![
        ("table_catalog", texts((0..n).map(|_| CATALOG))),
        (
            "table_schema",
            texts(tables.iter().map(|table| table.schema)),
        ),
        ("table_name", texts(tables.iter().map(|table| &table.name))),
        ("table_type", texts((0..n).map(|_| "BASE TABLE"))),
        ("is_insertable_into", texts((0..n).map(|_| "YES"))),
    ])
}

fn info_columns(listing: &Listing) -> DFResult<RecordBatch> {
    let (mut schema, mut table, mut column, mut position) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut nullable, mut data_type, mut udt) = (Vec::new(), Vec::new(), Vec::new());
    let (mut precision, mut radix, mut scale) = (Vec::new(), Vec::new(), Vec::new());
    for listed in &listing.tables {
        for (ordinal, field) in (1..).zip(listed.columns.fields().iter()) {
            let sql = crate::sql_type_from_arrow(field.data_type());
            schema.push(listed.schema);
            table.push(listed.name.clone());
            column.push(field.name().clone());
            position.push(ordinal);
            nullable.push(if field.is_nullable() { "YES" } else { "NO" });
            data_type.push(sql.pg_data_type());
            udt.push(sql.pg_name());
            let (digits, base, fraction) = numeric_shape(&sql);
            precision.push(digits);
            radix.push(base);
            scale.push(fraction);
        }
    }
    let n = schema.len();
    let none = || -> ArrayRef { Arc::new(Int64Array::from(vec![None; n])) };
    batch(vec![
        ("table_catalog", texts((0..n).map(|_| CATALOG))),
        ("table_schema", texts(schema)),
        ("table_name", texts(table)),
        ("column_name", texts(column)),
        ("ordinal_position", ints(position)),
        ("column_default", null_texts(n)),
        ("is_nullable", texts(nullable)),
        ("data_type", texts(data_type)),
        ("character_maximum_length", none()),
        ("character_octet_length", none()),
        ("numeric_precision", Arc::new(Int64Array::from(precision))),
        ("numeric_precision_radix", Arc::new(Int64Array::from(radix))),
        ("numeric_scale", Arc::new(Int64Array::from(scale))),
        ("datetime_precision", none()),
        ("udt_catalog", texts((0..n).map(|_| CATALOG))),
        ("udt_schema", texts((0..n).map(|_| "pg_catalog"))),
        ("udt_name", texts(udt)),
        ("is_identity", texts((0..n).map(|_| "NO"))),
        ("identity_generation", null_texts(n)),
        ("is_generated", texts((0..n).map(|_| "NEVER"))),
        ("generation_expression", null_texts(n)),
        ("is_updatable", texts((0..n).map(|_| "YES"))),
    ])
}

/// `(numeric_precision, numeric_precision_radix, numeric_scale)` as
/// PostgreSQL reports them for a column of `sql`.
fn numeric_shape(sql: &SqlType) -> (Option<i64>, Option<i64>, Option<i64>) {
    match sql {
        SqlType::Int16 | SqlType::UInt16 => (Some(16), Some(2), Some(0)),
        SqlType::Int32 | SqlType::UInt32 => (Some(32), Some(2), Some(0)),
        SqlType::Int64 | SqlType::UInt64 => (Some(64), Some(2), Some(0)),
        SqlType::Float32 => (Some(24), Some(2), None),
        SqlType::Float64 => (Some(53), Some(2), None),
        _ => (None, None, None),
    }
}

fn schemata() -> DFResult<RecordBatch> {
    let names = ["pg_catalog", "public", "information_schema", "rushdb_edges"];
    batch(vec![
        ("catalog_name", texts(names.iter().map(|_| CATALOG))),
        ("schema_name", texts(names)),
        (
            "schema_owner",
            texts(names.iter().map(|_| crate::SESSION_OWNER)),
        ),
    ])
}

fn batch(columns: Vec<(&str, ArrayRef)>) -> DFResult<RecordBatch> {
    let schema = Schema::new(
        columns
            .iter()
            .map(|(name, array)| Field::new(*name, array.data_type().clone(), true))
            .collect::<Vec<_>>(),
    );
    Ok(RecordBatch::try_new(
        Arc::new(schema),
        columns.into_iter().map(|(_, array)| array).collect(),
    )?)
}

fn ints(values: impl IntoIterator<Item = i64>) -> ArrayRef {
    Arc::new(Int64Array::from_iter_values(values))
}

fn texts<S: AsRef<str>>(values: impl IntoIterator<Item = S>) -> ArrayRef {
    Arc::new(StringArray::from_iter_values(values))
}

fn null_texts(rows: usize) -> ArrayRef {
    Arc::new(StringArray::from(vec![None::<&str>; rows]))
}

fn bools(value: bool, rows: usize) -> ArrayRef {
    Arc::new(BooleanArray::from(vec![value; rows]))
}

fn namespaces() -> DFResult<RecordBatch> {
    let rows = [
        (PG_CATALOG_OID, "pg_catalog"),
        (PUBLIC_OID, "public"),
        (INFORMATION_SCHEMA_OID, "information_schema"),
        (EDGES_OID, "rushdb_edges"),
    ];
    batch(vec![
        ("oid", ints(rows.iter().map(|(oid, _)| *oid))),
        ("nspname", texts(rows.iter().map(|(_, name)| *name))),
        ("nspowner", ints(rows.iter().map(|_| OWNER_OID))),
        ("nspacl", null_texts(rows.len())),
    ])
}

fn classes(listing: &Listing) -> DFResult<RecordBatch> {
    let tables = &listing.tables;
    let n = tables.len();
    #[expect(
        clippy::cast_precision_loss,
        reason = "a row estimate, as PostgreSQL's reltuples is"
    )]
    let tuples: Vec<f64> = tables
        .iter()
        .map(|table| table.rows.map_or(-1.0, |rows| rows as f64))
        .collect();
    batch(vec![
        ("oid", ints(tables.iter().map(|table| table.oid))),
        ("relname", texts(tables.iter().map(|table| &table.name))),
        (
            "relnamespace",
            ints(tables.iter().map(|table| table.namespace)),
        ),
        ("reltype", ints(tables.iter().map(|_| 0))),
        ("relowner", ints(tables.iter().map(|_| OWNER_OID))),
        ("relam", ints(tables.iter().map(|_| 0))),
        ("relkind", texts(tables.iter().map(|_| "r"))),
        ("relpersistence", texts(tables.iter().map(|_| "p"))),
        ("reltuples", Arc::new(Float64Array::from(tuples))),
        ("relpages", ints(tables.iter().map(|_| 0))),
        (
            "relnatts",
            ints(
                tables
                    .iter()
                    .map(|table| i64::try_from(table.columns.fields().len()).unwrap_or(i64::MAX)),
            ),
        ),
        ("relchecks", ints(tables.iter().map(|_| 0))),
        ("relhasindex", bools(false, n)),
        ("relhasrules", bools(false, n)),
        ("relhastriggers", bools(false, n)),
        ("relhassubclass", bools(false, n)),
        ("relrowsecurity", bools(false, n)),
        ("relforcerowsecurity", bools(false, n)),
        ("relispopulated", bools(true, n)),
        ("relispartition", bools(false, n)),
        ("relreplident", texts(tables.iter().map(|_| "d"))),
        ("relacl", null_texts(n)),
        ("reloptions", null_texts(n)),
    ])
}

fn attributes(listing: &Listing) -> DFResult<RecordBatch> {
    let mut rel = Vec::new();
    let mut name = Vec::new();
    let mut type_oid = Vec::new();
    let mut number = Vec::new();
    let mut length = Vec::new();
    let mut not_null = Vec::new();
    for table in &listing.tables {
        for (position, field) in (1..).zip(table.columns.fields().iter()) {
            let sql = crate::sql_type_from_arrow(field.data_type());
            rel.push(table.oid);
            name.push(field.name().clone());
            type_oid.push(i64::from(sql.pg_oid()));
            number.push(position);
            length.push(pg_length(&sql));
            not_null.push(!field.is_nullable());
        }
    }
    let n = rel.len();
    batch(vec![
        ("attrelid", ints(rel)),
        ("attname", texts(name)),
        ("atttypid", ints(type_oid)),
        ("attnum", ints(number)),
        ("attlen", ints(length)),
        ("atttypmod", ints((0..n).map(|_| -1))),
        ("attndims", ints((0..n).map(|_| 0))),
        ("attnotnull", Arc::new(BooleanArray::from(not_null))),
        ("atthasdef", bools(false, n)),
        ("atthasmissing", bools(false, n)),
        ("attidentity", texts((0..n).map(|_| ""))),
        ("attgenerated", texts((0..n).map(|_| ""))),
        ("attisdropped", bools(false, n)),
        ("attislocal", bools(true, n)),
        ("attinhcount", ints((0..n).map(|_| 0))),
        ("attcollation", ints((0..n).map(|_| 0))),
        ("attstattarget", ints((0..n).map(|_| -1))),
        ("attacl", null_texts(n)),
    ])
}

/// The bytes PostgreSQL stores a value of `sql` in, or `-1` for variable
/// length.
fn pg_length(sql: &SqlType) -> i64 {
    match sql {
        SqlType::Boolean => 1,
        SqlType::Int16 | SqlType::UInt16 => 2,
        SqlType::Int32 | SqlType::UInt32 | SqlType::Float32 | SqlType::Date => 4,
        SqlType::Int64
        | SqlType::UInt64
        | SqlType::Float64
        | SqlType::Time
        | SqlType::Timestamp => 8,
        SqlType::Utf8 | SqlType::Binary | SqlType::Json | SqlType::Unknown => -1,
    }
}

/// The types a column can have here, with their PostgreSQL names.
const TYPES: [SqlType; 12] = [
    SqlType::Boolean,
    SqlType::Int16,
    SqlType::Int32,
    SqlType::Int64,
    SqlType::Float32,
    SqlType::Float64,
    SqlType::Utf8,
    SqlType::Binary,
    SqlType::Date,
    SqlType::Time,
    SqlType::Timestamp,
    SqlType::Json,
];

fn types() -> DFResult<RecordBatch> {
    let n = TYPES.len();
    batch(vec![
        ("oid", ints(TYPES.iter().map(|ty| i64::from(ty.pg_oid())))),
        ("typname", texts(TYPES.iter().map(SqlType::pg_name))),
        ("typnamespace", ints((0..n).map(|_| PG_CATALOG_OID))),
        ("typowner", ints((0..n).map(|_| OWNER_OID))),
        ("typlen", ints(TYPES.iter().map(pg_length))),
        ("typbyval", bools(false, n)),
        ("typtype", texts((0..n).map(|_| "b"))),
        ("typcategory", texts(TYPES.iter().map(pg_category))),
        ("typisdefined", bools(true, n)),
        ("typrelid", ints((0..n).map(|_| 0))),
        ("typelem", ints((0..n).map(|_| 0))),
        ("typarray", ints((0..n).map(|_| 0))),
        ("typbasetype", ints((0..n).map(|_| 0))),
        ("typtypmod", ints((0..n).map(|_| -1))),
        ("typnotnull", bools(false, n)),
        ("typndims", ints((0..n).map(|_| 0))),
        ("typcollation", ints((0..n).map(|_| 0))),
    ])
}

fn pg_category(sql: &SqlType) -> &'static str {
    match sql {
        SqlType::Boolean => "B",
        SqlType::Int16
        | SqlType::Int32
        | SqlType::Int64
        | SqlType::UInt16
        | SqlType::UInt32
        | SqlType::UInt64
        | SqlType::Float32
        | SqlType::Float64 => "N",
        SqlType::Utf8 => "S",
        SqlType::Date | SqlType::Time | SqlType::Timestamp => "D",
        SqlType::Binary | SqlType::Json | SqlType::Unknown => "U",
    }
}

fn pg_tables(listing: &Listing) -> DFResult<RecordBatch> {
    let tables = &listing.tables;
    let n = tables.len();
    batch(vec![
        ("schemaname", texts(tables.iter().map(|table| table.schema))),
        ("tablename", texts(tables.iter().map(|table| &table.name))),
        ("tableowner", texts((0..n).map(|_| crate::SESSION_OWNER))),
        ("tablespace", null_texts(n)),
        ("hasindexes", bools(false, n)),
        ("hasrules", bools(false, n)),
        ("hastriggers", bools(false, n)),
        ("rowsecurity", bools(false, n)),
    ])
}

fn stat_user_tables(listing: &Listing) -> DFResult<RecordBatch> {
    let tables = &listing.tables;
    batch(vec![
        ("relid", ints(tables.iter().map(|table| table.oid))),
        ("schemaname", texts(tables.iter().map(|table| table.schema))),
        ("relname", texts(tables.iter().map(|table| &table.name))),
        (
            "n_live_tup",
            ints(tables.iter().map(|table| {
                table
                    .rows
                    .map_or(0, |rows| i64::try_from(rows).unwrap_or(i64::MAX))
            })),
        ),
    ])
}

#[expect(
    clippy::too_many_lines,
    reason = "one list of PostgreSQL's columns per empty catalog"
)]
/// A catalog of objects `RushDB` does not have (indexes as PostgreSQL models
/// them, constraints, defaults, comments, views): no rows, PostgreSQL's
/// columns.
fn empty(name: &str) -> DFResult<RecordBatch> {
    let columns: &[(&str, DataType)] = match name {
        "pg_description" => &[
            ("objoid", DataType::Int64),
            ("classoid", DataType::Int64),
            ("objsubid", DataType::Int64),
            ("description", DataType::Utf8),
        ],
        "pg_index" => &[
            ("indexrelid", DataType::Int64),
            ("indrelid", DataType::Int64),
            ("indnatts", DataType::Int64),
            ("indisunique", DataType::Boolean),
            ("indisprimary", DataType::Boolean),
            ("indisvalid", DataType::Boolean),
            (
                "indkey",
                DataType::List(Arc::new(Field::new_list_field(DataType::Int64, true))),
            ),
            ("indexprs", DataType::Utf8),
            ("indpred", DataType::Utf8),
        ],
        "pg_constraint" => &[
            ("oid", DataType::Int64),
            ("conname", DataType::Utf8),
            ("connamespace", DataType::Int64),
            ("contype", DataType::Utf8),
            ("conrelid", DataType::Int64),
            ("confrelid", DataType::Int64),
            (
                "conkey",
                DataType::List(Arc::new(Field::new_list_field(DataType::Int64, true))),
            ),
            (
                "confkey",
                DataType::List(Arc::new(Field::new_list_field(DataType::Int64, true))),
            ),
        ],
        "pg_attrdef" => &[
            ("oid", DataType::Int64),
            ("adrelid", DataType::Int64),
            ("adnum", DataType::Int64),
            ("adbin", DataType::Utf8),
        ],
        "pg_am" => &[
            ("oid", DataType::Int64),
            ("amname", DataType::Utf8),
            ("amtype", DataType::Utf8),
        ],
        "pg_enum" => &[
            ("oid", DataType::Int64),
            ("enumtypid", DataType::Int64),
            ("enumsortorder", DataType::Float64),
            ("enumlabel", DataType::Utf8),
        ],
        "pg_inherits" => &[
            ("inhrelid", DataType::Int64),
            ("inhparent", DataType::Int64),
            ("inhseqno", DataType::Int64),
        ],
        "pg_views" => &[
            ("schemaname", DataType::Utf8),
            ("viewname", DataType::Utf8),
            ("viewowner", DataType::Utf8),
            ("definition", DataType::Utf8),
        ],
        "pg_matviews" => &[
            ("schemaname", DataType::Utf8),
            ("matviewname", DataType::Utf8),
            ("matviewowner", DataType::Utf8),
            ("definition", DataType::Utf8),
        ],
        "views" => &[
            ("table_catalog", DataType::Utf8),
            ("table_schema", DataType::Utf8),
            ("table_name", DataType::Utf8),
            ("view_definition", DataType::Utf8),
        ],
        "table_constraints" => &[
            ("constraint_catalog", DataType::Utf8),
            ("constraint_schema", DataType::Utf8),
            ("constraint_name", DataType::Utf8),
            ("table_catalog", DataType::Utf8),
            ("table_schema", DataType::Utf8),
            ("table_name", DataType::Utf8),
            ("constraint_type", DataType::Utf8),
            ("is_deferrable", DataType::Utf8),
            ("initially_deferred", DataType::Utf8),
            ("enforced", DataType::Utf8),
        ],
        "key_column_usage" => &[
            ("constraint_catalog", DataType::Utf8),
            ("constraint_schema", DataType::Utf8),
            ("constraint_name", DataType::Utf8),
            ("table_catalog", DataType::Utf8),
            ("table_schema", DataType::Utf8),
            ("table_name", DataType::Utf8),
            ("column_name", DataType::Utf8),
            ("ordinal_position", DataType::Int64),
            ("position_in_unique_constraint", DataType::Int64),
        ],
        "constraint_column_usage" => &[
            ("table_catalog", DataType::Utf8),
            ("table_schema", DataType::Utf8),
            ("table_name", DataType::Utf8),
            ("column_name", DataType::Utf8),
            ("constraint_catalog", DataType::Utf8),
            ("constraint_schema", DataType::Utf8),
            ("constraint_name", DataType::Utf8),
        ],
        "referential_constraints" => &[
            ("constraint_catalog", DataType::Utf8),
            ("constraint_schema", DataType::Utf8),
            ("constraint_name", DataType::Utf8),
            ("unique_constraint_catalog", DataType::Utf8),
            ("unique_constraint_schema", DataType::Utf8),
            ("unique_constraint_name", DataType::Utf8),
            ("match_option", DataType::Utf8),
            ("update_rule", DataType::Utf8),
            ("delete_rule", DataType::Utf8),
        ],
        other => {
            return Err(DataFusionError::Internal(format!(
                "no catalog table {other}"
            )))
        }
    };
    let schema = Schema::new(
        columns
            .iter()
            .map(|(name, data_type)| Field::new(*name, data_type.clone(), true))
            .collect::<Vec<_>>(),
    );
    Ok(RecordBatch::new_empty(Arc::new(schema)))
}

/// What one catalog function answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Answer {
    /// Every listed object is usable by the session.
    True,
    /// No comment, default, expression or definition exists.
    Null,
    /// The schemas a search path names (`current_schemas`).
    Schemas,
    /// A type's name from its object id (`format_type`).
    TypeName,
    /// The owner's name (`pg_get_userbyid`).
    Owner,
    /// The encoding's name (`pg_encoding_to_char`).
    Encoding,
    /// `information_schema._pg_expandarray(keys)`: PostgreSQL expands a key
    /// array into `(x, n)` records. It is only called over `pg_constraint`
    /// and `pg_index` keys, which hold no rows here: a null record.
    Expand,
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct CatalogFunction {
    name: &'static str,
    answer: Answer,
    signature: Signature,
}

const FUNCTIONS: [(&str, Answer); 14] = [
    ("has_table_privilege", Answer::True),
    ("has_schema_privilege", Answer::True),
    ("has_any_column_privilege", Answer::True),
    ("has_column_privilege", Answer::True),
    ("pg_table_is_visible", Answer::True),
    ("pg_get_expr", Answer::Null),
    ("pg_get_indexdef", Answer::Null),
    ("obj_description", Answer::Null),
    ("col_description", Answer::Null),
    ("current_schemas", Answer::Schemas),
    ("format_type", Answer::TypeName),
    ("pg_get_userbyid", Answer::Owner),
    ("pg_encoding_to_char", Answer::Encoding),
    ("_pg_expandarray", Answer::Expand),
];

impl ScalarUDFImpl for CatalogFunction {
    fn name(&self) -> &str {
        self.name
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, _args: &[DataType]) -> DFResult<DataType> {
        Ok(match self.answer {
            Answer::True => DataType::Boolean,
            Answer::Schemas => {
                DataType::List(Arc::new(Field::new_list_field(DataType::Utf8, true)))
            }
            Answer::Null | Answer::TypeName | Answer::Owner | Answer::Encoding => DataType::Utf8,
            Answer::Expand => DataType::Struct(expanded_fields()),
        })
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> DFResult<ColumnarValue> {
        Ok(match self.answer {
            Answer::True => ColumnarValue::Scalar(ScalarValue::Boolean(Some(true))),
            Answer::Null => ColumnarValue::Scalar(ScalarValue::Utf8(None)),
            Answer::Owner => {
                ColumnarValue::Scalar(ScalarValue::Utf8(Some(crate::SESSION_OWNER.to_owned())))
            }
            Answer::Encoding => ColumnarValue::Scalar(ScalarValue::Utf8(Some("UTF8".to_owned()))),
            Answer::Expand => {
                ColumnarValue::Scalar(ScalarValue::try_from(&DataType::Struct(expanded_fields()))?)
            }
            Answer::Schemas => {
                // `current_schemas(true)` adds the implicit `pg_catalog`.
                let implicit = match args.args.first() {
                    Some(ColumnarValue::Scalar(ScalarValue::Boolean(Some(implicit)))) => *implicit,
                    _ => false,
                };
                let mut list = ListBuilder::new(StringBuilder::new());
                if implicit {
                    list.values().append_value("pg_catalog");
                }
                list.values().append_value("public");
                list.append(true);
                ColumnarValue::Scalar(ScalarValue::List(Arc::new(list.finish())))
            }
            Answer::TypeName => {
                let names: HashMap<i64, &str> = TYPES
                    .iter()
                    .map(|ty| (i64::from(ty.pg_oid()), ty.pg_name()))
                    .collect();
                let oids = match args.args.first() {
                    Some(value) => value.to_array(args.number_rows)?,
                    None => return Ok(ColumnarValue::Scalar(ScalarValue::Utf8(None))),
                };
                let oids = arrow::compute::cast(&oids, &DataType::Int64)?;
                let oids = arrow::array::as_primitive_array::<arrow::datatypes::Int64Type>(&oids);
                let type_names: StringArray = oids
                    .iter()
                    .map(|oid| oid.map(|oid| names.get(&oid).copied().unwrap_or("unknown")))
                    .collect();
                ColumnarValue::Array(Arc::new(type_names))
            }
        })
    }
}

/// PostgreSQL's `format(text, …)`: `%s` a value as text (null as empty),
/// `%I` an identifier quoted when it needs to be, `%L` a literal quoted
/// (null as `NULL`), `%%` a percent sign.
#[derive(Debug, PartialEq, Eq, Hash)]
struct Format {
    signature: Signature,
}

impl ScalarUDFImpl for Format {
    fn name(&self) -> &'static str {
        "format"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, _args: &[DataType]) -> DFResult<DataType> {
        Ok(DataType::Utf8)
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> DFResult<ColumnarValue> {
        let rows = args.number_rows;
        let columns = args
            .args
            .iter()
            .map(|arg| Ok(arrow::compute::cast(&arg.to_array(rows)?, &DataType::Utf8)?))
            .collect::<DFResult<Vec<ArrayRef>>>()?;
        let strings: Vec<&StringArray> = columns
            .iter()
            .map(|column| arrow::array::as_string_array(column))
            .collect();
        let Some((template, values)) = strings.split_first() else {
            return Err(DataFusionError::Plan("format needs a format string".into()));
        };
        let mut out = StringBuilder::new();
        for row in 0..rows {
            if template.is_null(row) {
                out.append_null();
                continue;
            }
            let value = |index: usize| {
                values
                    .get(index)
                    .and_then(|column| (!column.is_null(row)).then(|| column.value(row)))
            };
            out.append_value(format_row(template.value(row), value)?);
        }
        Ok(ColumnarValue::Array(Arc::new(out.finish())))
    }
}

fn format_row<'a>(template: &str, value: impl Fn(usize) -> Option<&'a str>) -> DFResult<String> {
    let mut out = String::new();
    let mut next = 0;
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let kind = chars.next();
        if kind == Some('%') {
            out.push('%');
            continue;
        }
        let argument = value(next);
        next += 1;
        match (kind, argument) {
            (Some('s'), argument) => out.push_str(argument.unwrap_or_default()),
            (Some('I'), Some(name)) => out.push_str(&quote_identifier(name)),
            (Some('I'), None) => {
                return Err(DataFusionError::Execution(
                    "null values cannot be formatted as an SQL identifier".into(),
                ))
            }
            (Some('L'), Some(literal)) => {
                out.push('\'');
                out.push_str(&literal.replace('\'', "''"));
                out.push('\'');
            }
            (Some('L'), None) => out.push_str("NULL"),
            (other, _) => {
                return Err(DataFusionError::Execution(format!(
                    "unrecognized format() type specifier \"{}\"",
                    other.map(String::from).unwrap_or_default()
                )))
            }
        }
    }
    Ok(out)
}

/// `name` as PostgreSQL's `quote_ident` writes it: bare when it is lowercase
/// letters, digits and underscores starting with a letter or underscore,
/// double-quoted otherwise.
fn quote_identifier(name: &str) -> String {
    let bare = name
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first == '_')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if bare {
        name.to_owned()
    } else {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
}

/// The fields of `_pg_expandarray`'s record: the element and its position.
fn expanded_fields() -> arrow::datatypes::Fields {
    arrow::datatypes::Fields::from(vec![
        Field::new("x", DataType::Int64, true),
        Field::new("n", DataType::Int64, true),
    ])
}

/// Register `pg_catalog`, `information_schema` and their functions on
/// `context`, over the schemas the session lists.
pub(crate) fn install(
    context: &SessionContext,
    labels: Arc<dyn SchemaProvider>,
    edges: Arc<dyn SchemaProvider>,
) -> DFResult<()> {
    let catalog = context
        .catalog("datafusion")
        .ok_or_else(|| DataFusionError::Internal("default catalog is unavailable".into()))?;
    let listings = Arc::new(Listings::new(labels, edges));
    catalog.register_schema(
        "pg_catalog",
        Arc::new(PgCatalogSchema {
            listings: Arc::clone(&listings),
        }),
    )?;
    catalog.register_schema(
        "information_schema",
        Arc::new(InformationSchema { listings }),
    )?;
    context.register_udf(ScalarUDF::new_from_impl(Format {
        signature: Signature::variadic_any(Volatility::Immutable),
    }));
    for (name, answer) in FUNCTIONS {
        context.register_udf(ScalarUDF::new_from_impl(CatalogFunction {
            name,
            answer,
            signature: Signature::variadic_any(Volatility::Stable),
        }));
    }
    Ok(())
}

/// PostgreSQL's own object ids of the catalogs a `'name'::regclass` names.
const CATALOG_OIDS: [(&str, i64); 9] = [
    ("pg_class", 1259),
    ("pg_type", 1247),
    ("pg_attribute", 1249),
    ("pg_proc", 1255),
    ("pg_namespace", 2615),
    ("pg_description", 2609),
    ("pg_constraint", 2606),
    ("pg_index", 2610),
    ("pg_attrdef", 2604),
];

/// `sql` as `DataFusion` can plan it when it uses PostgreSQL catalog
/// spellings: an unqualified catalog table is `pg_catalog`'s (PostgreSQL
/// searches `pg_catalog` first), a `pg_catalog.` qualified function call is
/// called by its name, `(record).field` reads the field,
/// `'pg_class'::regclass` is that catalog's object id, and a cast to an
/// object id type (`oid`, `regtype`, …) is a cast to `bigint`. `None` when
/// `sql` uses none of them or does not parse (it then runs as written).
pub(crate) fn compatible(sql: &str) -> Option<String> {
    use datafusion::sql::sqlparser::ast::VisitMut;
    use datafusion::sql::sqlparser::dialect::PostgreSqlDialect;
    use datafusion::sql::sqlparser::parser::Parser;

    let lowered = sql.to_ascii_lowercase();
    if !lowered.contains("pg_catalog.")
        && !lowered.contains("information_schema._pg_")
        && !lowered.contains("::reg")
        && !lowered.contains("::oid")
        && !TABLES.iter().any(|table| lowered.contains(table))
    {
        return None;
    }
    let mut statements = Parser::parse_sql(&PostgreSqlDialect {}, sql).ok()?;
    let mut rewriter = Compatible { changed: false };
    let _ = statements.visit(&mut rewriter);
    rewriter.changed.then(|| {
        statements
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    })
}

struct Compatible {
    changed: bool,
}

impl datafusion::sql::sqlparser::ast::VisitorMut for Compatible {
    type Break = ();

    fn pre_visit_relation(
        &mut self,
        relation: &mut datafusion::sql::sqlparser::ast::ObjectName,
    ) -> std::ops::ControlFlow<()> {
        use datafusion::sql::sqlparser::ast::{Ident, ObjectNamePart};
        if let [ObjectNamePart::Identifier(table)] = relation.0.as_slice() {
            if TABLES.contains(&table.value.to_ascii_lowercase().as_str()) {
                relation
                    .0
                    .insert(0, ObjectNamePart::Identifier(Ident::new("pg_catalog")));
                self.changed = true;
            }
        }
        std::ops::ControlFlow::Continue(())
    }

    fn post_visit_expr(
        &mut self,
        expr: &mut datafusion::sql::sqlparser::ast::Expr,
    ) -> std::ops::ControlFlow<()> {
        use datafusion::sql::sqlparser::ast::{
            AccessExpr, DataType as SqlDataType, Expr, Subscript, Value,
        };
        match expr {
            // `(record).field`, PostgreSQL's access to a computed record's
            // field, is `record['field']` here.
            Expr::CompoundFieldAccess { root, access_chain }
                if matches!(root.as_ref(), Expr::Nested(_)) =>
            {
                for access in access_chain.iter_mut() {
                    if let AccessExpr::Dot(Expr::Identifier(field)) = access {
                        *access = AccessExpr::Subscript(Subscript::Index {
                            index: Expr::value(Value::SingleQuotedString(field.value.clone())),
                        });
                        self.changed = true;
                    }
                }
            }
            Expr::Function(function) => {
                // `information_schema._pg_expandarray` is the one function
                // PostgreSQL keeps in `information_schema`.
                let qualified = function.name.0.len() == 2
                    && function.name.0[0].as_ident().is_some_and(|schema| {
                        schema.value.eq_ignore_ascii_case("pg_catalog")
                            || schema.value.eq_ignore_ascii_case("information_schema")
                    });
                if qualified {
                    function.name.0.remove(0);
                    self.changed = true;
                }
            }
            Expr::Cast {
                expr: inner,
                data_type,
                ..
            } => {
                let target = data_type.to_string().to_ascii_lowercase();
                if target == "regclass" {
                    let oid = match inner.as_ref() {
                        Expr::Value(value) => match &value.value {
                            Value::SingleQuotedString(name) => CATALOG_OIDS
                                .iter()
                                .find(|(catalog, _)| {
                                    name.trim_start_matches("pg_catalog.")
                                        .eq_ignore_ascii_case(catalog)
                                })
                                .map(|(_, oid)| *oid),
                            _ => None,
                        },
                        _ => None,
                    };
                    *expr = match oid {
                        Some(oid) => Expr::value(Value::Number(oid.to_string(), false)),
                        None => Expr::value(Value::Null),
                    };
                    self.changed = true;
                } else if matches!(
                    target.as_str(),
                    "oid" | "regtype" | "regproc" | "regprocedure" | "regnamespace" | "regrole"
                ) {
                    *data_type = SqlDataType::BigInt(None);
                    self.changed = true;
                }
            }
            _ => {}
        }
        std::ops::ControlFlow::Continue(())
    }
}
