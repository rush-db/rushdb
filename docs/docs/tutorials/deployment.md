---
title: Deployment
description: Learn how to deploy RushDB
sidebar_position: 5
---

# Deployment Guide

This guide provides comprehensive instructions for deploying RushDB in various environments. Choose the deployment option that best suits your needs.

## Deployment Options

RushDB offers two primary deployment options:

1. **RushDB Cloud (Managed Service)** - The simplest option with zero setup
2. **Self-Hosted RushDB** - Full control over your infrastructure with multiple deployment methods

## Option 1: RushDB Cloud (Managed Service)

The easiest way to start using RushDB is through the managed cloud service.

### Features
- Zero setup required
- Free tier available
- Fully managed infrastructure
- Automatic updates and maintenance
- Professional support

### Getting Started with RushDB Cloud
1. Sign up at [app.rushdb.com](https://app.rushdb.com)
2. Create a new project
3. Get your API token from the dashboard
4. Start using RushDB APIs via SDKs or REST

## Option 2: Self-Hosted RushDB

Self-hosting gives you complete control over your RushDB deployment and data.

### Prerequisites

Before deploying RushDB, ensure you have:

1. **Neo4j Instance**:
   - Minimum version: `5.25.1`
   - Required plugins:
     - `apoc-core` (installed and enabled)
     - `graph-data-science` (required for vector search capabilities)
   - Can be self-hosted or using Neo4j Aura cloud service

2. **For Docker Deployment**:
   - Docker Engine 20.10.0+
   - Docker Compose 2.0.0+ (if using Docker Compose)
   - Minimum 2GB RAM for the container

3. **For AWS Deployment**:
   - AWS account with necessary permissions
   - Terraform 1.0.0+ installed locally

### Option 2A: Docker Container Deployment

The simplest way to self-host RushDB is using Docker.

#### Basic Docker Run Command

```bash
docker run -p 3000:3000 \
--name rushdb \
-e NEO4J_URL='neo4j+s://your-neo4j-instance.databases.neo4j.io' \
-e NEO4J_USERNAME='neo4j' \
-e NEO4J_PASSWORD='your-password' \
rushdb/platform
```

#### Docker Compose Deployment

Create a `docker-compose.yml` file:

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
      # Add additional environment variables as needed
```

Then run:

```bash
docker-compose up -d
```

#### All-in-One Docker Compose Deployment (with Neo4j)

For development or testing environments, you can run both RushDB and Neo4j together:
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
      # Add additional environment variables as needed
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

### Option 2B: AWS Deployment with Terraform

For production-grade deployments, RushDB can be deployed to AWS using Terraform.

#### Terraform Deployment Steps

1. **Prepare Your Environment**

   Clone the RushDB repository or create a new directory for your Terraform configuration.

2. **Create Terraform Configuration File**

   Create a `main.tf` file with the following content (adjust as needed):

<details>
<summary>rushdb-terraform.tf</summary>

```hcl
terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 4.0"
    }
  }
}

# Configure AWS provider
provider "aws" {
  region = "us-east-1"  # Change to your preferred region
}

# Use default VPC and subnets
data "aws_vpc" "default" {
  default = true
}

data "aws_subnets" "all" {
  filter {
    name   = "vpc-id"
    values = [data.aws_vpc.default.id]
  }
}

# Security group for RushDB
resource "aws_security_group" "rushdb_sg" {
  name        = "rushdb-security-group"
  description = "Allow traffic for RushDB"
  vpc_id      = data.aws_vpc.default.id

  ingress {
    from_port   = 3000
    to_port     = 3000
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

# ECS cluster
resource "aws_ecs_cluster" "rushdb_cluster" {
  name = "rushdb-cluster"
}

# Task execution role
resource "aws_iam_role" "ecs_task_execution_role" {
  name = "rushdb-ecs-task-execution-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "ecs-tasks.amazonaws.com"
        }
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "ecs_task_execution_policy" {
  role       = aws_iam_role.ecs_task_execution_role.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy"
}

# ECS task definition
resource "aws_ecs_task_definition" "rushdb_task" {
  family                   = "rushdb"
  network_mode             = "awsvpc"
  requires_compatibilities = ["FARGATE"]
  cpu                      = "1024"
  memory                   = "2048"
  execution_role_arn       = aws_iam_role.ecs_task_execution_role.arn

  container_definitions = jsonencode([{
    name      = "rushdb"
    image     = "rushdb/platform:latest"
    essential = true

    environment = [
      { name = "NEO4J_URL", value = "neo4j+s://your-neo4j-instance.databases.neo4j.io" },
      { name = "NEO4J_USERNAME", value = "neo4j" },
      { name = "NEO4J_PASSWORD", value = "your-password" },
      { name = "RUSHDB_SELF_HOSTED", value = "true" },
      { name = "RUSHDB_AES_256_ENCRYPTION_KEY", value = "your-32-character-encryption-key" }
    ]

    portMappings = [{
      containerPort = 3000
      hostPort      = 3000
      protocol      = "tcp"
    }]

    logConfiguration = {
      logDriver = "awslogs"
      options = {
        "awslogs-group"         = "/ecs/rushdb"
        "awslogs-region"        = "us-east-1"
        "awslogs-stream-prefix" = "ecs"
        "awslogs-create-group"  = "true"
      }
    }
  }])
}

# ECS service
resource "aws_ecs_service" "rushdb_service" {
  name            = "rushdb-service"
  cluster         = aws_ecs_cluster.rushdb_cluster.id
  task_definition = aws_ecs_task_definition.rushdb_task.arn
  desired_count   = 1
  launch_type     = "FARGATE"

  network_configuration {
    subnets          = data.aws_subnets.all.ids
    security_groups  = [aws_security_group.rushdb_sg.id]
    assign_public_ip = true
  }
}

# Output the service URL
output "rushdb_url" {
  value = "http://${aws_ecs_service.rushdb_service.network_configuration[0].assign_public_ip ? "Public IP" : "Private IP"}:3000"
  description = "URL to access RushDB service"
}
```

</details>

3. **Initialize Terraform**

```bash
terraform init
```

4. **Plan Deployment**

```bash
terraform plan -out=tfplan
```

5. **Apply the Configuration**

```bash
terraform apply tfplan
```

6. **Access Your RushDB Service**

After deployment completes, Terraform will output the URL to access your RushDB service.

#### Advanced AWS Deployment with Load Balancer and SSL

For a production deployment with a load balancer and SSL:

1. Modify the Terraform configuration to include an Application Load Balancer
2. Add Route53 DNS records
3. Configure SSL certificates using ACM

For a complete example with these features, refer to the `main.tf` in the RushDB repository.

## Environment Variables

The following environment variables can be used to configure your RushDB deployment:

### Required Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `NEO4J_URL` | Connection string for Neo4j database | `neo4j+s://your-instance.databases.neo4j.io` or `bolt://localhost:7687` |
| `NEO4J_USERNAME` | Username for Neo4j database | `neo4j` |
| `NEO4J_PASSWORD` | Password for Neo4j database | `your-password` |

### Core Application Settings

| Variable | Description                                  | Default | Required |
|----------|----------------------------------------------|---------|----------|
| `RUSHDB_PORT` | Port for the application server              | `3000` | No |
| `RUSHDB_AES_256_ENCRYPTION_KEY` | 32-character key for token encryption        | `32SymbolStringForTokenEncryption` | Yes, for production |
| `RUSHDB_DASHBOARD_URL` | URL for dashboard access                     | `/` | No |
| `RUSHDB_SELF_HOSTED` | Whether running in self-hosted mode          | `true` | No |
| `RUSHDB_SERVE_STATIC` | Whether to serve static files (Dashboard UI) | `true` | No |

### Authentication Settings

| Variable | Description | Default | Required |
|----------|-------------|---------|----------|
| `RUSHDB_LOGIN` | Admin username | `admin` | No |
| `RUSHDB_PASSWORD` | Admin password | `password` | Yes, for production |
| `RUSHDB_ALLOWED_LOGINS` | List of allowed login usernames | `[]` (all allowed) | No |

### Rate Limiting

| Variable | Description | Default | Required |
|----------|-------------|---------|----------|
| `RATE_LIMITER_REQUESTS_LIMIT` | Max requests within time frame | `10` | No |
| `RATE_LIMITER_TTL` | Time frame for rate limiting (ms) | `1000` | No |

### OAuth and Authentication

| Variable | Description | Required |
|----------|-------------|----------|
| `GOOGLE_CLIENT_ID` | Google OAuth client ID | For Google auth |
| `GOOGLE_SECRET` | Google OAuth secret | For Google auth |
| `GH_CLIENT_ID` | GitHub OAuth client ID | For GitHub auth |
| `GH_SECRET` | GitHub OAuth secret | For GitHub auth |
| `SERVICE_CAPTCHA_KEY` | CAPTCHA service private key | For CAPTCHA |

### Email Configuration

| Variable | Description | Required |
|----------|-------------|----------|
| `MAIL_HOST` | Email service host | For email |
| `MAIL_USER` | Email service username | For email |
| `MAIL_PASSWORD` | Email service password | For email |
| `MAIL_FROM` | Default "from" email address | For email |

## CLI Commands

RushDB provides CLI commands for managing users in self-hosted installations:

### Create a New User

```bash
rushdb create-user <login> <password>
```

Example:
```bash
rushdb create-user admin@example.com securepassword123
```

### Update User Password

```bash
rushdb update-password <login> <newPassword>
```

Example:
```bash
rushdb update-password admin@example.com newsecurepassword456
```

## Security Best Practices

When deploying RushDB to production, follow these security best practices:

1. **Change default credentials**:
   - Change `RUSHDB_LOGIN` and `RUSHDB_PASSWORD`
   - Use a strong, unique `RUSHDB_AES_256_ENCRYPTION_KEY`

2. **Secure your Neo4j database**:
   - Use strong passwords
   - Limit network access to the database
   - Use encrypted connections where possible

3. **Use HTTPS**:
   - Configure SSL/TLS on your load balancer
   - Redirect HTTP to HTTPS

4. **Set up proper monitoring and logging**:
   - Monitor API usage
   - Set up alerts for unusual activity

## System Requirements

### Minimum Specifications

- **CPU**: 1 vCPU (2+ recommended for production)
- **Memory**: 1GB RAM (2GB+ recommended for production)
- **Storage**: 1GB for RushDB (excluding Neo4j storage requirements)
- **Neo4j Requirements**: Refer to [Neo4j system requirements](https://neo4j.com/docs/operations-manual/current/installation/requirements/)

### Recommended Production Specifications

- **CPU**: 2+ vCPUs
- **Memory**: 4GB+ RAM
- **Storage**: SSD storage for both RushDB and Neo4j
- **Network**: Low-latency connection between RushDB and Neo4j

## Troubleshooting

### Common Issues

1. **Connection Issues to Neo4j**:
   - Ensure Neo4j instance is running and accessible
   - Verify credentials and connection string format
   - Check network connectivity and firewall settings

2. **Authentication Failures**:
   - Verify admin credentials are correctly set
   - Check encryption key length (must be 32 characters)

3. **Performance Issues**:
   - Monitor resource utilization
   - Consider scaling up resources or optimizing Neo4j queries

### Getting Help

If you encounter problems with your RushDB deployment:

1. Check the RushDB logs for error messages
2. Visit the [RushDB documentation](https://docs.rushdb.com)
3. Submit an issue on the [RushDB GitHub repository](https://github.com/rush-db/rushdb)

## Conclusion

Following this guide, you should have successfully deployed RushDB in your chosen environment. Whether you're using the managed cloud service or self-hosting, RushDB provides a powerful database solution for modern applications.
