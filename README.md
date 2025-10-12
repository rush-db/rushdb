<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

### Developer‑first property‑centric graph store

RushDB lets you push raw JSON/CSV, auto-normalize it into a labeled meta property graph, and query records, schema, relationships, values, vectors and aggregations through one JSON search interface. No upfront schema design, no new query language to learn.
RushDB transforms how you work with graph data — no schema required, no complex queries, just push your data and go.

[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on X (Twitter)](https://img.shields.io/twitter/follow/rushdb?style=social)](https://x.com/RushDatabase)

[🌐 Website](https://rushdb.com) • [📖 Documentation](https://docs.rushdb.com) • [☁️ Cloud Platform](https://app.rushdb.com) • [🔍 Examples](https://github.com/rush-db/examples)

</div>

---
## ✨ Core Capabilities

- **Zero-Friction Ingest**: Push arbitrary nested JSON / CSV; RushDB performs BFS normalization into a labeled meta property graph (LMPG)
- **Unified Search DTO**: One JSON structure for filtering, traversing, grouping (`groupBy`), aggregating, vector similarity & introspection (records / labels / properties / values / relationships)
- **Property‑Centric Indexing**: Properties become first-class nodes ("HyperProperties") enabling dynamic schema discovery & integrity checks
- **Vector & Hybrid Search**: Embed, store, and query semantic similarity side‑by‑side with structured predicates
- **Aggregation & Grouping**: `aggregate` + `groupBy` for pivot tables, KPIs and multi-key rollups without pre-modeling
- **Self‑Describing Graph**: Auto-discover labels, properties, value ranges and relationship topology for UI generation & AI agents
- **Incremental Complexity**: Start with naïve data push; evolve into deep traversals & analytical queries—no migrations
- **Polyglot SDKs**: TypeScript/JavaScript, Python & REST all share identical semantics

## 💼 Use Cases

1. **AI Memory & Agentic Trails**
  Persist multi-turn conversations, reasoning chains and tool outputs. Property overlap inference auto-connects sessions, enabling retrieval, summarization and context window reconstruction without manual relationship modeling.

2. **Small Team → Big Ambition**
  Ship PoCs / MVPs fast: push raw data now, refine model later. Avoid schema migration churn while scaling query sophistication and data volume.

3. **Search & Intelligence over Connected Catalogs**
  Product catalogs, knowledge bases (RAG), fraud patterns, biotech entities: traverse, filter, vector match, aggregate & group in one DTO—no separate systems for metadata vs content.

4. **Make Flat Data Breathe**
  Turn CSV/JSON dumps into a navigable graph in minutes. BFS import + property linking + lightweight aliasing gives relationship intelligence without hand-curating edges.

5. **Fullstack Apps without CRUD Boilerplate**
  "What you see is what you push": nested create mirrors nested query. Auto-discovered labels/properties power dynamic forms, filters and analytics—no ORMs, no bespoke resolver layer.

6. **Operational + Analytical Unification**
  Real-time user-facing features and exploratory analytics run on the same query surface—no ETL lag or duplicated modeling.

7. **Adaptive Schema Exploration for AI Agents**
  Agents enumerate labels, properties & value distributions programmatically to self-orient and generate safe follow-up queries.

## 🧪 Architecture: LMPG & HyperProperties

Traditional property graphs bind you to early labeling, manual relationship planning, and separate APIs for schema vs data. RushDB’s **Labeled Meta Property Graph (LMPG)** model inverts this: properties are elevated to graph nodes (HyperProperties) capturing:

| Dimension | Tracked Meta (Examples) | Benefit |
|-----------|-------------------------|---------|
| Type distribution | inferred scalar kinds, vector dims | Detect drift & inconsistent ingestion |
| Cardinality & sparsity | distinct counts, null ratios | Index heuristics & UI filter hints |
| Value ranges / histograms | min / max / buckets | Instant range sliders & anomaly bounds |
| Relationship incidence | cross-label co-occurrence | Emergent data model visualization |
| Temporal evolution | first/last seen timestamps | Freshness & lineage checks |

### Data Observability
One JSON SearchQuery can retrieve perspectives (records, labels, properties, value ranges, relationships) sharing identical filter semantics. This unified projection removes the “N+1 inspection” pattern and enables:
* Schema drift detection (compare property sets over time)
* Automatic filter UI generation (value domains & type inference)
* Programmatic cataloging for governance & AI agents

### Data Integrity
HyperProperties enable soft constraints without rigid upfront schemas:
* Type cohesion scoring & anomaly flagging
* Optional uniqueness & frequency analysis guiding index strategy
* Automatic relationship inference via shared property linkage (reduces orphan edges)
* Consistent vector dimension enforcement at ingestion

### Query Semantics & Data Model
Let G = (R, P, E_rp, E_pp) where R are record nodes, P are property (HyperProperty) nodes. Edges encode (record ↔ property assignments) and (property ↔ property co-occurrence). Query evaluation leverages property-level adjacency to: (a) prune traversal (perforating execution), (b) derive emergent schema, (c) compose multi-perspective responses in a single pass.

This yields sub-linear schema introspection relative to naïve per-aspect scans because shared filter evaluation is amortized across all requested perspectives.

### Example: Single DTO, Multiple Perspectives
```ts
// Pseudocode: obtain records + property stats + grouped aggregations
const result = await db.records.find({
  labels: ['TRANSACTION'],
  where: { status: 'posted', amount: { $gte: 100 } },
  aggregate: { total: { fn: 'sum', field: 'amount', alias: '$record' } },
  groupBy: ['$record.category']
})
// Parallel: db.properties.find({ same filter })
```

For deeper architectural exposition see the blog article on [LMPG](https://rushdb.com/blog/labeled-meta-property-graphs-rushdb-s-revolutionary-approach-to-graph-database-architecture).

## 🚀 Quick Start

### 1. Get an API Key (Cloud) or Run Locally
RushDB Cloud: create a project at https://app.rushdb.com → copy API key.
Self-host (requires Neo4j 5.25.1+ with APOC & GDS):
```bash
docker run -p 3000:3000 \
  --name rushdb \
  -e NEO4J_URL='neo4j+s://your-instance.neo4j.io' \
  -e NEO4J_USERNAME='neo4j' \
  -e NEO4J_PASSWORD='password' \
  rushdb/platform
```

### 2. Ingest Nested Data

#### Python
```python
from rushdb import RushDB

db = RushDB("RUSHDB_API_KEY")

# Push any nested JSON - RushDB normalizes it into a graph
db.records.create_many(
   "COMPANY",
   {
      "name": "Google LLC",
      "rating": 4.9,
      "DEPARTMENT": [
         {
            "name": "Research & Development",
            "PROJECT": [
               {
                  "name": "Bard AI",
                  "budget": 1200000000,
                  "EMPLOYEE": [
                     {
                        "name": "Jeff Dean",
                        "position": "Head of AI Research",
                     }
                  ],
               }
            ],
         }
      ],
   },
)

# Traverse relationships with intuitive nested queries
employees = db.records.find({
   "labels": ["EMPLOYEE"],
   "where": {
      "position": {"$contains": "AI"},
      "PROJECT": {"DEPARTMENT": {"COMPANY": {"rating": {"$gte": 4}}}},
   },
})
```

#### TypeScript/JavaScript
```typescript
import RushDB from '@rushdb/javascript-sdk';

const db = new RushDB("RUSHDB_API_KEY");

// Push data with automatic relationship creation
await db.records.importJson({
   label: "COMPANY",
   payload: {
      name: 'Google LLC',
      rating: 4.9,
      DEPARTMENT: [{
         name: 'Research & Development',
         PROJECT: [{
            name: 'Bard AI',
            EMPLOYEE: [{
               name: 'Jeff Dean',
               position: 'Head of AI Research',
            }]
         }]
      }]
   }
});

// Simple queries that traverse complex relationships
const aiExperts = await db.records.find({
  labels: ['EMPLOYEE'],
  where: {
    position: { $contains: 'AI' },
    PROJECT: { DEPARTMENT: { COMPANY: { rating: { $gte: 4 } } } },
  },
});
```

## 💡 Unified Search DTO

One structure for all perspectives & operations:

```typescript
interface SearchQuery {
  labels?: string[];     // Filter by record labels
  where?: WhereClause;   // Filter by properties and relationships
  limit?: number;        // Maximum records to return
  skip?: number;         // Records to skip (pagination)
  orderBy?: OrderByClause; // Sorting configuration
  aggregate?: AggregateClause; // Data aggregation
}
```

Benefits:
- Learn once—identical across SDKs & REST
- Automatic schema / property / relationship discovery
- AI & tooling friendliness (introspect before generating follow-up queries)
- Operational + analytical reuse (no ETL / view duplication)

## 🛠️ Self-Hosting

### Requirements
- Neo4j 5.25.1+
- APOC plugin
- Graph Data Science plugin (for vector similarity & advanced aggregates)

### Minimal Docker Compose
```yaml
version: '3.8'
services:
  rushdb:
    image: rushdb/platform
    container_name: rushdb
    ports:
      - "3000:3000"
    environment:
      - NEO4J_URL=neo4j+s://your-instance.neo4j.io
      - NEO4J_USERNAME=neo4j
      - NEO4J_PASSWORD=password
      - RUSHDB_LOGIN=admin
      - RUSHDB_PASSWORD=secure-password
```

<details>
  <summary>Environment variables</summary>

| Name | Description | Required | Default |
|------|-------------|----------|---------|
| `NEO4J_URL` | Neo4j connection URL | yes | - |
| `NEO4J_USERNAME` | Neo4j username | yes | neo4j |
| `NEO4J_PASSWORD` | Neo4j password | yes | - |
| `RUSHDB_PORT` | HTTP port | no | 3000 |
| `RUSHDB_AES_256_ENCRYPTION_KEY` | 32 char encryption key for API tokens | yes (prod) | - |
| `RUSHDB_LOGIN` | Admin login | no | admin |
| `RUSHDB_PASSWORD` | Admin password | no | password |
</details>

<details>
  <summary>Local development (bundled Neo4j)</summary>

```yaml
version: '3.8'
services:
  rushdb:
    image: rushdb/platform
    container_name: rushdb
    depends_on:
      neo4j:
        condition: service_healthy
    ports:
      - "3000:3000"
    environment:
      - NEO4J_URL=bolt://neo4j
      - NEO4J_USERNAME=neo4j
      - NEO4J_PASSWORD=password
  neo4j:
    image: neo4j:5.25.1
    healthcheck:
      test: [ "CMD-SHELL", "wget --no-verbose --tries=1 --spider localhost:7474 || exit 1" ]
      interval: 5s
      retries: 30
      start_period: 10s
    ports:
      - "7474:7474"
      - "7687:7687"
    environment:
      - NEO4J_ACCEPT_LICENSE_AGREEMENT=yes
      - NEO4J_AUTH=neo4j/password
      - NEO4J_PLUGINS=["apoc", "graph-data-science"]
```
</details>

### CLI (container exec)
```bash
# Create user
rushdb create-user admin@example.com securepassword123
# Update password
rushdb update-password admin@example.com newsecurepassword456
```

## 📚 Key Docs

| Topic | Link |
|-------|------|
| Quick Tutorial | https://docs.rushdb.com/get-started/quick-tutorial |
| Grouping & Aggregations | https://docs.rushdb.com/concepts/search/group-by |
| Where / Filtering | https://docs.rushdb.com/concepts/search/where |
| Vector Search | https://docs.rushdb.com/concepts/search/where#vector-operators |
| Python SDK | https://docs.rushdb.com/python-sdk/introduction |
| TypeScript SDK | https://docs.rushdb.com/typescript-sdk/introduction |
| REST API | https://docs.rushdb.com/rest-api/introduction |

## 🤝 Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Issues & PRs welcome.


---

## 📄 License

| Path | License |
|------|---------|
| platform/core | Elastic License 2.0 |
| platform/dashboard | Elastic License 2.0 |
| docs | Apache 2.0 |
| website | Apache 2.0 |
| packages/javascript-sdk | Apache 2.0 |
| packages/mcp-server | Apache 2.0 |

---

---
Need something not supported yet? Open an issue—design discussions are welcome.
