# RushDB MCP Server

A Model Context Protocol server providing access to RushDB's Labeled Meta Property Graph (LMPG) database.

## Features

- **Record Management**: Create, read, update, and delete records
- **Graph Operations**: Attach and detach relationships between records
- **Advanced Querying**: Search across records using RushDB's flexible query language
- **Label & Property Discovery**: Browse labels and properties in your database
- **Bulk Operations**: Efficient bulk create and delete operations
- **Data Export**: Export records to CSV format
- **LMPG Architecture**: Work with RushDB's revolutionary property-first graph model

## Quick Start

1. **Install the package**:
   ```bash
   npm install -g @rushdb/mcp-server
   ```

2. **Get your RushDB API key** from [app.rushdb.com](https://app.rushdb.com)

3. **Configure your MCP client** (e.g., Claude Desktop):
   ```json
   {
     "mcpServers": {
       "rushdb": {
         "command": "npx",
         "args": ["@rushdb/mcp-server"],
         "env": {
            "RUSHDB_API_KEY": "your-rushdb-api-key-here",
            "RUSHDB_API_URL": "https://api.rushdb.com/api/v1"
         }
       }
     }
   }
   ```

   Note: `RUSHDB_API_URL` is optional and defaults to `https://api.rushdb.com/api/v1`. Override it for self-hosted or staging environments.

## Available Tools

### Database Discovery
- `FindLabels` - List / filter record labels and their counts
- `FindProperties` - List / filter properties
- `FindRelationships` - Search for relationships

### Record Operations
- `CreateRecord` - Create a new record
- `UpdateRecord` - Update an existing record
- `DeleteRecord` - Delete a record by ID
- `GetRecord` - Retrieve a record by ID
- `FindRecords` - Search for records using query conditions

### Relationship Management
- `AttachRelation` - Create relationships between records
- `DetachRelation` - Remove relationships between records
- `FindRelationships` - Search for relationships

### Bulk Operations
- `BulkCreateRecords` - Create multiple records at once
- `BulkDeleteRecords` - Delete multiple records matching a query

### Data Export
- `ExportRecords` - Export records to CSV format

### Utilities
- `OpenBrowser` - Open URLs in browser
- `HelpAddToClient` - Get setup instructions
- `GetQueryBuilderPrompt` - Returns the RushDB Query Builder system prompt (fallback for clients without MCP Prompts support)

## Built-in Query Builder Prompt (MCP Prompts)

This server exposes a system prompt via the MCP Prompts API to ensure discovery-first, schema-safe querying:

- Prompt name: `rushdb.queryBuilder`
- Purpose: guides the model to discover labels/properties first (FindLabels/FindProperties), then construct validated SearchQuery objects before calling find-related tools.

How clients should use it:

1) Call `ListPrompts` and look for `rushdb.queryBuilder`.
2) Call `GetPrompt` with that name and set the returned system message for the model session that will use RushDB tools.

Most MCP clients can do this automatically at session start. If your client does not yet support Prompts, fetch this prompt once and inject it as the conversation’s system message before using RushDB tools.

Fallback tool for non-Prompts clients:

- Call `GetQueryBuilderPrompt` and set the returned text as your session’s system message.

## Registry & Autodiscovery

- Manifest: a top-level `mcp.yaml` is provided so MCP registries and clients can auto-discover this server.
- Glama: see `packages/mcp-server/glama.json` for basic metadata.
- When registering, use package `@rushdb/mcp-server`, command `npx`, and ensure `RUSHDB_API_KEY` is set.

## Environment Variables

- `RUSHDB_API_KEY` - Your RushDB API key (required)
- `RUSHDB_API_URL` - RushDB API base URL (optional, defaults to https://api.rushdb.com/api/v1). Useful for self-hosted, on-prem, or staging deployments.

## About RushDB's LMPG Architecture

RushDB uses a revolutionary Labeled Meta Property Graph (LMPG) architecture where:

- **Properties are first-class citizens** with their own nodes
- **Records are connected through shared properties**
- **Relationships emerge automatically** based on property overlap
- **No rigid schemas** - data structure evolves naturally
- **Cross-domain insights** through property traversal

This enables unprecedented flexibility in data modeling and querying.

## Development

To build from source:

```bash
git clone <repository>
cd rushdb/packages/mcp-server
npm install
npm run build
```

## License

Apache 2.0
