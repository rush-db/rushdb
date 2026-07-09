---
'@rushdb/javascript-sdk': minor
'@rushdb/mcp-server': minor
'rushdb-dashboard': minor
'rushdb-core': minor
'rushdb-docs': minor
'@rushdb/skills': minor
---

Separate Smart Search from vector search across SDKs, MCP, dashboard, and docs.

- Add `db.records.vectorSearch({...})` as the JavaScript SDK API for direct vector similarity search.
- Keep `db.ai.search(prompt)` for schema-aware Smart Search that generates and executes a `SearchQuery`.
- Add MCP `vectorSearch` and keep `semanticSearch` as a deprecated compatibility alias.
- Update dashboard semantic retrieval to use the new records vector-search surface.
- Refresh docs and examples, including a Smart Search guide with flow, caveats, and limitations.
