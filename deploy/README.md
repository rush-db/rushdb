# Deploy RushDB

Ready-made configurations for running the RushDB engine. Each one runs a
single node with a persistent data directory and requires you to set a token.

| Where | Template | What you get |
|---|---|---|
| Any host with Docker | [`docker-compose/compose.yaml`](templates/docker-compose/) | The engine with a named volume, ports on `127.0.0.1` |
| Any host with Docker and a domain | [`docker-compose/compose.https.yaml`](templates/docker-compose/) | The engine behind Caddy with an automatic TLS certificate |
| Kubernetes | [`charts/rushdb`](../charts/rushdb/) | Helm chart: a StatefulSet, a volume claim, an optional Ingress |
| A Linux VM (DigitalOcean, Hetzner, AWS, GCP, Azure, ...) | [`cloud-init/cloud-init.yaml`](templates/cloud-init/) | The binary under systemd, set up from the VM's user data |
| A Linux host you manage | [`systemd/rushdb.service`](templates/systemd/) | The systemd unit from the manual |
| Fly.io | [`fly/fly.toml`](templates/fly/) | One machine with a volume, HTTPS at `<app>.fly.dev` |
| Render | [`render.yaml`](templates/render/) | A web service with a disk |
| Coolify | [`coolify/`](templates/coolify/) | The Compose template as a Coolify resource |
| Dokploy | [`dokploy/`](templates/dokploy/) | The Compose template as a Dokploy service |

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

The templates use the `rushdb/rushdb` image or the release binaries, which
are published from the first 3.x release.

## Before you expose a node

- **Set a token.** Every client presents it. Generate one with
  `openssl rand -hex 32`; the server refuses the default token anywhere but
  loopback.
- **Only HTTP has TLS.** The PostgreSQL wire, gRPC and Bolt listeners are
  plaintext. The templates keep them on `127.0.0.1` or a private network.
- **Settings live in the data directory.** The first start writes
  `config.yml` there; later, a flag that disagrees with it is a startup error.
- **One node per data directory.** For read replicas, see
  [Replication](../docs/manual/operations/replication.md).

See [Deployment](../docs/manual/operations/deployment.md) and
[Security](../docs/manual/operations/security.md) in the manual.

## Not covered

Heroku and DigitalOcean App Platform have no persistent disk for a
container, so a database there loses its data on every restart. On
DigitalOcean, create a Droplet with the [cloud-init](templates/cloud-init/)
template instead.
