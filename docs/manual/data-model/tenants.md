# Tenants

This page explains how RushDB separates data into tenant keyspaces: the
difference between single-tenant and multitenant nodes, how each request
selects its tenant on every protocol, what isolation you get, and how to
remove a tenant's data. It is for developers building multi-customer
applications and for operators choosing a deployment mode. The internal
design is covered in [Multitenancy](../architecture/multitenancy.md).

## Two modes

A node runs in exactly one tenancy mode, chosen when its data directory is
created:

| Mode | Start with | Requests | Use it when |
|---|---|---|---|
| Single-tenant (default) | `rushdb serve` / `rushdb up` | Carry no tenant scope; all data lives in one keyspace | One application owns the node |
| Multitenant | `rushdb serve --multitenant` | Must carry a tenant scope; each scope is its own keyspace | Many customers, projects or environments share one node |

The mode is recorded in the directory's `config.yml` (`tenancy.mode`) and is
an **isolation invariant**: a directory created in one mode refuses to start
in the other. To change modes, start a fresh directory and move the data.

## What a tenant is

A tenant is identified by its **scope**, an opaque string you choose — a
customer id, a project id, `staging`. In multitenant mode:

- **There is nothing to create.** The first request that names a new scope
  registers it.
- **Every key is tenant-prefixed.** Records, relationships, properties, the
  schema catalog, indexes, vector and text indexes, and transactions all live
  inside the tenant's keyspace. A query can never see another tenant's data,
  and the catalog of one tenant never mentions another tenant's labels.
- **No tenant property is stored on records.** The keyspace itself is the
  boundary, so your records do not carry a `tenantId` field, and you cannot
  forget to filter on one.
- **Scopes are case- and byte-exact.** `Acme` and `acme` are two tenants.
  Scope values are limited to 1,024 bytes on HTTP.

## Selecting the tenant per request

In multitenant mode every request must present a scope through its
transport. A missing scope is refused before any data is touched.

| Protocol | Where the scope goes |
|---|---|
| HTTP | The `X-RushDB-Tenant` header **and** `context.scope` in the envelope; both must be equal |
| PostgreSQL wire | The `database` connection parameter |
| gRPC | The `x-rushdb-tenant` metadata entry |
| Bolt | The `db` field of `HELLO` |

An HTTP request in multitenant mode:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H 'X-RushDB-Tenant: acme' \
  -H 'Content-Type: application/json' \
  --data '{
    "protocolVersion": 1,
    "context": { "scope": "acme" },
    "operation": { "kind": "query", "query": { "kind": "records",
      "query": { "labels": ["Invoice"], "limit": 10 } } }
  }'
```

A PostgreSQL client selects the tenant with the database name:

```bash
PGPASSWORD="$RUSHDB_TOKEN" psql "host=127.0.0.1 port=5432 user=rushdb dbname=acme sslmode=disable"
```

Failures are explicit:

| Situation | Result |
|---|---|
| Multitenant, no `X-RushDB-Tenant` header | `missing_scope`, HTTP 403 |
| Multitenant, header present but no `context.scope` | `missing_scope`, HTTP 403 |
| Multitenant, `context.scope` differs from the header | `scope_mismatch`, HTTP 403 |
| Single-tenant, `context.scope` present | `scope_mismatch`, HTTP 403 |
| Single-tenant, gRPC `x-rushdb-tenant` or Bolt `HELLO db` set | `scope_mismatch` (gRPC `UNAUTHENTICATED`; Bolt statement failure) |
| PostgreSQL wire, multitenant, no database name | Connection refused (SQLSTATE `3D000`) |

In single-tenant mode, omit tenant scopes everywhere: leave `context.scope`
out of envelopes and do not send tenant metadata on gRPC or a `db` value on
Bolt (those are refused, never served as a second keyspace). The HTTP
`X-RushDB-Tenant` header and the PostgreSQL database name are ignored in this
mode.

## Credentials and tenants

The node's bootstrap token is an **operator** credential: it is accepted for
every tenant. Anyone who holds it can name any scope. Two consequences:

- For clients that should reach only their own tenant, use **signed
  capability tokens** on HTTP. A capability carries a `read` or `write`
  permission and is bound to exactly one tenant scope; presenting it with a
  different `X-RushDB-Tenant` value is refused with HTTP 401. See
  [Authentication](../connectivity/authentication.md).
- The PostgreSQL wire, gRPC and Bolt listeners accept only the bootstrap
  token. On a multitenant node they therefore refuse to bind to a
  non-loopback address; reach them through a trusted local proxy that
  enforces tenant identity, or use a single-tenant node.

## Fairness and resource limits

Tenants share one process, so the node offers per-tenant guards:

- `--max-concurrent-requests-per-tenant` caps how many requests one tenant can
  run at once.
- `--tx-max-staged-bytes-per-tenant` caps how much transaction staging memory
  one tenant can claim.
- Idle tenants' in-memory state (such as loaded vector indexes) is released
  after `tenant-idle-seconds` and rebuilt on the next request. When every
  resident tenant is active and the residency budget is full, a request for a
  new tenant gets a retryable `unavailable` (HTTP 503) error.

Commits are serialized per node: a busy tenant's writes share the write path
with everyone else's. Batch writes for throughput rather than sending many
small concurrent writes. See
[Resource governance](../architecture/resource-governance.md) and
[Capacity planning](../operations/capacity-planning.md).

## Removing a tenant

`purgeTenant` erases every record, relationship and index entry of the
request's tenant:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H 'X-RushDB-Tenant: acme' \
  -H 'Content-Type: application/json' \
  --data '{"protocolVersion":1,"context":{"scope":"acme"},"operation":{"kind":"purgeTenant"}}'
```

What to expect:

- The purge runs as a sequence of bounded commits, so it scales to very large
  tenants. Each step is atomic, but the purge as a whole is not one
  transaction: a concurrent reader may see partially erased data while it
  runs.
- The tenant's security policy is kept; only data is erased.
- If the same scope is used again later, it gets a fresh, empty keyspace;
  leftovers of the purged tenant can never reappear in it.
- Disk space is returned by background compaction, not immediately.

> **Warning:** `purgeTenant` is irreversible. Take a tenant backup first if
> you may need the data; see [Backup and restore](../operations/backup-and-restore.md).

## Backups per tenant

The `backup` operation exports the requesting tenant's graph. On a
multitenant node a request may only back up its own scope; an empty `scopes`
list means "this tenant", never "every tenant". Whole-node exports are an
offline operator task. See [Backup and restore](../operations/backup-and-restore.md).

## See also

- [Multitenancy architecture](../architecture/multitenancy.md)
- [Authentication](../connectivity/authentication.md)
- [Security](../operations/security.md)
- [Configuration](../operations/configuration.md)
