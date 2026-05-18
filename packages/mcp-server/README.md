# RushDB MCP Server

### Give your agent a memory layer. Via MCP.

Connect RushDB to any MCP-compatible AI client and your agent gains persistent, structured memory — semantically searchable and graph-traversable, without managing embeddings or schemas.

Works with **ChatGPT**, **Claude.ai**, Claude Desktop, Cursor, Windsurf, Gemini CLI, and any MCP-compatible tool.

---

## Two ways to connect

### Remote OAuth (no installation)

For **ChatGPT**, **Claude.ai**, and other web-based AI clients — connect directly without installing anything locally:

**MCP endpoint:** `https://mcp.rushdb.com/mcp`

Your client redirects to RushDB for OAuth sign-in and receives a scoped token. No API key setup required.

**ChatGPT:**

1. Go to **Settings → Connectors → Add connector**
2. Enter `https://mcp.rushdb.com/mcp`
3. Sign in with your RushDB account

**Claude.ai:**

1. Go to **Settings → Integrations → Add integration**
2. Enter `https://mcp.rushdb.com/mcp`
3. Authorize with your RushDB account

---

### Local (API key)

For Claude Desktop, Cursor, Windsurf, VS Code, and other stdio MCP clients:

**1. Get an API key** at [app.rushdb.com](https://app.rushdb.com)

**2. Add to your MCP client config:**

```json
{
  "mcpServers": {
    "rushdb": {
      "command": "npx",
      "args": ["@rushdb/mcp-server"],
      "env": {
        "RUSHDB_API_KEY": "your-api-key-here"
      }
    }
  }
}
```

Place this in:

- **Claude Desktop** — `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Cursor** — MCP settings in the Cursor config panel
- **Windsurf** — `~/.codeium/windsurf/mcp_config.json`

**3. That's it.** The agent now has 13 tools for storing, querying, and traversing structured memory.

> `RUSHDB_API_URL` is optional and defaults to `https://api.rushdb.com/api/v1`. Override for self-hosted or staging environments.

---

## What an agent can do

After connecting, ask Claude or your agent naturally:

> "Remember that we decided to use Clerk for auth, replacing Auth0, on 2026-04-10."

The agent calls `CreateRecord` to store a structured memory. Later:

> "What auth decisions have we made?"

The agent calls `FindRecords` with a semantic query — no manual searching required.

> "Show me everything related to our auth system."

The agent calls `FindRelationships` to traverse the graph of connected records.

---

## Available tools

### Discovery

| Tool                | What it does                         |
| ------------------- | ------------------------------------ |
| `FindLabels`        | List record labels and record counts |
| `FindProperties`    | List properties across labels        |
| `FindRelationships` | Search relationships between records |

### Records

| Tool                | What it does                                                   |
| ------------------- | -------------------------------------------------------------- |
| `CreateRecord`      | Store a new record                                             |
| `GetRecord`         | Retrieve a record by ID                                        |
| `UpdateRecord`      | Update an existing record                                      |
| `DeleteRecord`      | Delete a record by ID                                          |
| `FindRecords`       | Search records by properties, relationships, or semantic query |
| `BulkCreateRecords` | Create multiple records at once                                |
| `BulkDeleteRecords` | Delete records matching a query                                |

### Relationships

| Tool             | What it does                              |
| ---------------- | ----------------------------------------- |
| `AttachRelation` | Create a relationship between two records |
| `DetachRelation` | Remove a relationship                     |

### Utilities

| Tool                    | What it does                                                                                        |
| ----------------------- | --------------------------------------------------------------------------------------------------- |
| `ExportRecords`         | Export records to CSV                                                                               |
| `HelpAddToClient`       | Get setup instructions for your MCP client                                                          |
| `GetQueryBuilderPrompt` | Returns the built-in query builder system prompt (fallback for clients without MCP Prompts support) |

---

## Built-in query builder prompt

RushDB exposes a system prompt via the MCP Prompts API that teaches the agent to query safely — discover labels and properties first, then construct queries.

**How it works:**

1. Client calls `ListPrompts` → finds `rushdb.queryBuilder`
2. Client calls `GetPrompt` → injects it as the session system message
3. Agent now does discovery-first queries automatically

Most MCP clients handle this at session start. If your client doesn't support MCP Prompts, call `GetQueryBuilderPrompt` and inject the result as your system message.

---

## Environment variables

These apply to local (API key) mode only. Remote OAuth connections do not use environment variables.

| Variable         | Required | Description                                             |
| ---------------- | -------- | ------------------------------------------------------- |
| `RUSHDB_API_KEY` | yes      | Your RushDB API key                                     |
| `RUSHDB_API_URL` | no       | API base URL (default: `https://api.rushdb.com/api/v1`) |

---

## Registry & autodiscovery

- `mcp.yaml` at the repo root enables autodiscovery by MCP registries
- `packages/mcp-server/glama.json` contains Glama metadata
- Package: `@rushdb/mcp-server`, command: `npx`

---

## Development

```bash
git clone https://github.com/rush-db/rushdb
cd rushdb/packages/mcp-server
npm install
npm run build
```

---

## License

Apache 2.0
