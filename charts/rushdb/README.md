# RushDB Helm chart

Runs the RushDB engine as a single-node StatefulSet with a persistent volume,
a Service for the HTTP and PostgreSQL wire listeners, and an optional Ingress
for the HTTP API.

## Install

```bash
helm install rushdb oci://ghcr.io/rush-db/charts/rushdb \
  --set auth.token="$(openssl rand -hex 32)"
```

From a checkout of this repository:

```bash
helm install rushdb ./charts/rushdb --set auth.token="$(openssl rand -hex 32)"
```

Check it:

```bash
kubectl port-forward svc/rushdb 3000:3000
curl http://127.0.0.1:3000/ready
```

Applications in the cluster reach `http://rushdb.<namespace>.svc:3000`, and
PostgreSQL clients `rushdb.<namespace>.svc:5432` with the token as the
password.

## Values

| Key | Default | Description |
|---|---|---|
| `auth.token` | `''` | Bootstrap token. Required unless `auth.existingSecret` is set. |
| `auth.existingSecret` | `''` | Name of an existing Secret holding the token. |
| `auth.existingSecretKey` | `token` | Key of the token in that Secret. |
| `image.repository` | `rushdb/rushdb` | Image. |
| `image.tag` | chart `appVersion` | Image tag. |
| `server.languages` | `[]` | Enabled languages; empty keeps the default set (`sql`, `gremlin`, `sql-pgq`, `mongodb`). |
| `server.multitenant` | `false` | Multitenant mode. Needs `pgwirePort`, `grpcPort` and `boltPort` set to `0`. |
| `server.httpPort` | `3000` | HTTP port. |
| `server.pgwirePort` | `5432` | PostgreSQL wire port; `0` turns it off. |
| `server.grpcPort` | `0` | gRPC port; `0` is off. |
| `server.boltPort` | `0` | Bolt port; `0` is off. Needs `cypher` in `server.languages`. |
| `server.extraArgs` | `[]` | Further `rushdb serve` flags. |
| `server.extraEnv` | `[]` | Further environment variables. |
| `persistence.size` | `10Gi` | Volume size. |
| `persistence.storageClass` | `''` | Storage class; empty uses the cluster default. |
| `ingress.enabled` | `false` | Ingress for the HTTP API. |
| `resources` | 250m / 512Mi requests, 4Gi memory limit | Container resources. |
| `terminationGracePeriodSeconds` | `90` | Time for a clean stop. |

## Notes

- **One replica.** One node owns one data directory. For read replicas, see
  [Replication](../../docs/manual/operations/replication.md).
- **Settings live in the volume.** On first start the engine writes
  `config.yml` into `/data`; afterwards a flag that disagrees with it is a
  startup error. Change stored settings by editing that file.
- **Plaintext listeners.** PostgreSQL wire, gRPC and Bolt have no TLS. Keep
  them inside the cluster; the Ingress exposes HTTP only.
- The pod runs as uid 65532 with a read-only root filesystem.
