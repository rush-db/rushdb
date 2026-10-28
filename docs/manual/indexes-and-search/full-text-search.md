# Full-text search

This page explains RushDB's keyword search: declaring a text index on a label,
choosing between `immediate` and `eventual` consistency, writing queries, and
reading BM25 scores. It is for developers adding search boxes, keyword
filtering, or the lexical half of [hybrid search](hybrid-search.md).

A text index is a declared index (`kind: "text"`) that shares the catalog,
naming and online build lifecycle of [ordered indexes](indexes.md). Its
inverted index is maintained by a background applier from a durable queue of
changed records.

## Declare a text index

A text index covers the records of **one label** and either **one property**
or, with `property: "*"`, **every string value of the record** (including
strings inside arrays and nested objects).

### Protocol

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "index",
      "index": {
        "kind": "create",
        "definition": {
          "name": "docs",
          "label": "Doc",
          "property": "*",
          "kind": "text",
          "consistency": "immediate"
        }
      }
    }
  }'
```

| Field | Required | Description |
|---|---|---|
| `name` | yes | Tenant-unique index name (shared namespace with ordered indexes). |
| `label` | yes | Label of the records to index. |
| `property` | yes | A property name, or `*` for every string value of the record. |
| `kind` | yes | `text`. |
| `consistency` | no | `eventual` (default) or `immediate`. Any other value is rejected. |

### Statement languages

Send these through `mutate.statement`:

| Language | Create |
|---|---|
| Cypher / GQL | `CREATE FULLTEXT INDEX name [IF NOT EXISTS] FOR (n:Label) ON EACH [n.prop] [OPTIONS {indexConfig: {consistency: 'immediate'}}]` |
| SQL | `CREATE INDEX name ON Label(prop) USING TEXT [IMMEDIATE]` |
| Mongo | `{"createIndex": {"collection": "Label", "property": "prop", "kind": "text", "name": "…", "consistency": "immediate"}}` |

```json
{
  "protocolVersion": 1,
  "operation": {
    "kind": "mutate",
    "mutation": {
      "kind": "statement",
      "language": "cypher",
      "statement": "CREATE FULLTEXT INDEX titles FOR (d:Doc) ON EACH [d.title] OPTIONS {indexConfig: {consistency: 'immediate'}}"
    }
  }
}
```

A text index covers exactly one property or `*`; `ON EACH [d.title, d.body]`
is rejected. To search several properties, declare an index with `*`, or one
index per property and run [hybrid](hybrid-search.md) or separate queries.
Text indexes are listed by `describe.indexes` and `SHOW INDEXES`, and dropped
with `index.drop` / `DROP INDEX` (see [Indexes](indexes.md)).

## Lifecycle and consistency

The new index starts in `building`. The builder walks the label's records in
batches and queues each one for the applier; when it reaches `ready`, searches
are accepted. Searching an index that is not ready fails with
``text index `<name>` is building`` (or `dropping`).

After that, every commit that creates, changes, relabels or deletes a covered
record queues the record in the same commit. The applier re-reads the
record's current text, replaces its document in the inverted index, and then
removes the queue entries it consumed. A change committed after the applier
took its snapshot stays queued, so no update is lost.

| `consistency` | When a search sees a committed write | Cost |
|---|---|---|
| `eventual` (default) | After the background applier processes it. The applier runs continuously with a linger of at most 250 ms. | Writes and searches never wait for indexing. |
| `immediate` | Always: a search first applies every queued change up to its snapshot (read-your-writes). | The first search after a burst of writes pays for indexing them. |

> **Note:** An `immediate` search applies at most 32,768 queued changes
> before it runs. After an extremely large write burst, a search may run
> before the rest are applied; they are applied by later searches and by the
> background applier.

`/metrics` exposes the backlog of every text index as the
`rushdb_text_pending{index="…"}` gauge (see
[Metrics](../reference/metrics.md)). In multitenant mode the gauge sums
indexes of the same name over the resident tenants. Each search also
reports its index's backlog as `pending`. Text indexes never hold back WAL
retention, and an index with nothing queued never commits.

## Search

`query.text` returns the top BM25 matches of one text index.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `index` | string | yes | — | Text index name. |
| `query` | string | yes | — | Query string (syntax below). An empty or blank query returns no hits. |
| `limit` | integer | no | 10 | Maximum hits. `0` returns no hits. |
| `allTerms` | boolean | no | `false` | Unmarked terms must all match (AND) instead of any (OR); explicit `OR`, `+` and `-` still apply. |
| `phrase` | boolean | no | `false` | Match the whole query as one phrase: the terms adjacent and in order. The [query syntax](#query-syntax) is not interpreted. |

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "operation": {
      "kind": "query",
      "query": { "kind": "text", "index": "docs", "query": "quick fox", "limit": 10 }
    }
  }'
```

```json
{
  "protocolVersion": 1,
  "response": {
    "kind": "textHits",
    "total": 1,
    "pending": 0,
    "rows": [
      { "id": "0192f0c4-7a1e-7b8a-9c3d-2f6e5a4b3c21", "score": 0.5753642 }
    ]
  }
}
```

Each row holds the record `id` and its `score`; `total` is the number of rows
returned. `pending` is the number of changes not yet applied to the index
when the search ran: an `eventual` index may miss up to that many recent
writes (an `immediate` index reports `0` unless a write burst exceeded its
apply bound). Fetch the records themselves with `query.records` and an `$id`
filter, or use [hybrid search](hybrid-search.md), which returns full records
and can apply a `predicate`.

> **Note:** `query.text` takes no `labels` or `predicate` and no `offset`.
> To filter or page lexical results, use `query.hybrid` with only a `text`
> leg.

`query.text` is unavailable when a data-access policy is enforced on the
tenant; `query.hybrid` remains available and applies the policy to every
returned record.

### From statement languages

Every statement language searches the same text indexes. A statement sees
the index as applied when it runs (an `immediate` index first applies its
pending changes); what the statement itself writes is not searchable until it
commits. Under an enforced data-access policy, statements are refused, as
for every statement.

| Language | Search | Example |
|---|---|---|
| SQL | `text_search(index, query [, limit [, options]])`, a table of `(id, score)` | `SELECT d.title, t.score FROM text_search('docs', 'quick fox', 20) AS t JOIN doc d ON d.__id = t.id ORDER BY t.score DESC` |
| Cypher, GQL | `text_match(n.prop, 'query')` (boolean) and `text_score(n.prop, 'query')` (BM25 score) | `MATCH (d:Doc) WHERE text_match(d.title, 'fox') RETURN d.title ORDER BY text_score(d.title, 'fox') DESC LIMIT 10` |
| Mongo | `{"$text": {"$search": "..."}}` in a `find` filter or the first `$match`; `{"$meta": "textScore"}` in `projection`/`sort` | `{"collection": "Doc", "filter": {"$text": {"$search": "fox"}}, "sort": {"score": {"$meta": "textScore"}}}` |

- **SQL:** `limit` defaults to 10; `options` is JSON text,
  `'{"allTerms": true}'` or `'{"phrase": true}'`. Arguments must be literals.
  A bad call (unknown index, wrong arguments, unparsable query) fails with
  `invalid_request`.
- **Cypher, GQL:** the property names the index: `text_match(d.title, …)`
  uses the `ready` text index on `title` of the record's label, and
  ``d.`*` `` the index over every string. `ORDER BY text_score(…) DESC LIMIT k`
  reads only the top `k` from the index. A property with no ready text index,
  or an unparsable query, fails with `invalid_request`.
- **Mongo:** the collection's one text index answers `$text` (MongoDB allows
  one per collection; a label with several is refused, as is one with none).
  The rest of the filter, `skip`, `limit` and `$group` apply to the matches.
  `$caseSensitive` and `$diacriticSensitive` must be `false` (analysis is
  fixed), `$language` is ignored, and sorting a pipeline by `textScore` is
  refused.

### Text analysis

Analysis is fixed; there are no configurable analyzers.

| Step | Behaviour |
|---|---|
| Tokenization | Text is split on every character that is not a letter or digit. |
| Long tokens | Tokens longer than 40 bytes are dropped. |
| Case | Tokens are lower-cased; search is case-insensitive. |
| Stemming, stop words | None. `running` does not match `run`; `the` is indexed. |
| Values | Only string values are indexed. With `*`, every string in the record (including array elements and nested objects) is joined into one document. |

The query string goes through the same analysis.

### Query syntax

| Syntax | Meaning | Example |
|---|---|---|
| `a b` | Any term may match (OR); records matching more terms rank higher. | `quick fox` |
| `"a b"` | Phrase: the terms must be adjacent, in order. | `"brown fox"` |
| `+a` | `a` must match. | `+graph database` |
| `-a` | `a` must not match. | `graph -relational` |
| `a AND b`, `a OR b`, `NOT a` | Boolean operators (upper case). | `graph AND NOT sql` |
| `( … )` | Grouping. | `(graph OR network) AND storage` |

Characters such as `:`, `^`, `*`, `[`, `]`, `{`, `}`, `!` and `\` have
special meaning in the parser; wrap terms containing them in double quotes.
A query that cannot be parsed fails with `execution_failed`.

### Scoring

Hits are ranked by BM25 with tantivy's default parameters, using per-index
term statistics. Scores are only comparable **within one result set**; do not
compare scores across queries or indexes, and do not combine them with vector
scores directly (use [hybrid search](hybrid-search.md), which fuses ranks, not
scores). Equal scores are ordered deterministically by record.

## Storage

Each text index keeps its inverted index in files under the data directory
(`<data-dir>/text/<tenant>/<index-id>/`); in-memory stores keep it in RAM.
These files are derived from the records and the durable change queue. They
are deleted when the index finishes dropping or the tenant is purged. A node
keeps at most 256 on-disk text indexes open and reopens an idle one from its
files when it is searched again; an in-memory store keeps every index open,
since the open index is its only copy.

## See also

- [Hybrid search](hybrid-search.md)
- [Indexes](indexes.md)
- [Text internals](../architecture/text-internals.md)
- [Metrics](../reference/metrics.md)
- [Cypher](../languages/cypher.md)
- [SQL](../languages/sql.md)
- [Mongo](../languages/mongo.md)
