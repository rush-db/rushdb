<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB

RushDB is an open-source alternative to Firebase, built on top of Neo4j.

It streamlines application development by automating data normalization, managing relationships, inferring data types automatically, and offering a suite of additional powerful features to accelerate your workflow.

[Homepage](https://rushdb.com) — [Blog](https://rushdb.com/blog) — [Platform](https://app.rushdb.com) — [Docs](https://docs.rushdb.com) — [Examples](https://github.com/rush-db/rushdb/examples)
</div>

## Setup

### Setting Up a Neo4j Instance

You can easily get started with Neo4j in one of two ways:

1. **Use Neo4j Aura (Free Tier)**  
   Quickly spin up a free Neo4j instance on [Neo4j Aura](https://neo4j.com/cloud/aura/), which provides a managed database service with no setup hassle.

2. **Set Up Your Own Instance**  
   Alternatively, you can deploy your own Neo4j instance by following [this guide](https://medium.com/@1pxone/deploying-neo4j-on-aws-ec2-instance-apoc-plugin-installation-884deaeb4765) for detailed instructions on deploying Neo4j on AWS EC2, including the installation of the APOC plugin.

#### Requirements
- **Minimum Neo4j Version**: `5.25.1`
- **Required Plugin**: `apoc-core` (installed and enabled)

Make sure your setup meets these requirements for optimal functionality.

### Running the RushDB Platform with Docker

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
- **Default**: `admin`

#### 4. `RUSHDB_PASSWORD`
- **Description**: The password for the RushDB admin account.
- **Important**: Change this to a secure value in production.

---

### Ports

- The platform will run on port `3000`. You can access it via `http://localhost:3000`.

### Prerequisites

1. A **Neo4j instance** must be up and running. You can use [Neo4j Aura](https://neo4j.com/cloud/aura/) or deploy your own instance.
2. Ensure your Neo4j database meets the following requirements:
  - **Neo4j Version**: `5.25.1` or higher
  - **APOC Plugin**: `apoc-core` must be installed and enabled

### Example Use Case

If you are using a free Neo4j Aura instance:
1. Replace `1234567.databases.neo4j.io` with your Aura instance URL.
2. Replace `password` with your Neo4j Aura password.

### Troubleshooting

- **Connection Issues**: Ensure your Neo4j instance is accessible and the credentials are correct.
- **Port Conflicts**: If port `3000` is already in use, change the mapping to another available port (e.g., `-p 8080:3000`).

Now you're ready to run and explore the RushDB Platform! 🚀

## SDK Installation

---
NPM:
```bash
npm install @rushdb/javascript-sdk
```

YARN:
```bash
yarn add @rushdb/javascript-sdk
```

PNPM:
```bash
pnmp add @rushdb/javascript-sdk
```


## Usage

---

1. **Obtain API Token**: Grab one from the [Dashboard](https://app.rushdb.com).
2. **Build anything**: Easily push, search, and manage relationships within your data.

### TLDR;
```ts
import RushDB, { Model } from '@rushdb/javascript-sdk'

// Setup SDK
const db = new RushDB("API_TOKEN", {
  // This is the default URL; no need to provide it unless overriding.
  url: "https://api.rushdb.com", 
});

// Push any data, and RushDB will automatically flatten it into Records 
// and establish relationships between them accordingly.
await db.records.createMany("COMPANY", {
  name: 'Google LLC',
  address: '1600 Amphitheatre Parkway, Mountain View, CA 94043, USA',
  foundedAt: '1998-09-04T00:00:00.000Z',
  rating: 4.9,
  DEPARTMENT: [{
    name: 'Research & Development',
    description:
        'Innovating and creating advanced technologies for AI, cloud computing, and consumer devices.',
    PROJECT: [{
      name: 'Bard AI',
      description:
          'A state-of-the-art generative AI model designed for natural language understanding and creation.',
      active: true,
      budget: 1200000000,
      EMPLOYEE: [{
        name: 'Jeff Dean',
        position: 'Head of AI Research',
        email: 'jeff@google.com',
        dob: '1968-07-16T00:00:00.000Z',
        salary: 3000000
      }]
    }]
  }]
})


// Find Records by specific criteria
const matchedEmployees = await db.records.find({
  labels: ['EMPLOYEE'],
  where: {
    position: { $contains: 'AI' },
    PROJECT: {
      DEPARTMENT: {
        COMPANY: {
          rating: { $gte: 4 }
        }
      }
    }
  }
})

const company = await db.records.findUniq('COMPANY', {
  where: {
    name: 'Google LLC'
  }
})

// Manage relationships
await company.attach(matchedEmployees, { type: "WORKING_AT" })
```

<div align="center">
<b>You're Awesome!</b>  🚀
</div>

---

<div align="center" style="margin-top: 20px">

> Check the [Docs](https://docs.rushdb.com) and [Examples Repository](https://github.com/collect-so/examples) to learn more 🤓


</div>


## Contributing

---
See [CONTRIBUTING.md](CONTRIBUTING.md).

