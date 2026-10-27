# Coolify

[Coolify](https://coolify.io) runs the [Compose template](../docker-compose/compose.yaml)
as one resource and puts its proxy, with TLS, in front.

1. **+ New** → **Docker Compose**. Point it at the `rush-db/rushdb`
   repository with the Compose path
   `deploy/templates/docker-compose/compose.yaml`, or paste that file.
2. In **Environment**, set `RUSHDB_TOKEN`. Coolify can generate it:
   `RUSHDB_TOKEN=$SERVICE_PASSWORD_64_RUSHDB`.
3. Give the `rushdb` service a domain on port **3000**.
4. **Deploy**, then open `https://<domain>/health`.

The `rushdb-data` volume persists across deploys. The PostgreSQL wire port is
published on the server's `127.0.0.1` only.
