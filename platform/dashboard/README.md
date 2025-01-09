<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB Dashboard
### The Instant Database for Modern Apps and DS/ML Ops

RushDB is an open-source alternative to Firebase, built on top of Neo4j.

It streamlines application development by automating data normalization, managing relationships, inferring data types automatically, and offering a suite of additional powerful features to accelerate your workflow.

[🌐 Homepage](https://rushdb.com) — [📢 Blog](https://rushdb.com/blog) — [☁️ Platform ](https://app.rushdb.com) — [📖 Docs](https://docs.rushdb.com) — [🧑‍💻 Examples](https://github.com/rush-db/rushdb/examples)
</div>

The `/platform/dashboard` directory contains the RushDB management interface, built with React and Vite. This README provides instructions for running, building, and configuring the dashboard both as a standalone app and as part of the core RushDB platform.

## Prerequisites

Before running the application, ensure the following are installed on your system:

1. **Node.js**: Recommended version 18 or above.
2. **PNPM**: Install PNPM globally by running:
   ```bash
   npm install -g pnpm
   ```

## Environment Variable

The dashboard uses one environment variable to configure the backend URL:

- `VITE_BACKEND_BASE_URL`:
  - When **bundled within the core** (`RUSHDB_SERVE_STATIC=true`): Leave this variable empty.
  - When running as a **standalone app**: Set this to the URL of the backend core (e.g., `http://localhost:3000`).

Example:
```bash
export VITE_BACKEND_BASE_URL=http://localhost:3000
```

## Running the Application

### Development Mode
To start the dashboard in development mode with hot-reloading:

```bash
pnpm dev
```

This will launch the Vite development server. Open your browser and navigate to the provided URL (typically `http://localhost:5173`).

### Building for Production
To build the dashboard for production:

```bash
pnpm build
```

The production-ready files will be located in the `dist/` directory.

### Previewing the Production Build
To preview the built production files:

```bash
pnpm preview
```

This will serve the contents of the `dist/` directory locally for testing.

## Linting and Type Checking

### Linting
Run the linter to check for code quality issues:

```bash
pnpm lint
```

To automatically fix linting issues:

```bash
pnpm lint:fix
```

### Type Checking
To check TypeScript types without emitting files:

```bash
pnpm types:check
```

## Reinstalling Dependencies

To clean up and reinstall all dependencies:

```bash
pnpm reinstall
```

This removes all `node_modules` directories and reinstalls dependencies, ensuring a clean setup.

## Usage Scenarios

### Bundled with Core
When using the dashboard as part of the core RushDB platform:

1. Set `RUSHDB_SERVE_STATIC=true` in the core environment configuration.
2. Leave `VITE_BACKEND_BASE_URL` empty.

This ensures the dashboard is served statically by the core backend.

### Standalone App
When using the dashboard as a standalone application:

1. Set `VITE_BACKEND_BASE_URL` to the URL of the RushDB core backend.
2. Use the development or production build commands as required.

Example:
```bash
export VITE_BACKEND_BASE_URL=http://localhost:3000
pnpm dev
```

## Additional Notes

- For production deployments, ensure environment variables are configured securely.
- Refer to the RushDB documentation for advanced configuration and troubleshooting.
