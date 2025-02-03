<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB
### The Instant Database for Modern Apps and DS/ML Ops

RushDB is an instant database for modern apps and DS/ML ops built on top of Neo4j.

It automates data normalization, manages relationships, and infers data types, enabling developers to focus on building features rather than wrestling with data.

[🌐 Homepage](https://rushdb.com) — [📢 Blog](https://rushdb.com/blog) — [☁️ Platform ](https://app.rushdb.com) — [📖 Docs](https://docs.rushdb.com) — [🧑‍💻 Examples](https://github.com/rush-db/examples)
</div>

---

## Setup

### Option 1: Managed Environment

The easiest way to start using RushDB is through **RushDB Cloud**. Free Tier is available.

Get up and running in less than 30 seconds by signing up at [app.rushdb.com](https://app.rushdb.com). RushDB Cloud provides a fully managed environment, so you can focus on building your application without worrying about setup or infrastructure.

### Option 2: Self-Hosted Environment

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

---

#### Running the RushDB Platform

You can quickly launch the **RushDB Platform** using the following Docker command:

```shell
docker run -p 3000:3000 \
--name rushdb \
-e NEO4J_URL='neo4j+s://1234567.databases.neo4j.io' \
-e NEO4J_USERNAME='neo4j' \
-e NEO4J_PASSWORD='password' \
rushdb/platform
```

Or by using Docker Compose:

```yaml
version: '3.8'
services:
  rushdb:
    image: rushdb/platform
    container_name: rushdb
    ports:
      - "3000:3000"
    environment:
      - NEO4J_URL=neo4j+s://1234567.databases.neo4j.io
      - NEO4J_USERNAME=neo4j
      - NEO4J_PASSWORD=password
```

#### Environment Variables

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

---

<details>
  <summary>Development Setup with local Neo4j [DOCKER COMPOSE]</summary>

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
         - NEO4J_PLUGINS=["apoc"]
   ```
</details>


### **CLI Commands**

The RushDB CLI allows you to manage users in self-hosted installations. Below are the available commands:

#### **Create a New User**

Command:
```bash
rushdb create-user <login> <password>
```

Example:
```bash
rushdb create-user admin@example.com securepassword123
```

This command creates a new user with the specified login and password. It is only allowed in self-hosted setups.

#### **Update User Password**

Command:
```bash
rushdb update-password <login> <newPassword>
```

Example:
```bash
rushdb update-password admin@example.com newsecurepassword456
```

This command updates the password for an existing user identified by the provided login. Like `create-user`, this command is restricted to self-hosted environments.

---

## Usage

1. **Obtain an API Token**:
   - If you’re using **RushDB Cloud**, get your token from [app.rushdb.com](https://app.rushdb.com).
   - For a self-hosted RushDB instance, retrieve the token from the **Dashboard** running locally (`localhost:3000`).

2. **Build Anything**:  
   Easily push, search, and manage relationships within your data.

### With Python

Explore the [Documentation](https://docs.rushdb.com/python-sdk/records-api)

#### Install the SDK

```bash
pip install rushdb
```

#### Push any json data

```python
from rushdb import RushDB

db = RushDB(
   "rushdb-api-key",
   # Default URL; only override if necessary.
   base_url="https://api.rushdb.com",
)

db.records.create_many(
   "COMPANY",
   {
      "name": "Google LLC",
      "address": "1600 Amphitheatre Parkway, Mountain View, CA 94043, USA",
      "foundedAt": "1998-09-04T00:00:00.000Z",
      "rating": 4.9,
      "DEPARTMENT": [
         {
            "name": "Research & Development",
            "description": "Innovating and creating advanced technologies for AI, cloud computing, and consumer devices.",
            "tags": ["AI", "Cloud Computing", "Research"],
            "profitable": true,
            "PROJECT": [
               {
                  "name": "Bard AI",
                  "description": "A state-of-the-art generative AI model for natural language understanding and creation.",
                  "active": true,
                  "budget": 1200000000,
                  "EMPLOYEE": [
                     {
                        "name": "Jeff Dean",
                        "position": "Head of AI Research",
                        "email": "jeff@google.com",
                        "salary": 3000000,
                     }
                  ],
               }
            ],
         }
      ],
   },
)
```

#### Find Records by specific criteria
```python
# Find Records by specific criteria
matched_employees = db.records.find(
   {
      "labels": ["EMPLOYEE"],
      "where": {
         "position": {"$contains": "AI"},
         "PROJECT": {"DEPARTMENT": {"COMPANY": {"rating": {"$gte": 4}}}},
      },
   }
)

```
---

### With TypeScript / JavaScript

Explore the [Documentation](https://docs.rushdb.com)

#### Install the SDK

```bash
npm install @rushdb/javascript-sdk
```

#### Push any json data

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
```

#### Find Records by specific criteria
```typescript
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
```
---

### With REST API and cURL

Explore the [Documentation](https://docs.rushdb.com)

#### Specify API base URL

- **RushDB Cloud**: `https://api.rushdb.com`
- **Self-Hosted**: Your custom URL (e.g., `http://localhost:3000`)

####  Push any json data

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

#### Find Records by specific criteria

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

---

<div align="center" style="margin-top: 20px">

> Check the [Documentation](https://docs.rushdb.com) and [Examples](https://github.com/rush-db/examples) to learn more 🤓

</div>

