#!/usr/bin/env node
import{Server as he}from"@modelcontextprotocol/sdk/server/index.js";import{StdioServerTransport as be}from"@modelcontextprotocol/sdk/server/stdio.js";import{CallToolRequestSchema as Re,ErrorCode as oe,ListToolsRequestSchema as Ie,McpError as se,ListPromptsRequestSchema as we,GetPromptRequestSchema as Be}from"@modelcontextprotocol/sdk/types.js";var k=[{name:"FindLabels",description:"Find / filter record labels (supports where, limit, skip, orderBy). Superset of GetLabels.",inputSchema:{type:"object",properties:{where:{type:"object",description:"Filter conditions for labels (e.g., by activity flags, counts)"},limit:{type:"number",description:"Maximum number of labels to return"},skip:{type:"number",description:"Number of labels to skip"},orderBy:{type:"object",description:"Sorting configuration: key = field, value = asc|desc",additionalProperties:{type:"string",enum:["asc","desc"]}}},required:[]}},{name:"CreateRecord",description:"Create a new record in the database",inputSchema:{type:"object",properties:{label:{type:"string",description:"Label for the record"},data:{type:"object",description:"The record data to insert"},transactionId:{type:"string",description:"Optional transaction ID for atomic creation"}},required:["label","data"]}},{name:"UpdateRecord",description:"Update an existing record (partial update)",inputSchema:{type:"object",properties:{recordId:{type:"string",description:"ID of the record to update"},label:{type:"string",description:"Label for the record"},data:{type:"object",description:"The updated (partial) record data"},transactionId:{type:"string",description:"Optional transaction ID for atomic update"}},required:["recordId","label","data"]}},{name:"DeleteRecord",description:"Delete a record from the database (alias of DeleteRecordById)",inputSchema:{type:"object",properties:{recordId:{type:"string",description:"ID of the record to delete"},transactionId:{type:"string",description:"Optional transaction ID for atomic deletion"}},required:["recordId"]}},{name:"FindRecords",description:"Find records in the database using a search query",inputSchema:{type:"object",properties:{labels:{type:"array",items:{type:"string"},description:"Filter by record labels"},where:{type:"object",description:"Search conditions for finding records"},limit:{type:"number",description:"Maximum number of records to return",default:10},skip:{type:"number",description:"Number of records to skip",default:0},orderBy:{type:"object",description:"Sorting configuration: key = field, value = asc|desc",additionalProperties:{type:"string",enum:["asc","desc"]}},aggregate:{type:"object",description:"Aggregation definitions (records only)",additionalProperties:{type:"object",properties:{fn:{type:"string",description:"Aggregation function (count,sum,avg,min,max,timeBucket)"},field:{type:"string",description:"Field to aggregate"},alias:{type:"string",description:"Optional alias override"},granularity:{type:"string",description:"For timeBucket, the time granularity (e.g., day, week, month, quarter, year)"}},required:["fn"]}},groupBy:{type:"array",items:{type:"string"},description:"Fields to group by (records only)"}},required:[]}},{name:"GetRecord",description:"Get a specific record by ID",inputSchema:{type:"object",properties:{recordId:{type:"string",description:"ID of the record to retrieve"}},required:["recordId"]}},{name:"GetRecordsByIds",description:"Get multiple records by their IDs",inputSchema:{type:"object",properties:{recordIds:{type:"array",items:{type:"string"},description:"Array of record IDs to retrieve"}},required:["recordIds"]}},{name:"AttachRelation",description:"Create a relationship between records (single or multiple targets)",inputSchema:{type:"object",properties:{sourceId:{type:"string",description:"ID of the source record"},targetId:{type:"string",description:"ID of one target record (deprecated if targetIds provided)"},targetIds:{type:"array",items:{type:"string"},description:"IDs of multiple target records"},relationType:{type:"string",description:"Type of the relationship"},direction:{type:"string",enum:["outgoing","incoming","bidirectional"],description:"Direction of the relationship",default:"outgoing"},transactionId:{type:"string",description:"Optional transaction ID for atomic relation creation"}},required:["sourceId"]}},{name:"DetachRelation",description:"Remove a relationship between records (single or multiple targets)",inputSchema:{type:"object",properties:{sourceId:{type:"string",description:"ID of the source record"},targetId:{type:"string",description:"ID of one target record (deprecated if targetIds provided)"},targetIds:{type:"array",items:{type:"string"},description:"IDs of multiple target records"},relationType:{type:"string",description:"Type of the relationship to remove"},direction:{type:"string",enum:["outgoing","incoming","bidirectional"],description:"Direction of the relationship",default:"outgoing"},transactionId:{type:"string",description:"Optional transaction ID for atomic relation removal"}},required:["sourceId"]}},{name:"FindRelationships",description:"Find relationships in the database",inputSchema:{type:"object",properties:{where:{type:"object",description:"Search conditions for finding relationships"},limit:{type:"number",description:"Maximum number of relationships to return",default:10},skip:{type:"number",description:"Number of relationships to skip",default:0},orderBy:{type:"object",description:"Sorting configuration: key = field, value = asc|desc",additionalProperties:{type:"string",enum:["asc","desc"]}}},required:[]}},{name:"BulkCreateRecords",description:"Create multiple records in a single operation",inputSchema:{type:"object",properties:{label:{type:"string",description:"Label for all records"},data:{type:"array",items:{type:"object"},description:"Array of record data to insert"},transactionId:{type:"string",description:"Optional transaction ID for atomic bulk creation"}},required:["label","data"]}},{name:"BulkDeleteRecords",description:"Delete multiple records matching a query",inputSchema:{type:"object",properties:{labels:{type:"array",items:{type:"string"},description:"Filter by record labels"},where:{type:"object",description:"Search conditions for records to delete"},transactionId:{type:"string",description:"Optional transaction ID for atomic bulk deletion"}},required:["where"]}},{name:"ExportRecords",description:"Export records to CSV format",inputSchema:{type:"object",properties:{labels:{type:"array",items:{type:"string"},description:"Filter by record labels"},where:{type:"object",description:"Search conditions for records to export"},limit:{type:"number",description:"Maximum number of records to export"},orderBy:{type:"object",description:"Sorting configuration for export",additionalProperties:{type:"string",enum:["asc","desc"]}}},required:[]}},{name:"OpenBrowser",description:"Open a web browser to a specific URL",inputSchema:{type:"object",properties:{url:{type:"string",description:"The URL to open"}},required:["url"]}},{name:"HelpAddToClient",description:"Help the user add the RushDB MCP server to their MCP client",inputSchema:{type:"object",properties:{},required:[]}},{name:"GetQueryBuilderPrompt",description:"Return the RushDB Query Builder system prompt. Use this if your MCP client does not support Prompts API.",inputSchema:{type:"object",properties:{},required:[]}},{name:"SetRecord",description:"Replace all fields of a record with provided values",inputSchema:{type:"object",properties:{recordId:{type:"string",description:"ID of the record to set"},label:{type:"string",description:"Label for the record"},data:{type:"object",description:"The new record data to set"},transactionId:{type:"string",description:"Optional transaction ID for atomic set"}},required:["recordId","label","data"]}},{name:"FindOneRecord",description:"Find a single record that matches the given search criteria",inputSchema:{type:"object",properties:{labels:{type:"array",items:{type:"string"},description:"Filter by record labels"},where:{type:"object",description:"Search conditions for finding the record"}},required:[]}},{name:"FindUniqRecord",description:"Find a unique record that matches the given search criteria",inputSchema:{type:"object",properties:{labels:{type:"array",items:{type:"string"},description:"Filter by record labels"},where:{type:"object",description:"Search conditions for finding the unique record"}},required:[]}},{name:"DeleteRecordById",description:"Delete a record by its ID",inputSchema:{type:"object",properties:{recordId:{type:"string",description:"ID of the record to delete"},transactionId:{type:"string",description:"Optional transaction ID for atomic deletion"}},required:["recordId"]}},{name:"PropertyValues",description:"Get values for a specific property",inputSchema:{type:"object",properties:{propertyId:{type:"string",description:"ID of the property to get values for"},query:{type:"string",description:"Optional search query for filtering values"},orderBy:{type:"string",enum:["asc","desc"],description:"Ordering for value results"},limit:{type:"number",description:"Max number of values to return"},skip:{type:"number",description:"Number of values to skip"}},required:["propertyId"]}},{name:"FindProperties",description:"Find properties in the database using a search query",inputSchema:{type:"object",properties:{where:{type:"object",description:"Search conditions for finding properties"},limit:{type:"number",description:"Maximum number of properties to return",default:10},skip:{type:"number",description:"Number of properties to skip",default:0},orderBy:{type:"object",description:"Sorting configuration: key = field, value = asc|desc",additionalProperties:{type:"string",enum:["asc","desc"]}}},required:[]}},{name:"FindPropertyById",description:"Find a specific property by ID",inputSchema:{type:"object",properties:{propertyId:{type:"string",description:"ID of the property to retrieve"}},required:["propertyId"]}},{name:"DeleteProperty",description:"Delete a property from the database",inputSchema:{type:"object",properties:{propertyId:{type:"string",description:"ID of the property to delete"}},required:["propertyId"]}},{name:"TransactionBegin",description:"Begin a new database transaction",inputSchema:{type:"object",properties:{ttl:{type:"number",description:"TTL in milliseconds"}},required:[]}},{name:"TransactionCommit",description:"Commit a database transaction",inputSchema:{type:"object",properties:{transactionId:{type:"string",description:"Transaction ID"}},required:["transactionId"]}},{name:"TransactionRollback",description:"Rollback a database transaction",inputSchema:{type:"object",properties:{transactionId:{type:"string",description:"Transaction ID"}},required:["transactionId"]}},{name:"TransactionGet",description:"Get information about a transaction",inputSchema:{type:"object",properties:{transactionId:{type:"string",description:"Transaction ID"}},required:["transactionId"]}},{name:"GetSettings",description:"Get the current database settings and configuration",inputSchema:{type:"object",properties:{},required:[]}}];import"dotenv/config";import u from"@rushdb/javascript-sdk";import{fileURLToPath as de}from"url";import{dirname as le,resolve as g}from"path";import $ from"fs";function pe(){if(process.env.RUSHDB_API_KEY&&process.env.RUSHDB_API_URL)return;let r=le(de(import.meta.url)),t=[g(process.cwd(),".env"),g(r,"../../.env"),g(r,"../.env")];for(let e of t)try{if($.existsSync(e)){let n=$.readFileSync(e,"utf8");for(let i of n.split(/\r?\n/)){if(!i||i.trim().startsWith("#"))continue;let s=i.indexOf("=");if(s===-1)continue;let a=i.slice(0,s).trim();if(a!=="RUSHDB_API_KEY"&&a!=="RUSHDB_API_URL")continue;let c=i.slice(s+1).trim();c&&(process.env[a]||=c)}if(process.env.RUSHDB_API_KEY&&process.env.RUSHDB_API_URL)break}}catch{}}pe();var T=process.env.RUSHDB_API_KEY,ue=process.env.RUSHDB_API_URL||"https://api.rushdb.com/api/v1";if(!T)throw new Error("RUSHDB_API_KEY environment variable is required. Set it in a .env file (packages/mcp-server/.env) or export it before running the server.");var ge=u?.default||u?.RushDB||u,o=new ge(T,{url:ue});async function P(r={}){let{where:t,limit:e,skip:n,orderBy:i}=r,s={};t&&(s.where=t),typeof e=="number"&&(s.limit=e),typeof n=="number"&&(s.skip=n),i&&Object.keys(i).length>0&&(s.orderBy=i);let a=await o.labels.find(s);return a?.success&&a.data?Object.entries(a.data).map(([c,d])=>({name:c,count:d})):[]}async function A(r){let{label:t,data:e,transactionId:n}=r;return{success:!0,id:(await o.records.create({label:t,data:e},n)).id(),message:`Record created successfully with label '${t}'`}}async function q(r){let{recordId:t,label:e,data:n,transactionId:i}=r;return await o.records.update({target:t,label:e,data:n},i),{success:!0,message:"Record updated successfully"}}async function O(r){let{recordId:t,transactionId:e}=r;return await o.records.deleteById(t,e),{success:!0,message:`Record '${t}' deleted successfully`}}async function E(r){let{labels:t,where:e,limit:n=10,skip:i=0,orderBy:s,aggregate:a,groupBy:c}=r,d={};t&&t.length>0&&(d.labels=t),e&&(d.where=e),n&&(d.limit=n),i&&(d.skip=i),s&&Object.keys(s).length>0&&(d.orderBy=s),a&&Object.keys(a).length>0&&(d.aggregate=a),c&&c.length>0&&(d.groupBy=c);let p=await o.records.find(d);return d.aggregate||d.groupBy?p:p.data.map(y=>y.data)}async function F(r){let{recordId:t}=r;return(await o.records.findById(t)).data}async function C(r){let{sourceId:t,targetId:e,targetIds:n,relationType:i,direction:s="outgoing",transactionId:a}=r,c={};i&&(c.type=i),s&&(c.direction=s);let d=n&&n.length>0?n:e?[e]:[];return d.length===0?{success:!1,message:"No targetId(s) provided"}:(await o.records.attach({source:t,target:d,options:c},a),{success:!0,message:`Relationship attached from '${t}' to ${d.length} target record(s)`})}async function N(r){let{sourceId:t,targetId:e,targetIds:n,relationType:i,direction:s="outgoing",transactionId:a}=r,c={};i&&(c.typeOrTypes=i),s&&(c.direction=s);let d=n&&n.length>0?n:e?[e]:[];return d.length===0?{success:!1,message:"No targetId(s) provided"}:(await o.records.detach({source:t,target:d,options:c},a),{success:!0,message:`Relationship detached from '${t}' to ${d.length} target record(s)`})}async function L(r){let{where:t,limit:e=10,skip:n=0,orderBy:i}=r,s={limit:e,skip:n};t&&(s.where=t),i&&Object.keys(i).length>0&&(s.orderBy=i);let a=await o.relationships.find(s);return a.success&&a.data?a.data:[]}async function j({label:r,data:t,transactionId:e}){let i=(await o.records.createMany({label:r,data:t},e)).data.map(s=>s.id());return{message:`Successfully created ${i.length} records with label '${r}'`,ids:i}}async function U({labels:r,where:t,transactionId:e}){let n={where:t};return r&&r.length>0&&(n.labels=r),{message:(await o.records.delete(n,e)).data?.message||"Records deleted successfully"}}async function G(r){let{labels:t,where:e,limit:n}=r,i={};t&&t.length>0&&(i.labels=t),e&&(i.where=e),n&&(i.limit=n);let s=await o.records.export(i);return s.success&&s.data?{csv:s.data.fileContent,dateTime:s.data.dateTime,message:"Records exported successfully"}:{csv:"",dateTime:new Date().toISOString(),message:"No records found to export"}}import{execFile as me}from"child_process";import{platform as M}from"os";import{URL as ye}from"url";async function _(r){let{url:t}=r,e;try{let i=new ye(t);if(!["http:","https:"].includes(i.protocol))return{success:!1,message:"Invalid URL protocol. Only http and https are supported."};e=i.toString()}catch(i){return{success:!1,message:`Invalid URL format: ${i instanceof Error?i.message:String(i)}`}}let n=M()==="darwin"?"open":M()==="win32"?"start":"xdg-open";return new Promise(i=>{me(n,[e],s=>{i(s?{success:!1,message:`Failed to open browser: ${s.message}`}:{success:!0,message:`Successfully opened ${e} in default browser`})})})}async function H(){return{instructions:`To add the RushDB MCP server to your MCP client, follow these steps:

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

For more information, visit https://docs.rushdb.com`}}async function Q(r){let{recordId:t,label:e,data:n,transactionId:i}=r;return await o.records.set({target:t,label:e,data:n},i),{success:!0,message:`Record '${t}' set successfully with label '${e}'`}}async function Y(r){let{labels:t,where:e}=r;return(await o.records.findOne({...t&&{labels:t},...e&&{where:e}}))?.data||null}async function V(r){let{labels:t,where:e}=r;return(await o.records.findUniq({...t&&{labels:t},...e&&{where:e}}))?.data||null}async function J(r){let{recordId:t,transactionId:e}=r;return await o.records.deleteById(t,e),{success:!0,message:`Record '${t}' deleted successfully`}}async function K(r){let{propertyId:t,query:e,orderBy:n,limit:i,skip:s}=r,a={};return e&&(a.query=e),n&&(a.orderBy=n),i&&(a.limit=i),s&&(a.skip=s),(await o.properties.values(t,a)).data}async function W(r){let{where:t,limit:e,skip:n,orderBy:i}=r,s={};return t&&(s.where=t),e&&(s.limit=e),n&&(s.skip=n),i&&Object.keys(i).length>0&&(s.orderBy=i),(await o.properties.find(s)).data}async function z(r){let{propertyId:t}=r;return(await o.properties.findById(t)).data}async function Z(r){let{propertyId:t}=r;return await o.properties.delete(t),{success:!0,message:`Property '${t}' deleted successfully`}}async function X(r){let{ttl:t}=r,e=t?{ttl:t}:void 0,n=await o.tx.begin(e);return{success:!0,transactionId:n.id,message:`Transaction started with ID: ${n.id}`}}async function ee(r){let{transactionId:t}=r,e=await o.tx.commit(t);return{success:!0,message:`Transaction '${t}' committed successfully`,data:e.data}}async function te(r){let{transactionId:t}=r,e=await o.tx.rollback(t);return{success:!0,message:`Transaction '${t}' rolled back successfully`,data:e.data}}async function re(r){let{transactionId:t}=r;return{id:(await o.tx.get(t)).id,message:"Transaction information retrieved successfully"}}async function ne(){return(await o.settings.get()).data}async function ie(r){let{recordIds:t}=r;if(!Array.isArray(t)||t.length===0)return{success:!1,message:"recordIds must be a non-empty array",data:[]};let e=await o.records.findById(t);return{success:!0,count:e.data.length,data:e.data.map(n=>n.data)}}var fe=`You are an autonomous RushDB Query Builder. Convert natural\u2011language requests into validated RushDB SearchQuery objects and invoke the correct MCP tools without hallucination.

Core SearchQuery (records): labels[], where{}, limit?, skip?, orderBy{}, aggregate{}, groupBy[].
Properties / Relationships reuse same shape but currently DO NOT support aggregate/groupBy (ignore those keys there). Aggregations & grouping are ONLY for FindRecords.

GENERAL PRINCIPLES
1) Discovery First: If label or field uncertainty exists, call FindLabels & FindProperties then FindRelations (optionally) before constructing complex filters.
2) Incremental Construction: Start minimal; only add complexity (relationships, grouping, vector ops) when user intent demands.
3) Deterministic Field Usage: Use only labels & properties actually discovered. Never invent names.
4) Separation of Concerns: Filters live in where. Sorting in orderBy. Metrics in aggregate (records only). Dimensions in groupBy (records only).
5) Safety: For destructive operations (BulkDeleteRecords), first present the query; require confirmation; suggest a small preview (FindRecords with limit) before deletion.
6) Zero / Large Result Strategy: On zero results propose precise relaxations; on huge result sets propose narrowing or aggregation instead of blindly raising limit.
7) Output Discipline: Always emit valid JSON for query. Provide a short natural-language explanation after JSON. Never expose internal chain-of-thought.

TOOL MAP (exact MCP tool names \u2013 use these; do not invent alternatives)
- FindLabels: discover / filter available record labels and their counts.
- FindProperties: search / filter properties \u2013 no aggregate/groupBy.
- FindRecords: execute SearchQuery over records (only place where aggregate + groupBy are valid).
- FindProperties: SearchQuery over properties (no aggregate/groupBy; may reuse labels + where pattern).
- FindRelations: search relationships (where + limit + orderBy only; no aggregate/groupBy).
- PropertyValues: canonicalize / enumerate values for a specific propertyId (value resolution, enumeration, normalization).
- GetRecord / GetRecordsByIds / FindOneRecord / FindUniqRecord / DeleteRecord / DeleteRecordById / SetRecord / UpdateRecord / CreateRecord: CRUD helpers (not for complex aggregation logic).
- BulkCreateRecords: batch insert.
- BulkDeleteRecords: destructive delete using same SearchQuery structure (confirm first).
- ExportRecords: export matching records (respects where / labels / orderBy / aggregate / groupBy).
- AttachRelation / DetachRelation: relationship mutations.
- FindPropertyById / DeleteProperty: property management.
- TransactionBegin / TransactionCommit / TransactionRollback / TransactionGet: multi-operation atomicity.
- GetSettings: retrieve environment/config (not part of query building).
- OpenBrowser / HelpAddToClient: out-of-band assistance.

--------------------------------------------------
5) NATURAL LANGUAGE \u2192 where TRANSLATION RULES
\u2022 Extract explicit numeric limits ("top 5", "first 3").
\u2022 Default limit=10 ONLY for record listing scenarios (tables, explicit list/show, browsing, entity resolution). For pure aggregation-only queries (only aggregated metrics, no raw rows requested) DO NOT set a limit unless the user explicitly gives one.
\u2022 Numeric normalization: 1k\u21921000, 1m\u21921000000, 1b\u21921000000000. Strip currency symbols ($100k \u2192 100000). Accept underscores or commas.
\u2022 Equality / sets:
	field: value
	Not equal: { field: { $ne: value } }
	One of: { field: { $in: [v1,v2,...] } }
	Not in: { field: { $nin: [v1,v2,...] } }
\u2022 Numbers:
	> => $gt, >= => $gte, < => $lt, <= => $lte
	Between X and Y => { $gte: X, $lte: Y }
\u2022 Strings (case-insensitive): $contains / $startsWith / $endsWith plus $in/$nin/$ne.
\u2022 Booleans: field: true|false or { field: { $ne: value } }.
\u2022 Datetime:
	- Prefer range boundaries (component objects) for phrases like "in 2023", "in Jan 2024", "on 2024-05-12".
		Year: { field: { $gte: { $year: 2023 }, $lt: { $year: 2024 } } }
		Month: { field: { $gte: { $year: 2024, $month: 1 }, $lt: { $year: 2024, $month: 2 } } }
		Day: { field: { $gte: { $year: 2024, $month: 5, $day: 12 }, $lt: { $year: 2024, $month: 5, $day: 13 } } }
	- Decades: "1990s" => { $gte:{ $year:1990 }, $lt:{ $year:2000 } } ; "2010s" similarly.
	- Two\u2011digit decade ("'90s"): assume 1990\u20132000 unless ambiguity critical (then ask concise question).
	- Relative windows (last 7 days, today, yesterday, this week/month/year): compute start boundary (UTC) and use $gte.
	- Month+day without year ("on Oct 6"): unsupported; ask user to supply year (plain brief note, no internals).
	- Comparisons & sets: $gt/$gte/$lt/$lte/$ne + $in/$nin supported on datetimes and components.
\u2022 Field checks: { field: { $exists: true|false } }, { field: { $type: 'string'|'number'|'boolean'|'datetime'|'null'|'vector' } }.
\u2022 Vectors: { field: { $vector: { fn, query:number[], threshold:number|{ $gte|$lte|$ne } } } } ; threshold default interpretation: euclidean/euclideanDistance => $lte; others => $gte.
\u2022 Logical grouping: $and, $or, $not, $nor, $xor (nest as needed). Prefer implicit AND when simple.
\u2022 Relationships:
	- Related label reference: { RELATED_LABEL: { ...conditions... } }
	- Direction/type: { RELATED_LABEL: { $relation:{ type:'REL', direction:'in'|'out' }, ... } }
	- ID filtering: $id with direct value or operators ($in etc.).
	- Aliasing: include $alias inside related label object when that related node is used in aggregation.
\u2022 Field names case-sensitive; string comparisons case-insensitive by default.

Example: "List 10 deals with amount > 100k" \u2192 labels:['DEAL']; where:{ amount:{ $gt:100000 } }; limit:10.

5.1) Example (salary filter): Input "List employees with salary below 60K" \u2192 Discover EMPLOYEE \u2192 locate numeric salary (fallback to compensation/baseSalary synonyms) \u2192 where:{ salary:{ $lt:60000 } } \u2192 limit:10. If salary absent choose best synonym, state brief assumption.

--------------------------------------------------
6) AGGREGATIONS & GROUPING (records only)
\u2022 Aggregations reside under aggregate. groupBy supplies dimensional keys.
\u2022 Pure aggregation (no listing requested): DO NOT set limit.
\u2022 Counting only (overall total) \u2013 rely on aggregate if user explicitly asks for metric; otherwise you can read total from response but if user phrased as a metric ("what is average...", "min / max / avg ..."), provide explicit aggregate.
\u2022 Breakdown intent tokens: by / per / grouped / distribution / count by / avg ... by / sum ... by / each ... . Must produce aggregate + groupBy.
\u2022 Every aggregate entry MUST include alias referencing source node:
	- '$record' for root records.
	- Related alias (declared via $alias in where) for related-node metrics.
\u2022 Common metric shapes:
	totalAmount:{ fn:'sum', field:'amount', alias:'$record' }
	avgAmount:{ fn:'avg', field:'amount', precision:2, alias:'$record' }
	maxAmount/minAmount same pattern.
	dealsCount:{ fn:'count', alias:'$record' }
	collect full rows: items:{ fn:'collect', alias:'$record', limit:10, orderBy:{ amount:'desc' } }
\u2022 Time bucket (fn:'timeBucket'): detect phrases (per day/week/month/quarter/year, every N months, quarterly, semi-annual, etc.). Steps:
	1) Pick appropriate datetime field (prefer closedAt > completedAt > updatedAt > createdAt > publishedAt if unspecified).
	2) aggregate:{ bucketKey:{ fn:'timeBucket', field:'<dateField>', granularity:'day'|'week'|'month'|'quarter'|'year'|'months', size?:N, alias:'$record' }, metric:{ fn:'count'|... alias:'$record' } }
	3) groupBy:[ 'bucketKey' ]; orderBy:{ bucketKey:'asc' }.
\u2022 No top-level limit for time bucket or pure group aggregates unless user gives explicit limit (top N groups).
\u2022 Ranking tokens (top, largest, most, highest, lowest, smallest) \u21D2 orderBy aggregate metric accordingly (desc for top/highest/largest/most; asc for lowest/smallest/min-oriented context).
\u2022 If user asks for both listing + metric: include limit ONLY for listing portion; aggregate unaffected.
\u2022 NEVER put group key inside aggregate entry; group keys only in groupBy.
\u2022 groupBy must have at least one aggregation present; no groupBy alone.
\u2022 Self-group pattern (overall multi-metric aggregates) \u2013 include groupBy containing at least one aggregation key (choose central metric). Required to avoid unintended row projection. Do not invent synthetic fields.
\u2022 Related-node aggregations require alias established in where via $alias.
\u2022 Late ordering: prefer ordering by aggregate keys for ranked group results (ensures ordering after full aggregation).

6.1) Grouped aggregation detection: tokens 'by <field>', 'per <field>', 'grouped by', 'distribution by', 'count by', 'avg ... by', 'sum ... by'. Build groupBy with fully qualified keys ('$record.status', '$customer.industry'). Always at least one aggregate entry.

6.2) METRIC FIELD DISCOVERY ACROSS RELATED LABELS
If metric field not on root label, search related labels (one hop, then two) before asking user. Use synonyms (bedrooms, beds, price/amount/value, revenue/sales, profit/earnings, area/sqft...). When metric found on related label CHILD, add { CHILD_LABEL:{ $alias:'$child', ...filters } } under where and aggregate referencing '$child'. Maintain filters for root-level adjectives (e.g., status) at root. No limit on pure aggregation.

6.3) MULTI-LABEL FILTER DISTRIBUTION
Place each filter with the label that actually holds the property. Attempt silent remediation (move filter) before asking clarification.

6.4) ENUM / VALUE-SET & GEOGRAPHIC NORMALIZATION
Never guess enumerated values. Use PropertyValues (PropertyValues tool) to resolve canonical values (status, state, city, category, type, industry...). Ranking resolution: exact (case-insensitive) > startsWith > contains; tie-break longer match then lexical. Map user-supplied value to canonical; mention assumption briefly. If user says "all <enum>", omit that field. For ambiguous zero matches, broaden query or show 1\u20132 plausible candidates then ask concise clarification.

--------------------------------------------------
7) RELATIONSHIP & PATH QUERIES
\u2022 Determine target label (what user wants listed or aggregated) vs related labels (filters or dimensions).
\u2022 Use related label objects in where; only include $relation if direction/type specified.
\u2022 Entity resolution: when named entity appears (project, user, department), probe candidate labels + identifier fields (name/title/code) via FindRecords small samples using $contains to confirm.
\u2022 Vague short queries: proactively attempt plausible label mappings; return preliminary results + (if required) one brief clarifying question.

7.1) MULTI-HOP + NESTED AGGREGATION
When counting or aggregating a related label per parent, prefer direct relationship if present; only BFS discover intermediate labels if necessary (depth \u22644). Build shortest path; assign $alias to terminal label only. Example pattern: labels:['PARENT'], where:{ CHILD_LABEL:{ $alias:'$child' } }, aggregate:{ childCount:{ fn:'count', alias:'$child' } }. Avoid redundant wrappers.

--------------------------------------------------
VALIDATION & NORMALIZATION
\u2022 Normalize partial user JSON: scalars at top moved into where; add default limit only for listing scenarios. Remove unsupported keys for non-record tools.
\u2022 Check: no groupBy without aggregate; alias present on every aggregate; group keys not inside aggregate; vector threshold semantics correct.
\u2022 If month+day without year given: request year.
\u2022 If ambiguous label or multiple metric synonyms remain after automatic attempts, ask one concise clarification.

ERROR & RESULT HANDLING
\u2022 Zero results: propose 1\u20132 specific relaxations (e.g., widen price range) before asking user.
\u2022 Large results w/o aggregation: suggest grouping or adding filters before raising limit.
\u2022 Destructive (BulkDeleteRecords): always preview and confirm.

OUTPUT FORMAT
1. JSON SearchQuery first (only the query object, not wrapped in prose).
2. Brief explanation (<3 sentences) stating: target label(s), filter summary, metrics/grouping (if any), assumptions.

EXAMPLES
Top 5 products with price between 10 and 50 ascending price:
{ "labels":["PRODUCT"], "where":{ "price":{ "$gte":10, "$lte":50 } }, "orderBy":{ "price":"asc" }, "limit":5 }

Count employees per department with headcount > 10 (grouped):
{ "labels":["DEPARTMENT"], "where":{ "headcount":{ "$gt":10 } }, "aggregate":{ "deptCount":{ "fn":"count", "alias":"$record" } }, "groupBy":["$record.name"], "orderBy":{ "deptCount":"desc" } }

If uncertain label (user: "vendors" dataset has 'SUPPLIER'): map to SUPPLIER and note assumption.

REMEMBER: Fractal reuse \u2013 same SearchQuery reused across records, properties (minus aggregate/groupBy), relations (minus aggregate/groupBy), export, bulk delete.
`,m=fe;var l=new he({name:"rushdb-mcp-server",version:"1.0.0"},{capabilities:{tools:{list:!0,call:!0},prompts:{}}});l.setRequestHandler(Ie,async()=>({tools:k}));l.setRequestHandler(we,async()=>({prompts:[{name:"rushdb.queryBuilder",description:"RushDB Query Builder system prompt: guides the model to discover labels/properties first and construct validated SearchQuery objects before calling find-related tools.",arguments:[]}]}));l.setRequestHandler(Be,async r=>{let t=r.params.name;if(t!=="rushdb.queryBuilder")throw new se(oe.InvalidRequest,`Unknown prompt: ${t}`);return{description:"RushDB Query Builder system prompt to enable discovery-first, schema-safe SearchQuery construction before find-related tool calls.",messages:[{role:"user",content:{type:"text",text:m}}]}});l.setRequestHandler(Re,async r=>{let t=r.params.name,e=r.params.arguments||{};try{switch(t){case"FindLabels":let n=await P({where:e.where,limit:e.limit,skip:e.skip,orderBy:e.orderBy});return{content:[{type:"text",text:n.length>0?n.map(D=>`${D.name}: ${D.count} records`).join(`
`):"No labels found"}]};case"CreateRecord":let i=await A({label:e.label,data:e.data,transactionId:e.transactionId});return{content:[{type:"text",text:`${i.message}
ID: ${i.id}`}]};case"UpdateRecord":return{content:[{type:"text",text:(await q({recordId:e.recordId,label:e.label,data:e.data,transactionId:e.transactionId})).message}]};case"DeleteRecord":return{content:[{type:"text",text:(await O({recordId:e.recordId,transactionId:e.transactionId})).message}]};case"FindRecords":let c=await E({labels:e.labels,where:e.where,limit:e.limit,skip:e.skip,orderBy:e.orderBy,aggregate:e.aggregate,groupBy:e.groupBy}),d=!!e.aggregate||!!e.groupBy;return{content:[{type:"text",text:Array.isArray(c)&&c.length===0?"No matching records found.":JSON.stringify(c,null,2)}]};case"GetRecord":let p=await F({recordId:e.recordId});return{content:[{type:"text",text:JSON.stringify(p,null,2)}]};case"AttachRelation":return{content:[{type:"text",text:(await C({sourceId:e.sourceId,targetId:e.targetId,targetIds:e.targetIds,relationType:e.relationType,direction:e.direction,transactionId:e.transactionId})).message}]};case"DetachRelation":return{content:[{type:"text",text:(await N({sourceId:e.sourceId,targetId:e.targetId,targetIds:e.targetIds,relationType:e.relationType,direction:e.direction,transactionId:e.transactionId})).message}]};case"FindRelationships":let f=await L({where:e.where,limit:e.limit,skip:e.skip,orderBy:e.orderBy});return{content:[{type:"text",text:f.length>0?JSON.stringify(f,null,2):"No relations found"}]};case"BulkCreateRecords":let h=await j({label:e.label,data:e.data,transactionId:e.transactionId});return{content:[{type:"text",text:`${h.message}
IDs: ${h.ids.join(", ")}`}]};case"BulkDeleteRecords":return{content:[{type:"text",text:(await U({labels:e.labels,where:e.where,transactionId:e.transactionId})).message}]};case"ExportRecords":let b=await G({labels:e.labels,where:e.where,limit:e.limit});return{content:[{type:"text",text:`Export completed at ${b.dateTime}

${b.csv}`}]};case"OpenBrowser":return{content:[{type:"text",text:(await _({url:e.url})).message}]};case"HelpAddToClient":return{content:[{type:"text",text:(await H()).instructions}]};case"GetQueryBuilderPrompt":return{content:[{type:"text",text:m}]};case"SetRecord":return{content:[{type:"text",text:(await Q({recordId:e.recordId,label:e.label,data:e.data,transactionId:e.transactionId})).message}]};case"FindOneRecord":let R=await Y({labels:e.labels,where:e.where});return{content:[{type:"text",text:R?JSON.stringify(R,null,2):"No matching record found."}]};case"FindUniqRecord":let I=await V({labels:e.labels,where:e.where});return{content:[{type:"text",text:I?JSON.stringify(I,null,2):"No unique record found."}]};case"DeleteRecordById":return{content:[{type:"text",text:(await J({recordId:e.recordId,transactionId:e.transactionId})).message}]};case"PropertyValues":let w=await K({propertyId:e.propertyId,query:e.query,orderBy:e.orderBy,limit:e.limit,skip:e.skip});return{content:[{type:"text",text:w?JSON.stringify(w,null,2):"No property values found"}]};case"FindProperties":let B=await W({where:e.where,limit:e.limit,skip:e.skip,orderBy:e.orderBy});return{content:[{type:"text",text:B.length>0?JSON.stringify(B,null,2):"No properties found"}]};case"GetRecordsByIds":let v=await ie({recordIds:e.recordIds});return{content:[{type:"text",text:v.count>0?JSON.stringify(v.data,null,2):"No records found"}]};case"FindPropertyById":let x=await z({propertyId:e.propertyId});return{content:[{type:"text",text:x?JSON.stringify(x,null,2):"Property not found"}]};case"DeleteProperty":return{content:[{type:"text",text:(await Z({propertyId:e.propertyId})).message}]};case"TransactionBegin":let S=await X({ttl:e.ttl});return{content:[{type:"text",text:`${S.message}
Transaction ID: ${S.transactionId}`}]};case"TransactionCommit":return{content:[{type:"text",text:(await ee({transactionId:e.transactionId})).message}]};case"TransactionRollback":return{content:[{type:"text",text:(await te({transactionId:e.transactionId})).message}]};case"TransactionGet":let ae=await re({transactionId:e.transactionId});return{content:[{type:"text",text:JSON.stringify(ae,null,2)}]};case"GetSettings":let ce=await ne();return{content:[{type:"text",text:JSON.stringify(ce,null,2)}]};default:throw new se(oe.MethodNotFound,"Tool not found")}}catch(n){return console.error("Error executing tool:",n),n instanceof Error&&(n.message.includes("RUSHDB_API_KEY")||n.message.includes("Invalid URL")||n.message.includes("Failed to fetch")||n.message.includes("Network error")||!process.env.RUSHDB_API_KEY)?{content:[{type:"text",text:"It seems like you haven't configured your RushDB credentials. Would you like me to open the RushDB dashboard for you so you can sign up and get your credentials?"}]}:{content:[{type:"text",text:`Error: ${n instanceof Error?n.message:String(n)}`}]}}});var ve=new be;await l.connect(ve);
