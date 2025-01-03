<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

RushDB is an open-source alternative to Firebase, built on top of Neo4j.

It streamlines application development by automating data normalization, managing relationships, inferring data types automatically, and offering a suite of additional powerful features to accelerate your workflow.

[🌐 Homepage](https://rushdb.com) — [📢 Blog](https://rushdb.com/blog) — [☁️ Platform ](https://app.rushdb.com) — [📖 Docs](https://docs.rushdb.com) — [🧑‍💻 Examples](https://github.com/rush-db/rushdb/examples)
</div>


## 🚀 Feature Highlights

### 1. **No Data Modeling Required**
Forget about manual data modeling! Simply push data of any shape and complexity—RushDB takes care of everything, including relationships, data types, and more.

---

### 2. **Automated Type Inference**
RushDB automatically infers data types, minimizing overhead while optimizing for speed and high-performance searches.

---

### 3. **Powerful Search API**
Built on top of Cypher, the search API allows you to query data with pinpoint accuracy. No need to wrestle with data—just ask for it with sniper-like precision.

---

### 4. **Flexible Data Import**
RushDB supports multiple data formats out of the box, including **JSON**, **CSV**, and **JSONB**. This flexibility empowers you to import data from virtually any source, enabling the creation of data-rich applications in hours instead of days.

---

### 5. **Developer Experience at Its Core**
Developer experience (DX) is in RushDB's DNA. Designed with elegance and simplicity in mind, its API is intuitive and consistent, reflecting the fractal nature of its underlying architecture.

RushDB is built by developers, for developers—making your experience seamless and delightful. 🌟

## Setup

### Managed Environment

The easiest way to start using RushDB is through **RushDB Cloud**. Free Tier is available.

Get up and running in minutes by signing up at [app.rushdb.com](https://app.rushdb.com). RushDB Cloud provides a fully managed environment, so you can focus on building your application without worrying about setup or infrastructure.

---

### Self-Hosted Environment

If you prefer to manage your own infrastructure, you can set up RushDB with a Neo4j instance. Here’s how:

1. **Use Neo4j Aura (Free Tier Available)**  
   Quickly create a Neo4j instance using [Neo4j Aura](https://neo4j.com/cloud/aura). It’s a managed service that allows you to get started with no configuration hassle.

2. **Deploy Your Own Instance**  
   Alternatively, you can host your own Neo4j instance. Follow [this detailed guide](https://medium.com/@1pxone/deploying-neo4j-on-aws-ec2-instance-apoc-plugin-installation-884deaeb4765) to deploy Neo4j on AWS EC2, including steps for installing the APOC plugin.

Both options allow you to connect RushDB to your Neo4j database for a fully customizable self-hosted environment.

#### Requirements
- **Minimum Neo4j Version**: `5.25.1`
- **Required Plugin**: `apoc-core` (installed and enabled)

Make sure your setup meets these requirements for optimal functionality.

### Running the RushDB Platform

You can quickly launch the **RushDB Platform** using the following Docker command:

```shell
docker run -p 3000:3000 \
-e NEO4J_URL='neo4j+s://1234567.databases.neo4j.io' \
-e NEO4J_USERNAME='neo4j' \
-e NEO4J_PASSWORD='password' \
rushdb/platform
```

### Environment Variables

Before running the container, ensure you provide the following required environment variables:

- **`NEO4J_URL`**: The connection string for your Neo4j database (e.g., `neo4j+s://<your-instance-id>.databases.neo4j.io`).
- **`NEO4J_USERNAME`**: The username for accessing the Neo4j database (default is `neo4j`).
- **`NEO4J_PASSWORD`**: The password for your Neo4j database instance.

### Additional Environment Variables

#### 1. `RUSHDB_PORT`
- **Description**: The port on which the application server will listen for incoming requests.
- **Default**: `3000`

#### 2. `RUSHDB_AES_256_ENCRYPTION_KEY`
- **Description**: The encryption key for securing API tokens using AES-256 encryption.
- **Requirement**: Must be exactly 32 characters long to meet the 256-bit key length requirement.
- **Important**: Change this to a secure value in production.
- **Default**: `32SymbolStringForTokenEncryption`


#### 3. `RUSHDB_LOGIN`
- **Description**: The login username for the RushDB admin account.
- **Important**: Change this to a secure value in production.
- **Default**: `admin`

#### 4. `RUSHDB_PASSWORD`
- **Description**: The password for the RushDB admin account.
- **Important**: Change this to a secure value in production.
- **Default**: `password`


### Usage with TypeScript / JavaScript

1. **Obtain an API Token**:
   - If you’re using **RushDB Cloud**, get your token from [app.rushdb.com](https://app.rushdb.com).
   - For a self-hosted RushDB instance, retrieve the token from the **Dashboard** running locally (`localhost:3000`).

2. **Build Anything**:  
   Easily push, search, and manage relationships within your data.

---

#### Install the SDK

**NPM:**
```bash
npm install @rushdb/javascript-sdk
```

---

### TL;DR

```typescript
import RushDB from '@rushdb/javascript-sdk';

// Setup SDK
const db = new RushDB("API_TOKEN", {
  // Default URL; only override if necessary.
  url: "https://api.rushdb.com",
});

// Push data: RushDB flattens it into Records and establishes relationships automatically.
await db.records.createMany("COMPANY", {
  name: 'Google LLC',
  address: '1600 Amphitheatre Parkway, Mountain View, CA 94043, USA',
  foundedAt: '1998-09-04T00:00:00.000Z',
  rating: 4.9,
  DEPARTMENT: [{
    name: 'Research & Development',
    description: 'Innovating and creating advanced technologies for AI, cloud computing, and consumer devices.',
    PROJECT: [{
      name: 'Bard AI',
      description: 'A state-of-the-art generative AI model for natural language understanding and creation.',
      active: true,
      budget: 1200000000,
      EMPLOYEE: [{
        name: 'Jeff Dean',
        position: 'Head of AI Research',
        email: 'jeff@google.com',
        dob: '1968-07-16T00:00:00.000Z',
        salary: 3000000,
      }]
    }]
  }]
});

// Find Records by specific criteria
const matchedEmployees = await db.records.find({
  labels: ['EMPLOYEE'],
  where: {
    position: { $contains: 'AI' },
    PROJECT: {
      DEPARTMENT: {
        COMPANY: {
          rating: { $gte: 4 },
        },
      },
    },
  },
});

const company = await db.records.findUniq('COMPANY', {
  where: {
    name: 'Google LLC',
  },
});

// Create relationships between the `COMPANY` Record and matched employees
await company.attach(matchedEmployees, { type: "WORKING_AT" });
```


### Usage with REST API and cURL

1. **Obtain an API Token**:
   - If you’re using **RushDB Cloud**, get your token from [app.rushdb.com](https://app.rushdb.com).
   - For a self-hosted RushDB instance, retrieve the token from the **Dashboard** running locally (`localhost:3000`).

2. **Build Anything**:  
   Use the REST API to push, search, and manage relationships in your data via simple HTTP requests.

---

#### API Base URL

- **RushDB Cloud**: `https://api.rushdb.com`
- **Self-Hosted**: Your custom URL (e.g., `http://localhost:3000`)

---

### TL;DR

#### Push Data

```bash
curl -X POST https://api.rushdb.com/api/v1/records/import/json \
-H "Authorization: Bearer API_TOKEN" \
-H "Content-Type: application/json" \
-d '{
  "label": "COMPANY",
  "payload": {
    "name": "Google LLC",
    "address": "1600 Amphitheatre Parkway, Mountain View, CA 94043, USA",
    "foundedAt": "1998-09-04T00:00:00.000Z",
    "rating": 4.9,
    "DEPARTMENT": [{
      "name": "Research & Development",
      "description": "Innovating and creating advanced technologies for AI, cloud computing, and consumer devices.",
      "PROJECT": [{
        "name": "Bard AI",
        "description": "A state-of-the-art generative AI model for natural language understanding and creation.",
        "active": true,
        "budget": 1200000000,
        "EMPLOYEE": [{
          "name": "Jeff Dean",
          "position": "Head of AI Research",
          "email": "jeff@google.com",
          "dob": "1968-07-16T00:00:00.000Z",
          "salary": 3000000
        }]
      }]
    }]
  }
}'
```

---

#### Find Records by Specific Criteria

```bash
curl -X POST https://api.rushdb.com/api/v1/records/search \
-H "Authorization: Bearer API_TOKEN" \
-H "Content-Type: application/json" \
-d '{
  "labels": ["EMPLOYEE"],
  "where": {
    "position": { "$contains": "AI" },
    "PROJECT": {
      "DEPARTMENT": {
        "COMPANY": {
          "rating": { "$gte": 4 }
        }
      }
    }
  }
}'
```

<div align="center">
<b>You're Awesome!</b>  🚀
</div>

---

<div align="center" style="margin-top: 20px">

> Check the [Documentation](https://docs.rushdb.com) and [Examples](https://github.com/rush-db/rushdb/examples) to learn more 🤓


</div>

