# Dokploy

[Dokploy](https://dokploy.com) runs the [Compose template](../docker-compose/compose.yaml)
as a Compose service.

1. **Create Service** → **Compose**. Use the `rush-db/rushdb` repository with
   the Compose path `deploy/templates/docker-compose/compose.yaml`, or paste
   that file in **Raw**.
2. In **Environment**, set `RUSHDB_TOKEN` (`openssl rand -hex 32`) and
   `RUSHDB_HTTP_PORT=3100`: Dokploy's own dashboard uses port 3000 on the host.
3. In **Domains**, add a domain for the `rushdb` service on port **3000** with
   HTTPS.
4. **Deploy**, then open `https://<domain>/health`.

The `rushdb-data` volume keeps the data across deploys. The PostgreSQL wire
port is published on the server's `127.0.0.1` only.
