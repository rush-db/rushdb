# Fly.io

`fly.toml` runs one machine from the `rushdb/rushdb` image with a volume at
`/data`. From this directory:

```bash
fly launch --copy-config --no-deploy --name <app>
fly volumes create rushdb_data --size 10 --region <region>
fly secrets set RUSHDB_TOKEN=$(openssl rand -hex 32)
fly deploy
curl https://<app>.fly.dev/health
```

- The HTTP API is public over HTTPS. The PostgreSQL wire port stays on Fly's
  private network (`<app>.internal:5432`); from your machine, run
  `fly proxy 5432`.
- The machine never stops on idle (`auto_stop_machines = "off"`).
- A Fly volume belongs to one machine. Take backups as described in
  [Backup and restore](../../../docs/manual/operations/backup-and-restore.md).
