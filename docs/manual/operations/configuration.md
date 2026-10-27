# Configuration

How `rushdb serve` combines command-line flags, environment variables, and the
data directory's `config.yml`, and how to change settings safely. Every
individual key is listed in the
[configuration reference](../reference/configuration-reference.md).

## Three sources, one source of truth

| Source | Example | Persisted? |
|---|---|---|
| Command-line flag | `--storage-cache 256MiB` | no |
| Environment variable | `RUSHDB_STORAGE_CACHE=256MiB` | no |
| `config.yml` in the data directory | `resources:` → `storage-cache: 256MiB` | yes |

A flag and its environment variable are the same setting; a flag on the
command line wins over the variable. The interesting part is how they
combine with `config.yml`:

1. **First start of an empty data directory.** The effective settings from
   flags, environment, and defaults are written to `config.yml`.
2. **Every later start.** `config.yml` is the source of truth. Settings you
   do not pass explicitly are taken from the file, not from the defaults.
3. **Disagreement is an error.** If you pass a flag or variable explicitly
   and its value differs from the stored one, the server refuses to start
   and names both values:

   ```text
   server bootstrap: config.yml disagrees with explicitly supplied startup flags
   (--storage-cache (flag 268435456, config.yml 134217728)); config.yml is the
   source of truth for this data directory: edit it or drop the flag
   ```

   Passing the same value as the file is fine.

So to change a setting on an existing data directory, **edit `config.yml`
and restart**. Nothing is overridden silently.

### Settings that are never stored

| Setting | Why |
|---|---|
| `--token`, `--capability-secret` | Secrets are never written to disk. Supply them on every start (environment file recommended). |
| `--data-dir` | Needed to find the file. (`server.data-dir` in the file is informational.) |
| `--allow-insecure-listen`, `--request-timeout-ms`, `--max-concurrent-requests-per-tenant`, `--raft`, `--experimental-raft` | Per-boot switches. |

### Settings that cannot change

- **Tenancy mode.** `tenancy.mode` (`single` or `multitenant`) is fixed for
  the life of a data directory. Starting with a different mode fails with
  `data directory tenancy mismatch`. To change it, export the data and
  restore it into a new directory created with the new mode.
- **`version`.** The file's schema version must be `1`.
- **Shard count.** The store's layout records the shard count; a different
  `shards` value is refused.

### An existing directory without `config.yml`

A data directory that already contains commits but no `config.yml` is
refused (`existing data directory has no config.yml`). Recreate the record
with `--init` using the settings the directory was created with.

## The `config.yml` file

The generated file is commented and lists every option. Its sections:

```yaml
version: 1
server:
  port: 3000
  listen-addr: 127.0.0.1
  # tls-cert: (TLS disabled)
  # tls-key: (TLS disabled)
  pgwire-port: 5432
  grpc-port: 0
  bolt-port: 0
  body-limit: 16777216B
  maintenance: false
  data-dir: "/var/lib/rushdb"
tenancy:
  mode: single
resources:
  managed-limit: 536870912
  storage-cache: 134217728
  # ... every resource key, see the reference
transactions:
  max-active: 128
  max-operations: 10000
  # ...
languages:
  sql: false
  gql: false
  cypher: false
  gremlin: false
  sql-pgq: false
  mongodb: false
```

- Byte values are written as plain byte counts; you may use `B`, `KiB`,
  `MiB`, or `GiB` strings when editing.
- `resources` and `transactions` reject unknown keys, so a misspelled key
  stops the server with a parse error instead of being ignored.
- `server.require-index-warmup: true` may be added by hand; it is not
  written by default.
- `server.body-limit` exists only in the file (no flag); it caps request
  bodies, gRPC messages, and responses.

Preview the file a set of flags would produce without touching anything:

```bash
rushdb serve --data-dir /var/lib/rushdb --multitenant --languages sql,cypher --print-config
```

## Changing a setting

```bash
sudo systemctl stop rushdb
sudoedit /var/lib/rushdb/config.yml     # e.g. resources: storage-cache: 256MiB
sudo systemctl start rushdb
journalctl -u rushdb -n 20
```

Settings are read at startup only; there is no live reload.

## Common setups

### Local development

```bash
RUSHDB_TOKEN=dev rushdb up            # data in ./.rushdb, HTTP on 127.0.0.1:3000
```

### Single-tenant service with SQL and Cypher on one host

```bash
rushdb serve --data-dir /var/lib/rushdb --languages sql,cypher
```

HTTP on 3000 and pgwire on 5432, both on loopback. Put your application on
the same host or behind a local proxy.

### Multitenant service behind an application proxy

```bash
rushdb serve --data-dir /var/lib/rushdb --multitenant \
  --max-concurrent-requests-per-tenant 32
```

Every request must carry `X-RushDB-Tenant`. Keep the listener on loopback
and let the proxy authenticate end users. See
[Tenants](../data-model/tenants.md).

### HTTPS on a private network

```bash
rushdb serve --data-dir /var/lib/rushdb --listen 0.0.0.0 \
  --tls-cert /etc/rushdb/tls/fullchain.pem --tls-key /etc/rushdb/tls/key.pem \
  --pgwire-port 0
```

`--pgwire-port 0` is needed only when the `sql` language is enabled: an
enabled pgwire listener is cleartext on a non-loopback address (see
[Security](security.md#network-exposure)).

### Bulk import window

For a one-off import you can re-run from its source, `async` durability
acknowledges commits before they are fsynced:

```yaml
resources:
  durability: async
```

`max-group-delay-ms` (default 0) is a gentler option that keeps `local`
durability: the publisher waits that many milliseconds so concurrent commits
share one fsync, trading a little latency for throughput.

> **Warning:** With `async`, acknowledged commits can be lost in a crash.
> Use it only for input you can re-import, and switch back to `local`
> before serving normal traffic.

### Point-in-time recovery

```yaml
resources:
  wal-archive-dir: /srv/rushdb-archive
  wal-archive-retention-seconds: 604800
```

See [Backup and restore](backup-and-restore.md).

## Logging

Logs go to stderr in a compact text format. Set the filter with `RUST_LOG`
(default `rushdb_server=info,tower_http=info`):

```bash
RUST_LOG=rushdb_server=debug,tower_http=info rushdb serve --data-dir /var/lib/rushdb
```

See [Monitoring](monitoring.md#logs) for the messages worth watching.

## See also

- [Configuration reference](../reference/configuration-reference.md)
- [Deployment](deployment.md)
- [Security](security.md)
- [Troubleshooting](troubleshooting.md)
