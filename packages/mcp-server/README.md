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

2. **Get your RushDB API key** from [rushdb.com](https://rushdb.com)

3. **Configure your MCP client** (e.g., Claude Desktop):
   ```json
   {
     "mcpServers": {
       "rushdb": {
         "command": "npx",
         "args": ["@rushdb/mcp-server"],
         "env": {
           "RUSHDB_API_KEY": "your-rushdb-api-key-here"
         }
       }
     }
   }
   ```

## Available Tools

### Database Discovery
- `GetLabels` - List all record labels and their counts
- `GetProperties` - List all properties in the database

### Record Operations  
- `CreateRecord` - Create a new record
- `UpdateRecord` - Update an existing record
- `DeleteRecord` - Delete a record by ID
- `GetRecord` - Retrieve a record by ID
- `FindRecords` - Search for records using query conditions

### Relationship Management
- `AttachRelation` - Create relationships between records
- `DetachRelation` - Remove relationships between records  
- `FindRelations` - Search for relationships

### Bulk Operations
- `BulkCreateRecords` - Create multiple records at once
- `BulkDeleteRecords` - Delete multiple records matching a query

### Data Export
- `ExportRecords` - Export records to CSV format

### Utilities
- `OpenBrowser` - Open URLs in browser
- `HelpAddToClient` - Get setup instructions

## Environment Variables

- `RUSHDB_API_KEY` - Your RushDB API key (required)

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
