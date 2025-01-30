---
sidebar_position: 4
---

# RushDB Platform Setup

You can quickly launch the **RushDB Platform** using the following Docker command:

```bash
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

### Development Setup with local Neo4j

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

