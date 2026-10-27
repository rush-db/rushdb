#!/usr/bin/env bash
# Static checks for deploy/ and charts/: every template parses, the Compose
# files and the Helm chart render, and the systemd unit matches the manual.
# Needs docker (compose) and helm; CI runs it in the deploy-templates job.
set -euo pipefail
cd "$(dirname "$0")/.."

# The unit file is the manual's unit, byte for byte.
manual_unit="$(awk '/^`\/etc\/systemd\/system\/rushdb.service`:/{f=1;next} f&&/^```ini/{g=1;next} g&&/^```/{exit} g' \
  docs/manual/operations/deployment.md)"
if [ "$manual_unit" != "$(cat deploy/templates/systemd/rushdb.service)" ]; then
  echo "deploy/templates/systemd/rushdb.service differs from the unit in docs/manual/operations/deployment.md" >&2
  exit 1
fi

python3 - <<'PY'
import tomllib, yaml
yaml.safe_load(open('deploy/templates/cloud-init/cloud-init.yaml'))
assert open('deploy/templates/cloud-init/cloud-init.yaml').readline().strip() == '#cloud-config'
yaml.safe_load(open('render.yaml'))
tomllib.load(open('deploy/templates/fly/fly.toml', 'rb'))
PY

for file in deploy/templates/docker-compose/compose.yaml deploy/templates/docker-compose/compose.https.yaml; do
  RUSHDB_TOKEN=check RUSHDB_DOMAIN=rushdb.example.com docker compose -f "$file" config --quiet
done

helm lint charts/rushdb --set auth.token=check
helm template check charts/rushdb --set auth.token=check >/dev/null
helm template check charts/rushdb --set auth.existingSecret=rushdb --set ingress.enabled=true \
  --set server.boltPort=7687 --set 'server.languages={sql,cypher}' >/dev/null
if helm template check charts/rushdb --set auth.token=check --set server.multitenant=true >/dev/null 2>&1; then
  echo "the chart accepted multitenant mode with the PostgreSQL wire listener on" >&2
  exit 1
fi
echo "deploy templates: ok"
