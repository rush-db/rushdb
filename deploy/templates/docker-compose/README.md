# Docker Compose

`compose.yaml` runs the engine with a named volume and publishes HTTP (3000)
and PostgreSQL wire (5432) on `127.0.0.1`:

```bash
cp .env.example .env    # set RUSHDB_TOKEN (openssl rand -hex 32)
docker compose up -d
docker compose exec rushdb rushdb health
```

`compose.https.yaml` puts [Caddy](https://caddyserver.com) in front, which
obtains a TLS certificate for `RUSHDB_DOMAIN`. Point the domain's DNS at the
host and open ports 80 and 443, then:

```bash
docker compose -f compose.https.yaml up -d
curl https://$RUSHDB_DOMAIN/health
```

| Variable | Default | Meaning |
|---|---|---|
| `RUSHDB_TOKEN` | — (required) | Token clients present |
| `RUSHDB_IMAGE_TAG` | `3` | Image tag: `3`, `3.0`, or an exact release |
| `RUSHDB_HTTP_PORT` | `3000` | Host port for HTTP (`compose.yaml`) |
| `RUSHDB_PGWIRE_PORT` | `5432` | Host port for PostgreSQL wire |
| `RUSHDB_DOMAIN` | — | Public domain (`compose.https.yaml`) |

Server options go in `command:` (the image's entrypoint is `rushdb serve`);
`compose.yaml` has a commented example that enables Cypher over Bolt. Data
lives in the `rushdb-data` volume; back it up with the commands in
[Backup and restore](../../../docs/manual/operations/backup-and-restore.md).
