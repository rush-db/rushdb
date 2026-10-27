# Installation

This page shows how to get the `rushdb` binary, start a
persistent local node, choose where its data lives, and replace the default
access token. It is for anyone setting up RushDB on a workstation or a test
server; production hardening is covered in
[Deployment](../operations/deployment.md) and
[Security](../operations/security.md).

## The `rushdb` binary

RushDB is one binary, `rushdb`, with three commands:

| Command | Purpose |
|---|---|
| `rushdb serve` | The database server. Every option is a command-line flag or an environment variable; `rushdb serve --help` lists them all. |
| `rushdb up` | The server for a local node: a persistent `./.rushdb` data directory by default and a startup summary. Takes every `serve` option. |
| `rushdb health` | Check that a node is up. |

## Install script (Linux and macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh | sh
```

The script picks the archive for your platform, verifies its SHA-256
checksum, installs `rushdb` into `~/.rushdb/bin`, and adds that
directory to `PATH` in your shell's startup file. When `cosign` is installed it
also verifies the archive's Sigstore signature.

| Option | Environment variable | Default | Meaning |
|---|---|---|---|
| `--version <tag>` | `RUSHDB_VERSION` | latest release | Install a specific release, e.g. `v3.0.0` |
| `--dir <path>` | `RUSHDB_INSTALL_DIR` | `~/.rushdb/bin` | Install directory |
| `--no-modify-path` | `RUSHDB_NO_MODIFY_PATH=1` | off | Leave shell startup files alone |
| `--verify-signature` | `RUSHDB_VERIFY_SIGNATURE=1` | off | Fail unless `cosign` verifies the signature |

Pass options after `sh -s --`:

```bash
curl -fsSL https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh | sh -s -- --version v3.0.0 --no-modify-path
```

Supported platforms:

| Platform | Requirement |
|---|---|
| Linux x86-64 and arm64 | glibc 2.28 or newer (RHEL 8, Debian 10, Ubuntu 18.10 and later) |
| macOS Apple silicon and Intel | macOS 11 or newer |

Alpine and other musl-based systems, and Windows, have no prebuilt binary; use
the Docker image (on Windows, through WSL 2 or Docker Desktop).

To upgrade, run the script again; it replaces the binary. To uninstall,
delete `~/.rushdb/bin` and the line ending in `# rushdb` from your shell's
startup file.

## Homebrew

```bash
brew install rush-db/tap/rushdb
brew services start rushdb    # optional: run in the background
```

The service keeps its data in `$(brew --prefix)/var/rushdb` and logs to
`$(brew --prefix)/var/log/rushdb.log`.

## Docker

The `rushdb/rushdb` image (mirrored as `ghcr.io/rush-db/rushdb`) runs
the server as a non-root user on `linux/amd64` and `linux/arm64`:

```bash
docker run -d --name rushdb \
  -p 127.0.0.1:3000:3000 -p 127.0.0.1:5432:5432 \
  -v rushdb-data:/data \
  -e RUSHDB_TOKEN="$(openssl rand -hex 32)" \
  rushdb/rushdb
```

- Data lives in the `/data` volume.
- `RUSHDB_TOKEN` is required: inside the container the server listens on all
  interfaces, and it refuses to start there with the default token.
- The image allows the cleartext transports off loopback
  (`RUSHDB_ALLOW_INSECURE_LISTEN=true`) because publishing ports is decided by
  `docker run -p`. Publish to `127.0.0.1` as above, or put TLS in front before
  exposing the port to a network.
- The entrypoint is `rushdb serve`, so arguments after the image name are
  server options, for example
  `rushdb/rushdb --languages sql,cypher,gql --bolt-port 7687`.
- The image's health check runs `rushdb health`.

Tags: `3.0.0` (exact), `3.0` and `3` (latest in that line). Images are signed
with Sigstore and carry an SBOM and build provenance.

## Cargo

```bash
cargo binstall rushdb-server            # downloads the release binary
cargo install rushdb-server --locked    # compiles from source
```

`cargo install` needs a C/C++ compiler, because the vector-index library is
compiled from C++ sources. Only `rushdb-server` is meant to be installed; the
other `rushdb-*` crates are internal components with no stable API.

## Release archive

Each release publishes a `tar.gz` archive per platform, containing `rushdb`,
`README.md` and `LICENSE`, with a SHA-256 checksum file, a software
bill of materials, and Sigstore signature bundles.

| Platform | Archive |
|---|---|
| Linux x86-64 | `rushdb-x86_64-unknown-linux-gnu.tar.gz` |
| Linux arm64 | `rushdb-aarch64-unknown-linux-gnu.tar.gz` |
| macOS Apple silicon | `rushdb-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `rushdb-x86_64-apple-darwin.tar.gz` |

Download the archive and its `.sha256` file from the project's GitHub releases
page, then verify and unpack:

```bash
shasum -a 256 -c rushdb-x86_64-unknown-linux-gnu.tar.gz.sha256
mkdir -p ~/rushdb && tar -C ~/rushdb -xzf rushdb-x86_64-unknown-linux-gnu.tar.gz
export PATH="$HOME/rushdb:$PATH"
rushdb --version
```

To check the signature as well, see [Security](../operations/security.md) or
the `cosign verify-blob` command in the repository's `RELEASING.md`.

## Build from source

Prerequisites:

- A Rust toolchain installed with `rustup`. The repository pins the exact
  toolchain in `rust-toolchain.toml`; `rustup` installs it automatically the
  first time you build.
- A C/C++ compiler.

```bash
git clone https://github.com/rush-db/rushdb.git
cd rushdb
cargo build --release -p rushdb-server --bin rushdb
ls target/release/rushdb
```

Copy the binary to a directory on your `PATH`, or build an image with
`docker build -t rushdb .`.

## Start a node with `rushdb up`

```bash
rushdb up
```

`rushdb up` runs the server with a persistent data directory, prints where
it is listening, and waits; stop the node with `Ctrl+C`. With no other
settings the node:

- serves the native HTTP protocol on `http://127.0.0.1:3000` (loopback only);
- serves the PostgreSQL wire protocol on `127.0.0.1:5432`;
- keeps its data in `./.rushdb`;
- accepts the token `local-token`;
- runs in single-tenant mode, with SQL, Gremlin, SQL/PGQ and Mongo-style
  statements enabled.

Check that it is up:

```bash
rushdb health
```

```json
{"status":"ok","engine":"rushdb-lmpg","protocolVersion":1,"queryLanguages":["gremlin","mongo","searchQuery","sql","sqlPgq"]}
```

`rushdb health` requests `GET /health` (which needs no token) and exits
non-zero unless the node answers. `curl -s http://127.0.0.1:3000/health`
returns the same document.

`rushdb up` takes every server option. The ones you are most likely to need:

| Option | Environment variable | Default | Meaning |
|---|---|---|---|
| `--data-dir <path>` | `RUSHDB_DATA_DIR` | `./.rushdb` | Persistent database directory |
| `--port <n>` | — | `3000` | HTTP port |
| `--pgwire-port <n>` | — | `5432` | PostgreSQL wire port; `0` picks a free port |
| `--token <token>` | `RUSHDB_TOKEN` | `local-token` | Token clients must present |
| `--languages <list>` | — | `sql,gremlin,sql-pgq,mongodb` | Enabled query languages |
| `--bolt-port <n>` | — | `0` (off) | Bolt port; needs `cypher` |

The [CLI reference](../reference/cli-reference.md) lists the rest.

If port 3000 or 5432 is already taken (a local web server or PostgreSQL),
choose others:

```bash
rushdb up --port 3100 --pgwire-port 5433
```

To enable Cypher and Bolt:

```bash
rushdb up --languages sql,gremlin,sql-pgq,mongodb,cypher --bolt-port 7687
```

The node binds `127.0.0.1` unless you pass `--listen`; read
[Security](../operations/security.md) before serving other machines.

> **Note:** Without `--data-dir`, `rushdb serve` runs an **in-memory** node:
> nothing is written to disk and all data is lost when the process exits.
> `rushdb up` always has a data directory.

## The data directory

On first start the node creates the directory and writes:

| Entry | Contents |
|---|---|
| `config.yml` | The effective configuration the directory was created with (ports, tenancy mode, languages, resource limits). Secrets are never written here. |
| `rushdb.redb` | The database file. |
| `rushdb.redb.wal/` | The write-ahead log: committed changes (by default fsynced before a commit is acknowledged), checkpointed into the database file in the background. |

`config.yml` is the **source of truth** for its directory. On later starts the
node reads it, and a command-line flag or environment variable that disagrees
with a stored value is a startup error that names both values — nothing is
silently overridden. To change a setting, stop the node, edit `config.yml`, and
start it again. For example, to enable Cypher on a directory created by
`rushdb up`, change the `languages` section:

```yaml
languages:
  sql: true
  gql: false
  cypher: true
  gremlin: true
  sql-pgq: true
  mongodb: true
```

> **Warning:** The tenancy mode (`single` or `multitenant`) is fixed when a
> directory is created. A directory cannot be switched between modes; use a
> fresh directory and move the data instead.

To review the configuration a set of flags would produce without starting a
server, use `rushdb serve --print-config`. To write and validate `config.yml` for a
new directory without starting a server, use `rushdb serve --init <dir>`. Both are
described in [Configuration](../operations/configuration.md).

## Set your own token

Every request except the `/health` and `/ready` probes must present the
node's bootstrap token (or, when configured, a signed capability token). The default, `local-token`, is meant for local
experiments only. Set your own before you start a node you care about:

```bash
export RUSHDB_TOKEN="$(openssl rand -hex 32)"
rushdb up --data-dir ./.rushdb
```

`rushdb up` and `rushdb serve` read the token from `--token` or
`RUSHDB_TOKEN`. The token is never stored in `config.yml`, so
supply it on every start. An empty token is refused at startup.

Clients send the token like this:

| Protocol | How to present the token |
|---|---|
| HTTP | `Authorization: Bearer <token>` or `X-RushDB-Token: <token>` |
| PostgreSQL wire | As the password (the user name is ignored) |
| gRPC | Metadata `authorization: Bearer <token>` |
| Bolt | `HELLO` with scheme `basic` and the token as credentials |

> **Warning:** The bootstrap token grants full access to every tenant on the
> node. The node binds to `127.0.0.1` by default; before you expose it on
> another address, read [Security](../operations/security.md) and
> [Authentication](../connectivity/authentication.md) — non-loopback HTTP
> requires TLS (or an explicit acknowledgement flag), and tenant-facing access
> should use signed capability tokens rather than the bootstrap token.

## See also

- [Quickstart](quickstart.md)
- [Configuration](../operations/configuration.md)
- [Configuration reference](../reference/configuration-reference.md)
- [CLI reference](../reference/cli-reference.md)
- [Deployment](../operations/deployment.md)
