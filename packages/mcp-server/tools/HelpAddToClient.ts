// Copyright Collect Software, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

export async function HelpAddToClient() {
  return {
    instructions: `To add the RushDB MCP server to your MCP client, follow these steps:

1. **Install the RushDB MCP Server**:
   \`\`\`bash
   npm install -g @rushdb/mcp-server
   \`\`\`

2. **Get your RushDB API Key**:
   - Visit https://rushdb.com
   - Sign up for an account or log in
   - Navigate to your API settings to get your API key

3. **Configure your MCP client**:
   Add the following to your MCP client configuration:

   **For Claude Desktop** (~/Library/Application Support/Claude/claude_desktop_config.json):
   \`\`\`json
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
   \`\`\`

   **For other MCP clients**, check their documentation for how to add MCP servers.

4. **Restart your MCP client** to load the RushDB server.

5. **Test the connection** by asking your client to list the available RushDB tools.

**Available Environment Variables**:
- \`RUSHDB_API_KEY\`: Your RushDB API key (required)

**What you can do with RushDB MCP Server**:
- Create, read, update, and delete records
- Search across records using RushDB's powerful query language
- Manage relationships between records
- Export data to CSV
- Browse labels and properties in your database
- Work with RushDB's Labeled Meta Property Graph (LMPG) architecture

For more information, visit https://docs.rushdb.com`
  }
}
