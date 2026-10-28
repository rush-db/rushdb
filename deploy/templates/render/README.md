# Render

The [`render.yaml`](../../../render.yaml) Blueprint at the repository root
creates a web service from the `rushdb/rushdb` image with a 10 GB disk at
`/data`, and generates `RUSHDB_TOKEN`.

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/rush-db/rushdb)

- Disks need a paid instance type; the Blueprint uses `starter`.
- Read the generated token in the service's **Environment** tab.
- Render serves the HTTP API over HTTPS at `https://<service>.onrender.com`.
  The PostgreSQL wire port is not exposed.
- A service with a disk runs one instance and restarts with a short downtime
  on each deploy.
