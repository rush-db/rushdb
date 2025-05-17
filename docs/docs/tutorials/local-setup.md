---
sidebar_position: 4
---

# Local Setup

This guide will help you set up a local development environment for RushDB using Docker, without needing to clone the repository. This is ideal for developers who want to work with RushDB in a containerized environment.

## Prerequisites

Before starting, ensure you have the following installed:

1. **Docker Engine**:
   - For macOS: [Docker Desktop for Mac](https://docs.docker.com/desktop/install/mac-install/)
   - For Windows: [Docker Desktop for Windows](https://docs.docker.com/desktop/install/windows-install/)
   - For Linux: [Docker Engine](https://docs.docker.com/engine/install/)

2. **Docker Compose** (included with Docker Desktop, but may need to be installed separately on Linux)

3. **Recommended System Resources**:
   - Minimum: 2GB RAM, 1 CPU
   - Recommended: 4GB RAM, 2 CPUs

## Option 1: Quick Setup with External Neo4j Instance

If you already have a Neo4j instance running (either locally or in the cloud), you can quickly start RushDB connected to it using Docker.

### Using Docker Run Command

```bash
docker run -p 3000:3000 \
--name rushdb \
-e NEO4J_URL='neo4j+s://your-neo4j-instance.databases.neo4j.io' \
-e NEO4J_USERNAME='neo4j' \
-e NEO4J_PASSWORD='your-password' \
rushdb/platform
```

### Using Docker Compose

Create a `docker-compose.yml` file with the following content:

```yaml
version: '3.8'
services:
  rushdb:
    image: rushdb/platform
    container_name: rushdb
    ports:
      - "3000:3000"
    environment:
      - NEO4J_URL=neo4j+s://your-neo4j-instance.databases.neo4j.io
      - NEO4J_USERNAME=neo4j
      - NEO4J_PASSWORD=your-password
```

Then run:

```bash
docker-compose up -d
```

## Option 2: Complete Development Environment with Neo4j

For a fully self-contained development environment with both RushDB and Neo4j:

### Create a Development Docker Compose Setup

1. Create a `docker-compose.yml` file with the following content:

<details>
<summary>docker-compose.yml</summary>

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
    volumes:
      - neo4j-plugins:/var/lib/neo4j/plugins
      - neo4j-data:/data
      - neo4j-logs:/logs
      - neo4j-conf:/var/lib/neo4j/conf

volumes:
  neo4j-plugins:
  neo4j-data:
  neo4j-logs:
  neo4j-conf:
```
</details>

2. Start the environment:

```bash
docker-compose up -d
```

3. Verify both services are running:

```bash
docker-compose ps
```

## Accessing the Development Environment

Once your containers are running:

1. **RushDB Dashboard**: Access at `http://localhost:3000`
2. **Neo4j Browser**: Access at `http://localhost:7474` (if using the local Neo4j setup)

### Default Credentials

- **RushDB Dashboard**:
  - Username: `admin`
  - Password: `password`

- **Neo4j Browser** (if using local Neo4j):
  - Username: `neo4j`
  - Password: `password`

## Advanced Development Workflow

### 1. Exposing Additional Ports

If you need to expose additional ports for development:

```yaml
services:
  rushdb:
    # ...existing configuration...
    ports:
      - "3000:3000"
      - "9229:9229"  # For Node.js debugging
```

### 2. Using Local Volume Mounts

For more extensive development, you might want to mount local files into the container:

```yaml
services:
  rushdb:
    # ...existing configuration...
    volumes:
      - ./your-local-code:/app/platform/core/src
```

### 3. Persistent Data Storage

The default configuration includes volume mounts for Neo4j data persistence. Your data will survive container restarts.

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

## Working with the RushDB CLI

The RushDB Docker image includes a command-line interface (CLI) that you can access from the running container.

### **CLI Commands**

The RushDB CLI allows you to manage users in self-hosted installations. Below are the available commands:

#### **Create a New User**

Command:
```bash
docker exec rushdb rushdb create-user <login> <password>
```

Example:
```bash
docker exec rushdb rushdb create-user admin@example.com securepassword123
```

This command creates a new user with the specified login and password. It is only allowed in self-hosted setups.

#### **Update User Password**

Command:
```bash
docker exec rushdb rushdb update-password <login> <newPassword>
```

Example:
```bash
docker exec rushdb rushdb update-password admin@example.com newsecurepassword456
```

This command updates the password for an existing user identified by the provided login. Like `create-user`, this command is restricted to self-hosted environments.

## Troubleshooting Common Issues

### 1. Connection Issues to Neo4j

If RushDB cannot connect to Neo4j:

- Verify Neo4j is running: `docker ps | grep neo4j`
- Check Neo4j logs: `docker logs neo4j`
- Ensure credentials match in your environment variables
- If using the local Neo4j setup, ensure the hostname `neo4j` resolves to the Neo4j container

### 2. RushDB Container Fails to Start

If the RushDB container exits unexpectedly:

- Check logs with: `docker logs rushdb`
- Verify all required environment variables are set correctly
- Ensure Neo4j is fully initialized before RushDB attempts to connect

### 3. Memory Issues

If containers are being killed due to memory constraints:

- Increase Docker's memory allocation in Docker Desktop settings
- Consider reducing memory usage in Neo4j configuration
- Use the `--memory` flag with `docker run` or set memory limits in `docker-compose.yml`

## Next Steps

After successfully setting up your local development environment:

1. Explore the RushDB Dashboard at `http://localhost:3000`
2. Create your first project and database
3. Generate API tokens for your applications
4. Explore the API documentation available in the dashboard
5. Connect your applications to RushDB using the available SDKs or REST API

