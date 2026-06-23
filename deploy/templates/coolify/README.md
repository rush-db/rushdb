# Deploy RushDB on Coolify

[Coolify](https://coolify.io) deploys RushDB straight from the bundled
full-stack Compose file — RushDB + Neo4j + Postgres in one resource.

## Steps

1. In Coolify: **+ New** → **Docker Compose** (Empty or from a Git repository).
2. If using the Git option, point it at `rush-db/rushdb` and set the Compose
   path to `deploy/templates/docker-compose/compose.full.yaml`. Otherwise paste
   the contents of that file.
3. Set the environment variables (Coolify → your resource → **Environment**):

   | Variable                                   | Value                                              |
   | ------------------------------------------ | -------------------------------------------------- |
   | `RUSHDB_PASSWORD`                          | a strong admin password                            |
   | `RUSHDB_AES_256_ENCRYPTION_KEY`            | exactly 32 chars — `openssl rand -hex 16`          |
   | `NEO4J_PASSWORD`                           | a strong Neo4j password                            |
   | `POSTGRES_PASSWORD`                        | a strong Postgres password                         |
   | `RUSHDB_BASE_URL` / `RUSHDB_DASHBOARD_URL` | your public URL, e.g. `https://rushdb.example.com` |

   Coolify can generate secrets for you with its magic variables, e.g. set
   `RUSHDB_PASSWORD=$SERVICE_PASSWORD_RUSHDB`. Set the public URLs from the FQDN
   Coolify assigns the `rushdb` service.

4. Expose the `rushdb` service on port **3000** and attach your domain. Coolify
   handles TLS via its built-in proxy.
5. **Deploy.** Open your domain and sign in with `RUSHDB_LOGIN` / `RUSHDB_PASSWORD`.

## Notes

- Neo4j first boot downloads the APOC plugin (~30s); RushDB waits for it via a
  health check.
- Persist the `neo4j-data` and `postgres-data` volumes (Coolify does by default).
- For a managed graph database, drop the `neo4j` service and point `NEO4J_URL`
  at Neo4j Aura instead.
