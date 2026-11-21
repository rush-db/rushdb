#!/usr/bin/env python3
"""Text and hybrid search against a running server; scripts/smoke-image.sh
runs it against the image it tests. Per collection: a vector index,
documents with vectors, a text index created after the load (the order a
bulk-loading client uses), then a text search and a hybrid search that
must both rank document 1 first. Results only, never timing.

usage: scripts/smoke-search.py BASE_URL TOKEN [ROUNDS]
"""
import json, sys, time, urllib.error, urllib.request

BASE, TOKEN = sys.argv[1].rstrip("/"), sys.argv[2]
ROUNDS = int(sys.argv[3]) if len(sys.argv) > 3 else 5
DOCS = [(1, "quantum world", "machines learn quantum mechanics", [1.0, 0.0, 0.0, 0.0]),
        (2, "dragons", "magic dragon call", [0.0, 1.0, 0.0, 0.0]),
        (3, "gardens", "roses and tulips in spring", [0.0, 0.0, 1.0, 0.0])]


def op(operation):
    body = json.dumps({"protocolVersion": 1, "context": {}, "operation": operation}).encode()
    req = urllib.request.Request(f"{BASE}/protocol/v1/execute", data=body, headers={
        "authorization": f"Bearer {TOKEN}", "content-type": "application/json"})
    return json.loads(urllib.request.urlopen(req, timeout=60).read())["response"]


def settled(query):
    deadline = time.time() + 30
    while True:
        indexes = op({"kind": "describe", "describe": {"kind": "indexes"}}).get("indexes", [])
        if all(i.get("state") == "ready" for i in indexes):
            r = op({"kind": "query", "query": query})
            if not r.get("pending") or time.time() > deadline:
                return r
        if time.time() > deadline:
            raise SystemExit(f"indexes never settled: {indexes}")
        time.sleep(0.05)


def main():
    for _ in range(300):
        try:
            urllib.request.urlopen(f"{BASE}/ready", timeout=1)
            break
        except Exception:
            time.sleep(0.1)
    else:
        try:
            urllib.request.urlopen(f"{BASE}/ready", timeout=1)
        except urllib.error.HTTPError as error:
            raise SystemExit(f"server never ready: {error.read()[:300]!r}")
    failures = []
    for round_ in range(ROUNDS):
        for prop in ("*", "body"):
            label = f"smoke_{round_}_{'all' if prop == '*' else prop}"
            op({"kind": "index", "index": {"kind": "createVector", "definition": {
                "name": f"{label}-embedding", "label": label, "property": "embedding", "dimensions": 4,
                "similarity": "cosine"}}})
            op({"kind": "ingest", "ingest": {"kind": "documents", "label": label, "documents": [
                {"doc_id": d, "title": t, "body": b,
                 "$vectors": [{"propertyName": "embedding", "vector": v, "index": f"{label}-embedding"}]}
                for d, t, b, v in DOCS], "options": {"returnResult": True}}})
            op({"kind": "index", "index": {"kind": "create", "definition": {
                "name": f"{label}_text", "label": label, "property": prop, "kind": "text"}}})
            text = settled({"kind": "text", "index": f"{label}_text", "query": "quantum", "limit": 5})
            if len(text.get("rows", [])) != 1:
                failures.append(f"text {label}: {json.dumps(text)[:300]}")
            hybrid = op({"kind": "query", "query": {"kind": "hybrid", "query": {
                "labels": [label], "vector": {"property": "embedding", "vector": [0.0, 0.5, 1.0, 0.0],
                                              "index": f"{label}-embedding"},
                "text": {"index": f"{label}_text", "query": "quantum"}, "limit": 2, "fusion": "rrf", "rrfK": 60}}})
            ids = [r["properties"].get("doc_id") for r in hybrid.get("records", [])]
            # The vector ranks documents 3, 2, 1; the text finds document 1
            # alone. Fused, document 1 leads (first for text, third for the
            # vector); without the text side documents 3 and 2 would.
            if ids[:1] != [1]:
                failures.append(f"hybrid {label}: doc ids {ids}")
    print(f"e2e smoke: {ROUNDS * 2} collections, {len(failures)} failures")
    for f in failures:
        print("  " + f)
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
