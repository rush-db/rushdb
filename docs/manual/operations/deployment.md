# Deployment

This page shows how to install `rushdb serve` on a Linux host, lay out its data
directory, run it under systemd, and size it for a small VPS. It is for
operators running RushDB as a service.

## What you deploy

RushDB is a single self-contained binary, `rushdb`; `rushdb serve` runs the
server. There are no other runtime services: storage,
WAL, indexes, and every protocol listener live in the one process.

Release archives are published per platform:

| Archive | Platform |
|---|---|
| `rushdb-x86_64-unknown-linux-gnu.tar.gz` | Linux x86-64, glibc 2.28+ |
| `rushdb-aarch64-unknown-linux-gnu.tar.gz` | Linux arm64, glibc 2.28+ |
| `rushdb-aarch64-apple-darwin.tar.gz` | macOS 11+ on Apple silicon |
| `rushdb-x86_64-apple-darwin.tar.gz` | macOS 11+ on Intel |

Each archive contains `rushdb`, `README.md`, and `LICENSE`, and
is published with a `.sha256` checksum, a capabilities file, an SPDX
dependency inventory, and Sigstore signature bundles. The same binary is
packaged as the `rushdb/rushdb` container image (see
[Run in a container](#run-in-a-container)).

Ready-made configurations live in the repository's
[`deploy/`](https://github.com/rush-db/rushdb/tree/main/deploy) directory:
Docker Compose (with an HTTPS variant behind Caddy), a
[Helm chart](https://github.com/rush-db/rushdb/tree/main/charts/rushdb),
cloud-init for Linux VMs, the systemd unit below, and templates for Fly.io,
Render, Coolify and Dokploy.

## Install the binary

```bash
sha256sum -c rushdb-x86_64-unknown-linux-gnu.tar.gz.sha256
tar -xzf rushdb-x86_64-unknown-linux-gnu.tar.gz
sudo install -m 0755 rushdb /usr/local/bin/
rushdb --version
```

Create a dedicated user and data directory:

```bash
sudo useradd --system --home /var/lib/rushdb --shell /usr/sbin/nologin rushdb
sudo install -d -o rushdb -g rushdb -m 0750 /var/lib/rushdb
```

## Run in a container

The `rushdb/rushdb` image (also `ghcr.io/rush-db/rushdb`) packages the
release binary for `linux/amd64` and `linux/arm64` on a distroless base and
runs as uid 65532. It sets `RUSHDB_LISTEN=0.0.0.0`, `RUSHDB_DATA_DIR=/data`
and `RUSHDB_ALLOW_INSECURE_LISTEN=true`, exposes 3000 (HTTP) and 5432
(PostgreSQL wire), and checks health with `rushdb health`. The entrypoint is
`rushdb serve`, so arguments after the image name are server options.

```yaml
# compose.yaml
services:
  rushdb:
    image: rushdb/rushdb:3
    environment:
      RUSHDB_TOKEN: ${RUSHDB_TOKEN:?set RUSHDB_TOKEN}
    ports:
      - "127.0.0.1:3000:3000"
      - "127.0.0.1:5432:5432"
    volumes:
      - rushdb-data:/data
    # Clean shutdown drains connections and writes a final checkpoint;
    # Docker's default 10 s is not enough.
    stop_grace_period: 90s
    restart: unless-stopped
volumes:
  rushdb-data:
```

- The server refuses to start without `RUSHDB_TOKEN`, because it listens on
  all interfaces inside the container.
- The cleartext transports are allowed off loopback inside the container.
  Publish ports to `127.0.0.1` or a private network, or terminate TLS in front
  of the container; HTTP TLS can also be enabled by mounting a certificate and
  setting `RUSHDB_TLS_CERT` and `RUSHDB_TLS_KEY`.
- With `docker run`, pass `--stop-timeout 90` for the same reason as
  `stop_grace_period`.
- `config.yml` in the volume is the source of truth after the first start, as
  on a host: changing a flag later requires editing that file.

To build the image yourself from a source checkout, run
`docker build -t rushdb .`.

## Ports

Every listener binds the same address (`--listen`, default `127.0.0.1`).

| Protocol | Flag | Default | Bound when |
|---|---|---|---|
| HTTP (Protocol v1, `/health`, `/ready`, `/metrics`) | `--port` | 3000 | always |
| PostgreSQL wire | `--pgwire-port` | 5432 | the `sql` language is enabled |
| gRPC | `--grpc-port` | 0 (off) | the port is non-zero |
| Bolt (Neo4j wire) | `--bolt-port` | 0 (off) | the port is non-zero and `cypher` is enabled |

Only HTTP supports TLS. Exposing anything beyond loopback has extra rules;
read [Security](security.md#network-exposure) before changing `--listen`.

## Data directory layout

A persistent node keeps everything under `--data-dir`:

```text
/var/lib/rushdb/
├── config.yml                 # configuration record, written on first start
├── rushdb.redb                # the store file
├── rushdb.redb.layout.json    # storage format and shard manifest
└── rushdb.redb.wal/           # write-ahead log, 64 MiB segments
```

With the Preview multi-shard option (`--shards N`), additional store files
`rushdb.redb-1` … `rushdb.redb-(N-1)` appear beside the first one.

- Keep the whole directory on one local filesystem with reliable `fsync`.
- **Copy-on-write filesystems (btrfs).** On Linux the server turns
  copy-on-write off for the data directory (`FS_NOCOW_FL`, what
  `chattr +C` sets) before it creates any file, so the store files and the
  WAL inherit it; ext4, xfs and tmpfs have no such flag and need nothing.
  With copy-on-write on, each checkpoint's fsync re-writes and re-compresses
  every changed extent: on a btrfs + zstd volume a bulk load stalled every
  tenant's commits for up to 15 s, against 1.2 s with copy-on-write off
  (same host and load). The flag only applies to files created empty, so a
  data directory created before this release keeps copy-on-write on its
  existing files; the server says so at boot (`data files still copy on
  write`, with the paths). Re-create the files by restoring a backup into a
  new data directory to drop it. Turning copy-on-write off also turns off
  btrfs checksums and compression for those files; the WAL and redb
  checksum their own records and pages.
- Back up with the tools in [Backup and restore](backup-and-restore.md),
  not by copying files from a running server.
- Omitting `--data-dir` runs an in-memory node: nothing survives a restart.

## First start

`config.yml` is written on the first start of an empty data directory and
from then on is the source of truth for that directory. To review and
create it explicitly before the first start:

```bash
sudo -u rushdb rushdb serve --data-dir /var/lib/rushdb --languages sql \
  --print-config
sudo -u rushdb rushdb serve --data-dir /var/lib/rushdb --languages sql \
  --init /var/lib/rushdb
```

Decide the tenancy mode now: `--multitenant` cannot be switched later for
an existing data directory. See [Configuration](configuration.md).

## Run under systemd

Store secrets in an environment file readable only by root:

```bash
sudo install -d -m 0750 /etc/rushdb
sudo tee /etc/rushdb/rushdb.env >/dev/null <<'EOF'
RUSHDB_TOKEN=replace-with-a-long-random-secret
RUSHDB_DATA_DIR=/var/lib/rushdb
EOF
sudo chmod 0600 /etc/rushdb/rushdb.env
```

`/etc/systemd/system/rushdb.service`:

```ini
[Unit]
Description=RushDB database server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=rushdb
Group=rushdb
EnvironmentFile=/etc/rushdb/rushdb.env
ExecStart=/usr/local/bin/rushdb serve
Restart=on-failure
RestartSec=5
# Clean shutdown drains connections (up to 30 s), stops maintenance
# (up to 30 s) and writes a final checkpoint; give it time.
KillSignal=SIGTERM
TimeoutStopSec=90
LimitNOFILE=65536
# Memory limits for the 8 GB VPS profile below; adjust to your host.
MemoryHigh=3500M
MemoryMax=4G
# Hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=/var/lib/rushdb

[Install]
WantedBy=multi-user.target
```

If you enable WAL archiving, add the archive directory to
`ReadWritePaths`.

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now rushdb
journalctl -u rushdb -f
```

The log shows one line with the bound ports and tenancy, for example
`rushdb listening http_port=3000 pgwire_port=5432 grpc_port=0 bolt_port=0 tenancy="single"`.

## Probes

The HTTP port is bound before the store is opened, so probes get an answer
during a long startup:

| Probe | Use for | During startup | Serving |
|---|---|---|---|
| `GET /health` | liveness | 200 `{"status":"starting","phase":...}` | 200 `{"status":"ok"}` |
| `GET /ready` | readiness / load-balancer membership | 503 with the boot phase | 200 `{"status":"ready"}` |

With TLS enabled the early probe listener is not used; the port answers once
the node has opened its store.

```bash
curl -fsS http://127.0.0.1:3000/ready
```

## Stopping the server

Send `SIGTERM` (or `SIGINT`). The server stops accepting work, waits for
in-flight HTTP requests (up to 30 s) and background maintenance (up to
30 s), writes a final durability checkpoint, closes the store, and exits 0.

> **Warning:** `SIGKILL` does not lose acknowledged commits under the default
> `local` durability (the WAL is fsynced first), but the next start must
> replay the WAL. Prefer a clean stop, and do not set a short stop timeout.

## 8 GB VPS profile

The measured reference profile for a 4 vCPU / 8 GB VPS running `rushdb serve`
behind an application proxy on the same host:

| Budget | Setting |
|---|---|
| OS and agents | ~0.5 GB |
| Application proxy | ~1 GB |
| `rushdb serve` | systemd `MemoryHigh=3500M`, `MemoryMax=4G` |
| Kernel page cache | the remaining ~2.5 GB (store file, vector segments, text indexes) |

`config.yml` `resources` section for that profile:

```yaml
resources:
  threads: 0                         # = vCPUs
  storage-cache: 256MiB              # storage page cache; the kernel caches the rest
  logical-cache-bytes: 384MiB        # vector and text caches
  query-memory-max: 256MiB
  http-inflight-body-bytes: 256MiB
  tx-log-retention-bytes: 4GiB
  disk-space-threshold-percent: 10
  retention-consumer-lease-seconds: 86400
```

> **Note:** The logical caches, the transaction pool, and per-query phase
> reservations all draw from `managed-limit` (default 512 MiB), and
> `storage-cache + transaction-pool` must stay below it. When you enlarge the
> caches, watch `rushdb_budget_used_bytes{pool="managed"}` against
> `rushdb_budget_limit_bytes{pool="managed"}` and raise `managed-limit` if
> usage runs close to the limit. See
> [Capacity planning](capacity-planning.md) for disk and throughput sizing.

## See also

- [Configuration](configuration.md)
- [Security](security.md)
- [Monitoring](monitoring.md)
- [Capacity planning](capacity-planning.md)
- [CLI reference](../reference/cli-reference.md)
