# CLI

RushDB ships as one binary, `rushdb`. It runs the database server
(`rushdb serve`), runs a persistent local node with one command
(`rushdb up`), and checks whether a node is up (`rushdb health`). This page
covers `rushdb up` and how it relates to `rushdb serve`. It is for developers
who want a database running on their machine with one command.

> **Note:** `rushdb` is not an interactive query shell. To run queries, use the
> [HTTP API](http-api.md) (for example with `curl`), `psql` over the
> [PostgreSQL wire protocol](postgresql-wire.md), or a Bolt client.

## Commands

```text
$ rushdb --help
RushDB, the LMPG database

Usage: rushdb <COMMAND>

Commands:
  serve   Run the database server. Without `--data-dir` the node is in memory
  up      Run a persistent local node: `serve` with a `./.rushdb` data directory and a startup summary. Takes every `serve` option
  health  Check that a node is up: exits 0 when `GET /health` answers 200
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### `rushdb up`

Starts a persistent, single-node server in the foreground. It is
`rushdb serve` with two differences: the data directory defaults to
`./.rushdb` instead of memory, and it prints where the node listens before it
starts. It takes every `rushdb serve` option and environment variable.

```bash
rushdb up --data-dir ~/rushdb-data
```

```text
RushDB 3.0.0
  HTTP     http://127.0.0.1:3000
  Postgres postgresql://127.0.0.1:5432
  Data     /home/me/rushdb-data
  Token    local-token (local default; set --token or RUSHDB_TOKEN)
Press Ctrl+C to stop.
```

The server listens on `127.0.0.1:3000` with the default settings. Check it:

```bash
rushdb health
```

## Configuring a node started with `rushdb up`

Pass any server option as a flag or its `RUSHDB_*` environment variable:

```bash
RUSHDB_TOKEN="$(openssl rand -hex 32)" \
rushdb up --data-dir ~/rushdb-data --languages sql,cypher --bolt-port 7687
```

**`config.yml`.** On first start the server writes `config.yml` into the data
directory. It is the source of truth for that directory afterwards: a flag
that disagrees with it is rejected. To change a stored setting, stop the
server, edit the file, and start it again:

```yaml
languages:
  sql: true
  gql: false
  cypher: true
  gremlin: false
  sql-pgq: false
  mongodb: false
```

> **Warning:** The default bootstrap token is `local-token`. Set `RUSHDB_TOKEN`
> to a secret value before you expose the node to anything other than your own
> machine. Secrets are never written to `config.yml`.

For production deployments, run `rushdb serve` under a service manager. All of
its flags are listed in the [CLI reference](../reference/cli-reference.md);
`rushdb serve --print-config` shows the effective `config.yml` without
starting a server.

## See also

- [Quickstart](../getting-started/quickstart.md)
- [Installation](../getting-started/installation.md)
- [CLI reference](../reference/cli-reference.md)
- [Configuration](../operations/configuration.md)
