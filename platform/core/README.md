<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB Core
### The Instant Database for Modern Apps and DS/ML Ops

RushDB is an open-source alternative to Firebase, built on top of Neo4j.

It streamlines application development by automating data normalization, managing relationships, inferring data types automatically, and offering a suite of additional powerful features to accelerate your workflow.

[🌐 Homepage](https://rushdb.com) — [📢 Blog](https://rushdb.com/blog) — [☁️ Platform ](https://app.rushdb.com) — [📖 Docs](https://docs.rushdb.com) — [🧑‍💻 Examples](https://github.com/rush-db/examples)
</div>

--- 

## Prerequisites

Before running the application, ensure that you have the following installed:

1. **PNPM**: Install PNPM globally by running:
   ```bash
   npm install -g pnpm@10.1.0
   ```
2. **Docker**: The application requires a running Docker instance to start the Neo4j database. Make sure Docker is installed and running on your machine.

## Running the Application Locally

To run the application locally, execute the following command from the root directory:

```bash
pnpm dev
```

This command is defined in the root `package.json` as:

```json
"dev": "cd platform && pnpm dev:platform"
```

### What Happens When You Run `pnpm dev`

1. **Neo4j Instance (Docker Container)**: The command starts a Docker container with a Neo4j database instance required by the application.

2. **RushDB Dashboard**: The dashboard, located in `/platform/dashboard`, is launched and serves the management interface for the RushDB platform.

3. **NestJS Core App**: The core application, located in `/platform/core`, is started. This is the main backend service handling API requests.

## Configuration Settings

You can customize the application by setting environment variables. Below are the available configurations:

### Application Server
- `RUSHDB_PORT=3000`
  - The port on which the application server will listen for incoming requests.

### Security & Encryption
- `RUSHDB_AES_256_ENCRYPTION_KEY=32SymbolStringForTokenEncryption`
  - The encryption key for securing API tokens using AES-256 encryption. It must be exactly 32 characters long.

### Dashboard
- `RUSHDB_DASHBOARD_URL=http://localhost:3005`
  - The URL of the RushDB dashboard.

### Static Files
- `RUSHDB_SERVE_STATIC=false`
  - A flag to enable or disable serving static files. Set to `true` to serve static assets.

### Hosting Mode
- `RUSHDB_SELF_HOSTED=true`
  - Indicates whether RushDB is running in self-hosted mode (`true`) or a managed environment (`false`).

### Admin Credentials
> **Note:** These credentials are applicable and a default user is created only when `RUSHDB_SELF_HOSTED=true`.

- `RUSHDB_LOGIN=admin`
  - The login username for the RushDB admin account.
- `RUSHDB_PASSWORD=password`
  - The password for the RushDB admin account.

### Allowed Logins
- `RUSHDB_ALLOWED_LOGINS=[]`
  - A find of allowed login usernames. Leave empty to allow all logins.

### Rate Limiting
- `RATE_LIMITER_REQUESTS_LIMIT=10`
  - Maximum number of requests allowed within a specific time frame.
- `RATE_LIMITER_TTL=1000`
  - Time-to-live (TTL) for the rate limiter.

### OAuth Configuration
#### Google
- `GOOGLE_CLIENT_ID=`
- `GOOGLE_SECRET=`

#### GitHub
- `GH_CLIENT_ID=`
- `GH_SECRET=`

### CAPTCHA
- `SERVICE_CAPTCHA_KEY=`
  - The private key for the CAPTCHA service.

### Mailer Configuration
- `MAIL_HOST=`
  - The host address for the email service (e.g., `smtp.gmail.com`).
- `MAIL_USER=`
  - The username for authenticating with the email service.
- `MAIL_PASSWORD=`
  - The password for authenticating with the email service.
- `MAIL_FROM=`
  - The default "from" email address for outgoing emails.

### Billing
- `STRIPE_SECRET_KEY=`
  - The secret key for integrating with Stripe for billing.
- `STRIPE_ENDPOINT_SECRET=`
  - The webhook signing secret for validating Stripe events.

### Database Configuration
#### Neo4j
- `NEO4J_URL=bolt://localhost:7687`
  - The URL for connecting to the Neo4j database instance.
- `NEO4J_USERNAME=neo4j`
  - The username for authenticating with Neo4j.
- `NEO4J_PASSWORD=password`
  - The password for authenticating with Neo4j. **Change this to a secure value in production.**

## Production Deployment

### Updating Terraform Configuration
For production deployment, ensure that the Terraform configuration reflects the necessary changes to support the ECS task definition. Update the `task_definition` and other relevant configurations as needed in the Terraform files.

### GitHub Workflow Configuration
Adjust the GitHub Actions workflow to automate the deployment process. Ensure that the workflow includes steps to update the ECS task definition and deploy the updated container to your production environment.

### Syncing Infrastructure with Terraform
To deploy or update the ECS task in ECR, follow these steps:

1. Run the command `terraform init` from the project root directory. This will create the necessary `.terraform` folder.

2. Run `terraform plan -out="tfplan"` to create a plan for the changes, which will also show details of the AWS resources to be created or updated.

3. If you are satisfied with the plan, run `terraform apply "tfplan"` to apply the changes and update the infrastructure.

4. (Optional) To destroy the planned resources, run `terraform plan -destroy -out=tfplan`.

## Additional Notes
- Ensure your environment variables are properly configured in a `.env` file or through your preferred method of environment variable management.
- Refer to the platform's documentation for advanced configuration and troubleshooting.
