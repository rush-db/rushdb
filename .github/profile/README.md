<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

### The memory layer for AI agents and apps.

Push any JSON. Get graph relationships and vector search — automatically.
No schema. No pipeline. No glue code.

[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X](https://img.shields.io/twitter/follow/rushdb?style=social)](https://x.com/RushDatabase)
[![NPM](https://img.shields.io/npm/v/%40rushdb%2Fjavascript-sdk?label=npm)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![PyPI](https://img.shields.io/pypi/v/rushdb?label=pypi)](https://pypi.org/project/rushdb/)

[Website](https://rushdb.com) · [Docs](https://docs.rushdb.com) · [Cloud](https://app.rushdb.com) · [Examples](https://github.com/rush-db/examples)

</div>

---

Your agent needs memory. The standard answer is three databases — Redis, a vector store, a graph DB — plus glue code to keep them in sync.

RushDB replaces all three. One API. Push JSON, query by meaning and by relationship, in a single call.

```python
from rushdb import RushDB

db = RushDB('RUSHDB_API_KEY')

# One-time: tell RushDB to auto-embed 'output' on every write
db.ai.indexes.create(label='MEMORY', property_name='output')

# Store an agent action — no embedder, no vectors array
db.records.create(
    label='MEMORY',
    data={'agent_id': 'agent-42', 'topic': 'auth decision', 'output': summary_text},
)

# Recall semantically — just pass the query string
memories = db.ai.search({
    'labels': ['MEMORY'],
    'propertyName': 'output',
    'query': 'what did we decide about auth?',
    'where': {'agent_id': 'agent-42'},
    'limit': 10,
})
```

---

## Repositories

| Repo | What it is |
|---|---|
| [rushdb](https://github.com/rush-db/rushdb) | Platform — core API, dashboard, self-hosting |
| [rushdb-python](https://github.com/rush-db/rushdb-python) | Python SDK |
| [mcp-server](https://github.com/rush-db/mcp-server) | MCP server for Claude, Cursor, Windsurf |
| [examples](https://github.com/rush-db/examples) | Code samples across frameworks and use cases |

---

## MCP — connect to your AI client in 30 seconds

```json
{
  "mcpServers": {
    "rushdb": {
      "command": "npx",
      "args": ["@rushdb/mcp-server"],
      "env": { "RUSHDB_API_KEY": "your-api-key-here" }
    }
  }
}
```

Get an API key at [app.rushdb.com](https://app.rushdb.com) · [hi@rushdb.com](mailto:hi@rushdb.com)
