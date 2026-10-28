# CLI reference

RushDB ships one binary, `rushdb`, in every release archive, package and
image. Its commands:

| Command | Purpose |
|---|---|
| `rushdb serve` | The database server. Also runs one-shot maintenance commands (init, export, restore, compaction, base backup, point-in-time restore, verification). |
| `rushdb up` | `rushdb serve` for a local node: a persistent `./.rushdb` data directory by default and a startup summary. |
| `rushdb health` | Probe a running node. |

`rushdb --help` (`-h`) and `rushdb --version` (`-V`) work on their own and
after every command.

## `rushdb`

```text
rushdb <COMMAND>

Commands:
  serve   Run the database server. Without `--data-dir` the node is in memory
  up      Run a persistent local node: `serve` with a `./.rushdb` data directory and a startup summary. Takes every `serve` option
  health  Check that a node is up: exits 0 when `GET /health` answers 200
  help    Print this message or the help of the given subcommand(s)
```

### `rushdb up`

Runs the server in the foreground, like `rushdb serve`, after printing the
HTTP, PostgreSQL and Bolt addresses, the data directory, and whether the
default token is in use. It takes every [`rushdb serve`](#rushdb-serve)
option and environment variable; only one default differs:

| Option | Environment | Default under `up` | Default under `serve` |
|---|---|---|---|
| `--data-dir <DATA_DIR>` | `RUSHDB_DATA_DIR` | `./.rushdb` | none: an in-memory node |

```bash
RUSHDB_TOKEN=change-me rushdb up
rushdb up --port 3100 --pgwire-port 5433 --languages sql,cypher --bolt-port 7687
```

### `rushdb health`

Sends `GET /health` over plain HTTP and prints the response body. Exits `0`
when the node answers `200`, `1` otherwise (connection refused, timeout,
maintenance mode, or any other status). Use `curl` for HTTPS nodes.

| Option | Environment | Default | Description |
|---|---|---|---|
| `--url <URL>` | `RUSHDB_URL` | `http://127.0.0.1:3000` | Base URL of the node. |

The Docker image uses it as its health check.

## `rushdb serve`

```text
rushdb serve [OPTIONS]
```

Without a one-shot command flag, `rushdb serve` starts the server. Every server
option (listeners, security, resources, retention, replication,
transactions) is listed with its environment variable, `config.yml` key and
default in the [configuration reference](configuration-reference.md).

### Server startup

```bash
rushdb serve \
  --data-dir /var/lib/rushdb \
  --token "$RUSHDB_TOKEN" \
  --languages sql,cypher
```

On start the server logs one `rushdb listening` line with the `listen`
address, the bound `http_port`, `pgwire_port`, `grpc_port`, `bolt_port`
(0 = not bound) and the tenancy mode.

Signals: `SIGTERM` and `SIGINT` trigger a clean shutdown (drain listeners,
stop maintenance, final durability checkpoint, close the store). See
[Deployment](../operations/deployment.md#stopping-the-server).

### Maintenance commands

Each command below runs instead of the server and exits. Unless noted, it
opens `--data-dir` itself, so **stop the server first**: the store can be
opened by one process at a time. Pass the same tenancy (`--multitenant`)
setting the data directory was created with, or the stored `config.yml`
rejects the run.

| Command | Needs | Effect | Success output (stderr) |
|---|---|---|---|
| `--print-config` | — | Print the effective `config.yml` for the given flags to stdout. Does not touch any directory. | the YAML document on stdout |
| `--init <DATA_DIR>` | — | Create `DATA_DIR` if needed, write `config.yml` for the effective configuration, and validate it by reading it back. If a `config.yml` already exists, only validate it. | `--init: wrote and validated ...` / `already exists and validates` |
| `--export <PATH>` | `--data-dir` | Write a consistent logical graph snapshot (records and relationships) to `PATH`. Refuses to overwrite an existing file. | `export: wrote snapshot at commit index N to ...` |
| `--restore <PATH>` | fresh `--data-dir` | Validate and replay a snapshot written by `--export`. A truncated or corrupt file is rejected before any state is applied. Prints one progress line per chunk. | `restore: applied R records and L relationships across C chunks into ...` |
| `--compact` | `--data-dir`, server stopped | Reclaim retained MVCC history and storage-file slack. | `compact: store compacted at commit index N` |
| `--backup-base <PATH>` | `--data-dir`, server stopped | Copy the store files and `config.yml` into `PATH/base/` and write `PATH/manifest.json` with sizes and CRC32 checksums, pinned at the current commit index. `PATH` must not exist. If `--wal-archive-dir` is given, the base is registered in that archive. | `backup-base: K file(s) at commit index N -> ...` |
| `--verify-base <BASE_DIR>` | — | Recompute every file's size and CRC32 and compare with the manifest. | `verify-base: OK at commit index N` |
| `--restore-pitr <BASE_DIR>` | new `--data-dir` (must not exist), `--wal-archive-dir` | Verify the base, copy it into `--data-dir`, then replay archived WAL segments from the base's commit index up to `--to-lsn` (default: the newest archived commit). | `restore-pitr: replayed archived WAL to commit index N` |
| `--to-lsn <LSN>` | `--restore-pitr` | Last commit index to replay. | — |
| `--verify` | `--data-dir`, server stopped | Logical integrity check of every tenant. | `verify: no violations` |

Only one command runs per invocation. If several are given, the first in
this order runs: `--print-config`, `--init`, `--export`, `--restore`,
`--compact`, `--backup-base`, `--restore-pitr`, `--verify`, `--verify-base`.

### Exit codes

| Code | Meaning |
|---|---|
| `0` | Success (server shut down cleanly, or the command succeeded). |
| `1` | Server bootstrap failed (for example the port could not be bound, or `config.yml` validation failed), shutdown was incomplete, or `--backup-base` could not register the base with the retention coordinator or the WAL archive. |
| `2` | Invalid arguments (`--raft` without `--experimental-raft`, bad `--languages`, non-hex capability secret), or a one-shot command failed. `--verify` also exits 2 when it finds invariant violations. |

## Examples

Review the configuration a set of flags would produce, then persist it:

```bash
rushdb serve --data-dir /var/lib/rushdb --multitenant --languages sql --print-config
rushdb serve --data-dir /var/lib/rushdb --multitenant --languages sql --init /var/lib/rushdb
```

Offline base backup and verification:

```bash
systemctl stop rushdb
rushdb serve --data-dir /var/lib/rushdb --wal-archive-dir /srv/rushdb-archive \
  --backup-base /srv/rushdb-backups/2026-10-10
systemctl start rushdb
rushdb serve --verify-base /srv/rushdb-backups/2026-10-10
```

Point-in-time restore into a new directory:

```bash
rushdb serve --data-dir /var/lib/rushdb-restored \
  --wal-archive-dir /srv/rushdb-archive \
  --restore-pitr /srv/rushdb-backups/2026-10-10 --to-lsn 120000
```

## See also

- [Configuration reference](configuration-reference.md)
- [Backup and restore](../operations/backup-and-restore.md)
- [Storage and compaction](../operations/storage-and-compaction.md)
- [Command-line clients](../connectivity/cli.md)
