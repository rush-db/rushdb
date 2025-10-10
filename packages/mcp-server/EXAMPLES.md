# Example Configuration

Here are example configurations for different MCP clients:

## Claude Desktop

Add this to your `claude_desktop_config.json` file:

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

## VS Code MCP Extension

If you're using a VS Code MCP extension, add this to your settings:

```json
{
  "mcp.servers": {
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

## Example Usage

Once configured, you can use the RushDB MCP server like this:
### Use the Query Builder prompt (recommended)

If your client supports MCP Prompts:

1. List prompts and select `rushdb.queryBuilder`.
2. Ask: `How many workers in my company?`

If your client does not support Prompts:

1. Call `GetQueryBuilderPrompt`.
2. Paste the response as the session system message, then ask your question.


### Create a record
```
Create a new customer record with name "John Doe", email "john@example.com", and age 30
```

### Find records
```
Find all customers with age greater than 25
```

### Create relationships
```
Attach a "PURCHASED" relationship from customer ID abc123 to product ID def456
```

### Export data
```
Export all customer records to CSV format
```
