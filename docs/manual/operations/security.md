# Security

How RushDB authenticates clients, isolates tenants, protects traffic, and
which settings to check before exposing a server. Client-side details for
each protocol are in [Authentication](../connectivity/authentication.md).

## Credentials

RushDB has two kinds of credentials.

| Credential | Set with | Accepted on | Grants |
|---|---|---|---|
| **Boot token** | `--token` / `RUSHDB_TOKEN` (default `local-token`) | HTTP, pgwire (as the password), gRPC, Bolt, replication endpoints | Full access to every tenant. |
| **Capability token** | issued by your application, verified with `--capability-secret` | HTTP only | The listed permissions, optionally bound to one tenant, until it expires. |

> **Warning:** The default boot token is `local-token`. Always set your own
> long random token for anything beyond a local experiment:
> `RUSHDB_TOKEN=$(openssl rand -hex 32)`. An empty token is refused at
> startup, and so is the default token on a non-loopback `--listen` address
> (with or without TLS).

The boot token is an operator credential. It is compared in constant time,
is never written to `config.yml`, and should be passed through an
environment file with restrictive permissions rather than on the command
line (where other local users can read it from the process list).

### Presenting the token over HTTP

Either header works; `X-RushDB-Token` is checked first:

```bash
curl -s -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"protocolVersion":1,"operation":{"kind":"describe","describe":{"kind":"capabilities"}}}' \
  http://127.0.0.1:3000/protocol/v1/execute
```

The server checks credentials from the headers **before** it reads the
request body, so unauthenticated clients cannot make it buffer large bodies.

### Capability tokens

Set a secret of at least 16 bytes, hex-encoded:

```bash
RUSHDB_CAPABILITY_SECRET=$(openssl rand -hex 32)
```

A capability token is `base64url(claims) + "." + base64url(signature)`,
without padding, where the signature is HMAC-SHA256 over the base64url
claims string using the decoded secret. Claims are a JSON object:

| Claim | Type | Meaning |
|---|---|---|
| `sub` | string | Subject (who the token was issued to). |
| `scope` | string | Space-separated permissions: `read`, `write`, or `*` for both. `write` does not imply `read`; issue `read write` for a client that does both. |
| `tenant` | string | The tenant scope this token may use. Required in multitenant mode (the token must match `X-RushDB-Tenant` exactly); must be absent on a single-tenant server. |
| `iat` | integer | Issued-at, Unix seconds. |
| `exp` | integer | Expiry, Unix seconds. Expired tokens are refused. |

Clients send the capability in `X-RushDB-Token` (or as a bearer token).
Which permission an operation needs:

| Permission | Operations |
|---|---|
| `read` | `query`, `describe`, `backup`, and `GET /metrics` |
| `write` | `mutate`, `ingest`, `index`, `transaction`, `purgeTenant` |

A failed check returns HTTP 401. Rotating `--capability-secret` invalidates
every outstanding capability. Capabilities are HTTP-only; pgwire, gRPC, and
Bolt accept only the boot token.

## Tenant isolation

In multitenant mode (`--multitenant`) every request must select a tenant:

| Transport | Scope comes from |
|---|---|
| HTTP | `X-RushDB-Tenant` header (and `context.scope` in the envelope must match it) |
| PostgreSQL wire | `database` connection parameter |
| gRPC | `x-rushdb-tenant` metadata |
| Bolt | HELLO `db` field |

A missing scope is refused, never defaulted. Each tenant is a separate
keyspace; no tenant identifier is stored on records. The mode is fixed when
the data directory is created. See [Tenants](../data-model/tenants.md).

Because pgwire, gRPC, and Bolt authenticate only with the boot token, any
holder of that token can select any tenant on them. RushDB therefore
refuses to start them on a non-loopback multitenant listener (see below).
For tenant-facing traffic, expose HTTP with tenant-bound capabilities, or
put a trusted proxy in front that enforces tenant identity.

## Network exposure

All listeners bind `--listen` (default `127.0.0.1`). The startup guard:

| Listen address | HTTP | pgwire / gRPC / Bolt (any non-zero port) |
|---|---|---|
| Loopback (`127.x.x.x`, `::1`, `localhost`) | allowed | allowed |
| Non-loopback, single-tenant | requires TLS **or** `--allow-insecure-listen` | requires `--allow-insecure-listen` |
| Non-loopback, multitenant | requires TLS **or** `--allow-insecure-listen` | **refused**, even with `--allow-insecure-listen` |

The check counts only listeners that will actually start, on the effective
configuration (after `config.yml`): pgwire when `sql` is enabled, Bolt when
`cypher` is enabled, and gRPC whenever its port is set. To expose HTTPS on a
non-loopback address without the insecure flag, keep those listeners off
(for example `--pgwire-port 0` when `sql` is enabled).

## TLS

Only the HTTP listener supports TLS:

```bash
rushdb serve --data-dir /var/lib/rushdb --listen 0.0.0.0 \
  --tls-cert /etc/rushdb/tls/fullchain.pem \
  --tls-key /etc/rushdb/tls/privkey.pem \
  --pgwire-port 0
```

- `--tls-cert` is a PEM certificate chain; `--tls-key` a PEM private key in
  PKCS#8 form. Both must be set, or startup fails.
- The files are read at startup; restart to rotate certificates.
- PostgreSQL wire, gRPC, and Bolt are cleartext. The PostgreSQL listener
  declines TLS negotiation, so clients must use `sslmode=disable` or
  `sslmode=prefer`. To encrypt those protocols across hosts, tunnel them
  (for example a TLS-terminating proxy or an SSH or WireGuard tunnel).

## Data-access policies

Beneath the tenant boundary, a tenant can carry a data-access policy that
limits which labels can be read and written, which relationship types can be
traversed, and which properties are visible. Install or read it with the
`policy` operation, which only the boot token may call (a capability token
can never change the policy that limits it):

```json
{"protocolVersion":1,"context":{"scope":"acme"},
 "operation":{"kind":"policy","policy":{"kind":"put","enforced":true,"policy":{
   "allowedLabels":["Note"],
   "deniedRelTypes":["AUDITED_BY"],
   "visibleProperties":{"Note":["title","createdAt"]},
   "maxResults":1000}}}}
```

| Field | Meaning |
|---|---|
| `allowedLabels` | Labels that can be read and written, compared case-insensitively. `["*"]` allows every label, including labels created later. An empty list allows none: an enforced policy with `"allowedLabels": []` locks the tenant's data (reads fail with `unauthorized` or answer empty, writes fail with `unauthorized`). |
| `deniedRelTypes` | Relationship types readers cannot traverse. |
| `visibleProperties` | Per label, the only properties a reader sees (an allow-list). A label without an entry shows every property. |
| `maxResults` | Most rows a query may return (0 = no cap). |
| `minCount` | Smallest group a catalog read reports (0 = no threshold). Under an enforced policy, `describe.*`, `propertyValues` and `searchValues` withhold labels, properties, values, relationship shapes and value overlap counted over fewer records, and drop sample, minimum and maximum values (often one record's value). It does not apply to record reads or SQL rows, which return records the policy already lets the reader see. |
| `enforced` | `false` keeps tenant isolation only and filters nothing. |

To hide columns or apply `minCount` without restricting labels, allow every
label and keep the other fields:

```json
{"allowedLabels":["*"],"visibleProperties":{"Patient":["age","sex"]},"minCount":10}
```

Each `put` is a new policy generation; a transaction begun under the previous
generation aborts at commit. `{"kind":"get"}` returns the active policy, its
`generation` and `enforced`. While a policy is enforced, the catalog and
SQL see only what it allows: `describe` `schema`, `labels`, `properties`,
`schemaSummary`, `names` and `valueOverlap`, `propertyValues` and
`searchValues` answer over the allowed labels, their visible properties and
relationship shapes of allowed types between allowed labels, and an SQL
`SELECT` sees a table per allowed label with its visible columns and an edge
table per allowed type (see [SQL](../languages/sql.md#under-a-data-access-policy)).
A hidden label, property or type is never named, counted or suggested. Read
paths that cannot apply a policy still fail closed with `unauthorized`
(`policy-enforced reads are unavailable for this adapter`): Cypher, GQL,
Gremlin, SQL/PGQ and Mongo statements, `joinPairs`, `vectorIndexCandidates`,
`text`, `backup` and `describe.explain`.

## Replication endpoints

`/internal/replication/wal` and `/internal/replication/promote` accept only
the boot token (not capabilities). A follower authenticates to its leader
with `--replica-token`, which defaults to the follower's own `--token`. Keep
these endpoints off untrusted networks.

## Backups and secrets

Backups contain data but no secrets: neither the boot token nor the
capability secret is stored in the data directory. Base backups include
`config.yml`, which holds no secrets. Snapshot and archive files are not
encrypted by RushDB; encrypt them at rest with your own tooling.

## Hardening checklist

- [ ] Set a long random `RUSHDB_TOKEN`; never use `local-token` outside a
      laptop.
- [ ] Pass secrets through a root-owned `0600` environment file, not command
      line flags.
- [ ] Keep `--listen 127.0.0.1` unless clients on other hosts need direct
      access; prefer a local proxy.
- [ ] If you expose HTTP, enable TLS; do not use `--allow-insecure-listen` on
      untrusted networks.
- [ ] For multitenant deployments, give tenant-facing clients capability
      tokens bound to their tenant with only the permissions they need and
      short expiry; keep the boot token server-side.
- [ ] Enable only the languages you use (`--languages`; SQL, Gremlin,
      SQL/PGQ and MongoDB are on by default); pgwire and Bolt bind only when
      their languages are enabled.
- [ ] Leave gRPC and Bolt ports at 0 unless needed.
- [ ] Run as a dedicated user with the data directory mode `0750`; use the
      systemd hardening options in [Deployment](deployment.md).
- [ ] Restrict network access to the replication endpoints to the follower
      hosts.
- [ ] Encrypt backup and WAL archive storage.
- [ ] Set `--request-timeout-ms` and, for multitenant servers,
      `--max-concurrent-requests-per-tenant` so one client cannot monopolize
      the server.

## See also

- [Authentication](../connectivity/authentication.md)
- [Tenants](../data-model/tenants.md)
- [Deployment](deployment.md)
- [Configuration reference](../reference/configuration-reference.md#security-and-tenancy)
