# RushDB Helm Chart

[RushDB](https://rushdb.com) is a developer-friendly graph database platform built on Neo4j.

## TL;DR

```bash
helm install rushdb oci://ghcr.io/rush-db/charts/rushdb \
  --set rushdb.encryptionKey="$(openssl rand -hex 16)"
```

## Prerequisites

- Kubernetes 1.27+
- Helm 3.12+

## Installing the chart

```bash
# With bundled Neo4j (default)
helm install rushdb oci://ghcr.io/rush-db/charts/rushdb \
  --set rushdb.encryptionKey="32SymbolStringHere00000000000000" \
  --set rushdb.adminLogin=admin \
  --set rushdb.adminPassword=changeme

# Against an existing Neo4j
helm install rushdb oci://ghcr.io/rush-db/charts/rushdb \
  --set rushdb.encryptionKey="32SymbolStringHere00000000000000" \
  --set neo4j.enabled=false \
  --set rushdb.neo4jUrl=bolt://my-neo4j:7687 \
  --set rushdb.neo4jPassword=password
```

## Configuration

See [values.yaml](./values.yaml) for the full reference.

| Key                       | Description                                 | Default         |
| ------------------------- | ------------------------------------------- | --------------- |
| `rushdb.encryptionKey`    | **Required.** AES-256 key, exactly 32 chars | `""`            |
| `rushdb.adminLogin`       | Admin username                              | `admin`         |
| `rushdb.adminPassword`    | Admin password                              | `changeme`      |
| `rushdb.sqlDbType`        | `sqlite` or `postgres`                      | `sqlite`        |
| `rushdb.sqlDbUrl`         | PostgreSQL DSN (when `sqlDbType=postgres`)  | `""`            |
| `rushdb.replicaCount`     | Pod replicas (use 1 with sqlite)            | `1`             |
| `rushdb.ingress.enabled`  | Create an Ingress resource                  | `false`         |
| `neo4j.enabled`           | Deploy bundled Neo4j                        | `true`          |
| `neo4j.password`          | Neo4j password                              | `neo4jpassword` |
| `neo4j.persistence.size`  | Neo4j data PVC size                         | `10Gi`          |
| `rushdb.persistence.size` | SQLite data PVC size                        | `1Gi`           |

## Upgrading

```bash
helm upgrade rushdb oci://ghcr.io/rush-db/charts/rushdb -f values.yaml
```

## Uninstalling

```bash
helm uninstall rushdb
# PVCs are retained; delete them manually if desired:
kubectl delete pvc -l app.kubernetes.io/instance=rushdb
```
