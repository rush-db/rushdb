# Operations

Running RushDB in production: installing and configuring the server,
securing it, backing it up, replicating it, watching it, and keeping its
storage healthy. These pages are for operators and are task oriented; exact
settings and values are in the [Reference](../reference/README.md).

## Pages

1. [Deployment](deployment.md) — install the binary, data directory layout,
   ports, systemd unit, probes, and the 8 GB VPS profile.
2. [Configuration](configuration.md) — how flags, environment variables, and
   `config.yml` combine, and common setups.
3. [Security](security.md) — tokens, capability tokens, tenant isolation,
   TLS, network exposure rules, and a hardening checklist.
4. [Backup and restore](backup-and-restore.md) — base backups, WAL
   archiving, point-in-time restore, graph snapshots, and verification.
5. [Replication](replication.md) — streaming followers, seeding from a base
   backup, promotion, and the Raft Preview.
6. [Monitoring](monitoring.md) — `/health`, `/ready`, Prometheus metrics,
   logs, and alerts.
7. [Capacity planning](capacity-planning.md) — measured envelope and sizing
   for disk, memory, and throughput.
8. [Storage and compaction](storage-and-compaction.md) — growth model,
   compaction, WAL retention, and the disk guard.
9. [Upgrading from 2.x](upgrading-from-2x.md) — moving from RushDB 2.x, and
   upgrades between 3.x releases.
10. [Troubleshooting](troubleshooting.md) — common errors and their fixes.

## Quick start for operators

```bash
# 1. Install
sudo install -m 0755 rushdb /usr/local/bin/
# 2. Create the data directory and its config (choose the tenancy mode now)
sudo -u rushdb rushdb serve --data-dir /var/lib/rushdb --init /var/lib/rushdb
# 3. Run with a real token
RUSHDB_TOKEN=$(openssl rand -hex 32) rushdb serve --data-dir /var/lib/rushdb
# 4. Check
curl -s http://127.0.0.1:3000/ready
```

## See also

- [Reference](../reference/README.md)
- [Architecture](../architecture/README.md)
- [Getting started](../getting-started/README.md)
