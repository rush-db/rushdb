<!-- filepath: /Users/onepx/personal/rushdb/README.md -->
<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# 🚀 RushDB 1.0

### The Instant Graph Database for Modern Apps

RushDB transforms how you work with graph data — no schema required, no complex queries, just push your data and go.

[![GitHub Stars](https://img.shields.io/github/stars/rush-db/rushdb?style=social)](https://github.com/rush-db/rushdb)
[![Follow on Twitter](https://img.shields.io/twitter/follow/rushdb?style=social)](https://twitter.com/rushdb)

[🌐 Website](https://rushdb.com) • [📖 Documentation](https://docs.rushdb.com) • [☁️ Cloud Platform](https://app.rushdb.com) • [🔍 Examples](https://github.com/rush-db/examples)

</div>

---

## ✨ Key Features

- **Instant Setup**: Be productive in seconds, not days
- **Push Any JSON**: Nested objects are automatically normalized into a graph
- **Fractal API**: Same query syntax everywhere - learn once, use everywhere
- **Vector Search**: Comprehensive similarity search for AI-powered applications
- **Zero Schema Headaches**: We handle the data structure so you can focus on building

## 🌟 What's New in 1.0?

- **Vector Search**: Comprehensive vector search with similarity aggregates and query builder support
- **Member Management**: Complete workspace membership system with invitations and access controls
- **Remote Database**: Connect to existing Neo4j/Aura databases
- **Enhanced Auth**: Google OAuth support and improved authorization flows
- **Documentation**: Reworked docs with clear tutorials and guides

## 🚀 Quick Start

### 1. Get RushDB

**Option A: Use RushDB Cloud (Free Tier Available)**
```bash
# Sign up and get an API token at app.rushdb.com
# No installation required!
```

**Option B: Self-Host with Docker**
```bash
docker run -p 3000:3000 \
  --name rushdb \
  -e NEO4J_URL='neo4j+s://your-instance.neo4j.io' \
  -e NEO4J_USERNAME='neo4j' \
  -e NEO4J_PASSWORD='password' \
  rushdb/platform
```

### 2. Start Building

#### Python
```python
from rushdb import RushDB

db = RushDB("your-api-token")

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

const db = new RushDB("your-api-token");

// Push data with automatic relationship creation
await db.records.createMany({
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

## 💡 The Power of RushDB's Fractal API

RushDB uses a consistent query structure across all operations:

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

This approach means:
- **Learn Once, Use Everywhere**: The same pattern works across records, relationships, labels, and properties
- **Predictable API**: No surprises as you build more complex features
- **Self-Discovering Database**: The graph knows its own structure and exposes it consistently
- **Perfect for AI & RAG**: AI agents can explore and generate queries on-the-fly

## 🛠️ Self-Hosting Options

### Requirements
- **Neo4j**: Version `5.25.1` or higher
- **APOC Plugin**: Required and can be installed either via volume mount or auto-install
- **Graph Data Science Plugin**: Required for advanced features like vector search and aggregations

### Docker Compose Setup
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
  <summary>View all environment variables</summary>

  - **`NEO4J_URL`**: Connection string for Neo4j
  - **`NEO4J_USERNAME`**: Neo4j username (default: `neo4j`)
  - **`NEO4J_PASSWORD`**: Neo4j password
  - **`RUSHDB_PORT`**: Server port (default: `3000`)
  - **`RUSHDB_AES_256_ENCRYPTION_KEY`**: Encryption key for API tokens (32 chars)
  - **`RUSHDB_LOGIN`**: Admin username (default: `admin`)
  - **`RUSHDB_PASSWORD`**: Admin password (default: `password`)
</details>

<details>
  <summary>Development setup with local Neo4j</summary>

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

### CLI Commands

<details>
  <summary>View available CLI commands</summary>

#### Create a New User
```bash
rushdb create-user admin@example.com securepassword123
```

#### Update User Password
```bash
rushdb update-password admin@example.com newsecurepassword456
```
</details>

## 📚 Learn More

- [Quick Tutorial](https://docs.rushdb.com/get-started/quick-tutorial): Create a mini social network in minutes
- [Reusable SearchQuery](https://docs.rushdb.com/tutorials/reusable-search-query): Harness the power of RushDB's fractal architecture
- [Python SDK](https://docs.rushdb.com/python-sdk/introduction): Comprehensive Python API documentation
- [TypeScript SDK](https://docs.rushdb.com/typescript-sdk/introduction): Complete TypeScript/JavaScript guide
- [REST API](https://docs.rushdb.com/rest-api/introduction): Full HTTP API reference

## 🤝 Contributing

We welcome contributions! See our [Contributing Guide](CONTRIBUTING.md) for details.

---

<div align="center">
  <p>
    <a href="https://rushdb.com">
      <img src="https://img.shields.io/badge/Learn_more-rushdb.com-6D28D9?style=for-the-badge" alt="Learn more" />
    </a>
  </p>
</div>
