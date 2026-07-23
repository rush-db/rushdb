---
'rushdb-dashboard': patch
'rushdb-core': patch
'rushdb-docs': patch
---

Faster, more reliable vector search

- Vector search no longer stalls behind background indexing work — when an index isn't ready yet, queries fall back to exact scoring instead of waiting, so results always come back promptly.
- Embedding provider calls now have a strict time limit. A slow or unresponsive provider results in a quick, clear error instead of a request that hangs until it times out.
- Index backfill automatically retries transient provider failures instead of marking the index as failed on the first error.
- Switched the default embedding model to `openai/text-embedding-3-small` for consistently fast query-time embeddings. When the configured model changes, existing indexes are re-embedded automatically on startup — no manual steps required.
- Dashboard: semantic search now waits until you finish typing and cancels superseded requests, instead of firing several parallel searches per keystroke.
