# Upgrading from RushDB 2.x

RushDB 3 is a new, standalone database engine, not an in-place upgrade of
RushDB 2.x. This page explains what changes, how to plan the move, and how
to move data. It also covers upgrades between 3.x releases.

## What changed

| | RushDB 2.x | RushDB 3 |
|---|---|---|
| Architecture | Application platform on top of Neo4j, with Core REST routes and managed services | A single database engine binary (`rushdb`) with its own storage |
| Storage | Neo4j store | RushDB's own store and write-ahead log in a data directory |
| API | Core REST routes, SDKs | Protocol v1 over HTTP, gRPC, and Bolt; PostgreSQL wire for SQL; optional query languages |
| Tenancy | Application-level project isolation | Engine-level tenants selected per request by scope |

RushDB 3 does **not** read a 2.x data directory or a Neo4j store, and it does
not accept 2.x Core REST requests. RushDB 2.x remains available at its final
`2.x` maintenance branch in the `rush-db/rushdb` repository.

## Planning the migration

Treat the move as an application migration, in three tracks:

1. **Keep 2.x running.** Pin the 2.x deployment to its supported tag while
   you export the data and the relationship meaning you need to keep.
2. **Map and validate the data.** Map your data to RushDB 3 records,
   labels, typed relationships, and properties. In a disposable 3.x
   database, validate:
   - record identifiers you depend on;
   - null and array handling;
   - label choices;
   - fields that feed vector indexes;
   - tenant boundaries (one 2.x project typically becomes one tenant in a
     multitenant 3.x server).
3. **Move callers.** In RushDB 3 the engine is what you self-host; the
   dashboard, the REST API with API keys, managed embeddings and the AI
   features (natural-language search, relationship-pattern suggestions) are
   part of [RushDB Cloud](https://app.rushdb.com). SDK and MCP clients of a
   self-hosted 2.x deployment use the SDKs' direct mode against this server
   (3.x SDKs, `direct: true` and `engineUrl`); they supply their own
   embeddings through external vector indexes. Clients that need the RushDB
   Cloud features move to RushDB Cloud. Clients that talk to the database
   directly port to the native protocol or a query language.

## Moving the data

1. Create the target server with the tenancy mode you need. The mode cannot
   be changed later:

   ```bash
   rushdb serve --data-dir /var/lib/rushdb --multitenant --init /var/lib/rushdb
   ```

2. Export your 2.x data as JSON documents (one export per project).
3. Import each project into its tenant with the `ingest` operation, which
   turns nested JSON into records and relationships. Send batches of about
   5,000 documents per request (larger batches load no faster and hold
   more memory):

   ```bash
   curl -s -H "Authorization: Bearer $RUSHDB_TOKEN" \
     -H "X-RushDB-Tenant: project-42" -H 'Content-Type: application/json' \
     -d '{"protocolVersion":1,"context":{"scope":"project-42"},
          "operation":{"kind":"ingest","ingest":{"kind":"documents","label":"Customer",
          "documents":[{"name":"Acme","orders":[{"total":120}]}]}}}' \
     http://127.0.0.1:3000/protocol/v1/execute
   ```

   See [Nested import](../data-model/nested-import.md) for how documents map
   to records.
4. Re-create relationships that are not expressed by nesting with the
   `linkRecords*` mutations, and declare the indexes your queries need.
5. Re-create vector indexes and upload embeddings.
6. Compare record counts per label between 2.x and 3.x, then run your
   application's test suite against the 3.x server before cutover.

## Upgrading between 3.x releases

- **Binary upgrade.** Stop the server cleanly, replace the `rushdb`
  binary, and start again. Take a base backup first.
- **Storage format.** The storage format version is recorded in
  `rushdb.redb.layout.json`. A release opens stores of its own format and
  migrates the earlier 3.x formats it knows at its first start, once: 3.0.14
  (format 7) rewrites the datetimes of a 3.0.13 or earlier store from epoch
  milliseconds to microsecond instants, and a start interrupted part way
  finishes at the next one. Once migrated, a store is refused by earlier
  releases, so take the base backup before upgrading if you may roll back.
  A store in a format the binary does not know is refused at startup with
  `unsupported storage format version N; this build uses version M`. In
  that case, use the **old** binary to write a graph snapshot (`--export`),
  then the **new** binary to restore it into a new data directory
  (`--restore`), and re-declare indexes and vectors. See
  [Backup and restore](backup-and-restore.md#graph-snapshot-export-and-restore).
- **Configuration.** `config.yml` carries a schema `version`; a mismatch is
  refused with `config version mismatch`. New settings that are absent from
  an existing file take their defaults. A setting a later release removed is
  ignored with a warning (``ignoring setting `KEY`: it was removed in
  VERSION``), in the file and as a flag or variable; delete it when
  convenient. Any other unknown key is refused.
- **Release notes.** Read the changelog for each release you skip.

## See also

- [Backup and restore](backup-and-restore.md)
- [Nested import](../data-model/nested-import.md)
- [Tenants](../data-model/tenants.md)
- [HTTP API](../connectivity/http-api.md)
