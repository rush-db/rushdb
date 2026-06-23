# Deploy RushDB on Dokploy

[Dokploy](https://dokploy.com) runs RushDB from the bundled full-stack Compose
file — RushDB + Neo4j + Postgres as a single Compose service.

## Steps

1. In Dokploy: create a project → **Create Service** → **Compose**.
2. Choose the Git provider, point it at `rush-db/rushdb`, and set the Compose
   path to `deploy/templates/docker-compose/compose.full.yaml`. (Or use the Raw
   provider and paste that file's contents.)
3. Add environment variables under the service's **Environment** tab:

   | Variable                                   | Value                                     |
   | ------------------------------------------ | ----------------------------------------- |
   | `RUSHDB_PASSWORD`                          | a strong admin password                   |
   | `RUSHDB_AES_256_ENCRYPTION_KEY`            | exactly 32 chars — `openssl rand -hex 16` |
   | `NEO4J_PASSWORD`                           | a strong Neo4j password                   |
   | `POSTGRES_PASSWORD`                        | a strong Postgres password                |
   | `RUSHDB_BASE_URL` / `RUSHDB_DASHBOARD_URL` | your public URL                           |

4. Add a **Domain** pointing to the `rushdb` service on port **3000**; Dokploy
   provisions TLS via Traefik.
5. **Deploy**, then open your domain and sign in.

## Notes

- The bundled `neo4j` and `postgres` services keep their data in named volumes;
  Dokploy persists Compose volumes across redeploys.
- Neo4j first boot fetches APOC (~30s); RushDB gates on its health check.
- To use Neo4j Aura instead, remove the `neo4j` service and set `NEO4J_URL`,
  `NEO4J_USERNAME`, `NEO4J_PASSWORD` to your Aura instance.
