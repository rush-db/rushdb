#!/usr/bin/env bash
# Fetch and convert the frozen vector-search calibration fixtures
# (docs/disk-backed-storage/11-filter-aware-ann.md §5.2 and
# 03-operations-migration-validation.md §6.8):
#
#   1. DBPedia-100K (Qdrant / OpenAI text-embedding-3-large, 1536-dim)
#      -> data/dbpedia/vectors.fvecs + data/dbpedia/payload.csv
#   2. SIFT1M (ann-benchmarks sift-128-euclidean, 128-dim)
#      -> data/sift1m/base.fvecs + data/sift1m/query.fvecs + data/sift1m/neighbors.ivecs
#
# Each fixture is written as `.fvecs` (uint32 dim + float32 rows) / `.ivecs`
# (uint32 k + uint32 ids) and a payload CSV for filter predicates, so the Rust
# benchmark loads them with no heavy deps. SHA-256 pins are recorded after the
# run in data/FIXTURES.sha256 (provenance for the frozen gate).

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
DATA_DIR="$ROOT/data/vector-fixtures"
mkdir -p "$DATA_DIR/dbpedia" "$DATA_DIR/sift1m"

# ---- DBPedia-100K -----------------------------------------------------------
DBPEDIA_REPO="Qdrant/dbpedia-entities-openai3-text-embedding-3-large-1536-100K"
DBPEDIA_URLS=(
  "https://huggingface.co/api/datasets/$DBPEDIA_REPO/parquet/default/train/0.parquet"
  "https://huggingface.co/api/datasets/$DBPEDIA_REPO/parquet/default/train/1.parquet"
  "https://huggingface.co/api/datasets/$DBPEDIA_REPO/parquet/default/train/2.parquet"
)

# ---- SIFT1M ------------------------------------------------------------------
SIFT_URL="https://ann-benchmarks.com/sift-128-euclidean.hdf5"

echo "==> Fetching DBPedia-100K parquet (3 shards)..."
for i in "${!DBPEDIA_URLS[@]}"; do
  curl -L --fail --retry 3 -o "$DATA_DIR/dbpedia/train-$i.parquet" "${DBPEDIA_URLS[$i]}"
done

echo "==> Fetching SIFT1M hdf5..."
curl -L --fail --retry 3 -o "$DATA_DIR/sift1m/sift-128-euclidean.hdf5" "$SIFT_URL"

echo "==> Installing python deps in a venv..."
python3 -m venv "$DATA_DIR/.venv"
"$DATA_DIR/.venv/bin/pip" install --quiet --upgrade pip
"$DATA_DIR/.venv/bin/pip" install --quiet pyarrow pandas h5py

echo "==> Converting DBPedia-100K..."
"$DATA_DIR/.venv/bin/python" "$HERE/convert_dbpedia.py" \
  "$DATA_DIR/dbpedia" \
  "$DATA_DIR/dbpedia/vectors.fvecs" \
  "$DATA_DIR/dbpedia/payload.csv"

echo "==> Converting SIFT1M..."
"$DATA_DIR/.venv/bin/python" "$HERE/convert_sift1m.py" \
  "$DATA_DIR/sift1m/sift-128-euclidean.hdf5" \
  "$DATA_DIR/sift1m"

echo "==> Recording provenance (sha256)..."
(
  cd "$DATA_DIR"
  find . -type f \( -name "*.fvecs" -o -name "*.ivecs" -o -name "*.csv" -o -name "*.parquet" -o -name "*.hdf5" \) \
    -not -path "./.venv/*" -print0 | sort -z | xargs -0 shasum -a 256
) > "$DATA_DIR/FIXTURES.sha256"

echo "==> Done."
echo "    DBPedia:  $DATA_DIR/dbpedia/vectors.fvecs + payload.csv"
echo "    SIFT1M:   $DATA_DIR/sift1m/{base,query}.fvecs + neighbors.ivecs"
echo "    Provenance pin: $DATA_DIR/FIXTURES.sha256"