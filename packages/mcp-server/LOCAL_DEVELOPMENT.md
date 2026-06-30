# Local Development & Testing with ngrok

This guide explains how to run and test the RushDB MCP server locally, including the OAuth authorization flow end-to-end using ngrok to expose your local servers publicly.

## Prerequisites

- [ngrok](https://ngrok.com/) installed and authenticated (`ngrok config add-authtoken <token>`)
- pnpm installed
- Dependencies installed (`pnpm install` from the repo root)

---

## 1. Start the Platform

From the **repo root**, start the platform (API on port 3000 + dashboard on port 3005):

```bash
pnpm dev
```

This runs `platform/core` (NestJS REST API) and `platform/dashboard` (React UI) concurrently.

Before connecting ChatGPT, make sure `platform/core` has:

```bash
RUSHDB_PUBLIC_URL=http://localhost:3000
```

The value must match the MCP server's `RUSHDB_OAUTH_ISSUER` (for Docker-based MCP runs this is often `http://host.docker.internal:3000`). If these differ, OAuth may link successfully but MCP action discovery fails with `401 Reauthentication required`.

---

## 2. Expose the MCP Server with ngrok

**Start ngrok before the MCP server** — you need the public URL first to set `MCP_RESOURCE_URL`.

Only the MCP server (port 3001) needs to be publicly reachable. ChatGPT's servers call it directly to fetch OAuth metadata and exchange tokens. Everything else (consent page, login) happens in your browser, which can reach `localhost:3005` without a tunnel.

```bash
ngrok http 3001
```

Copy the `https://` URL from the output — you'll need it as `MCP_RESOURCE_URL` in the next step.

---

## 3. Build and Run the MCP Server

First build the MCP server:

```bash
# from repo root
pnpm --filter @rushdb/mcp-server build

# or from packages/mcp-server
pnpm build
```

Then start it in HTTP mode, substituting the ngrok URL you got in the previous step:

```bash
MCP_TRANSPORT=http \
RUSHDB_API_URL=http://localhost:3000/api/v1 \
RUSHDB_OAUTH_ISSUER=http://localhost:3000 \
MCP_RESOURCE_URL=https://<your-mcp-ngrok-subdomain>.ngrok-free.app \
RUSHDB_JWT_PRIVATE_KEY_BASE64=<base64-pem-private-key> \
RUSHDB_JWT_PUBLIC_KEY_BASE64=<base64-pem-public-key> \
RUSHDB_JWT_KID=local-rs256-key-1 \
node packages/mcp-server/build/index.js
```

> **`MCP_RESOURCE_URL`** must be the ngrok URL from step 2. Every time ngrok restarts it issues a new URL (unless you have a paid static domain), so you'll need to restart the MCP server with the updated value.

> Preferred: run OAuth with RS256 keys (`RUSHDB_JWT_PRIVATE_KEY*` + `RUSHDB_JWT_PUBLIC_KEY*`) so AI clients can verify tokens via JWKS.

> Backward-compatible fallback: `RUSHDB_AES_256_ENCRYPTION_KEY` (32 chars) is still supported for local HS256 signing if RS256 keys are not set.

---

## 4. Connect an AI Client

### ChatGPT

1. Open **ChatGPT** → Settings → Connected Apps → Add MCP connector
2. Enter the MCP server URL: `https://<your-mcp-ngrok-subdomain>.ngrok-free.app/mcp`
3. ChatGPT will trigger the OAuth flow:
   - Your browser is redirected to `http://localhost:3005/oauth/consent` — no tunnel needed for the dashboard
   - Log in to RushDB (if prompted)
   - Select the project to grant access to → click **Allow**
4. ChatGPT receives an access token and can now use the 31 RushDB tools.

### MCP Inspector (quick local test, no OAuth)

To test tools directly without an AI client:

```bash
pnpm --filter @rushdb/mcp-server inspector
# or: npx @modelcontextprotocol/inspector packages/mcp-server/build/index.js
```

Set the transport to **HTTP** and point it at `http://localhost:3001/mcp`. The inspector lets you call any tool interactively and inspect request/response payloads.

---

## 5. Verify the OAuth Discovery Endpoints

Before connecting an AI client, confirm the discovery endpoints are reachable:

```bash
# OpenID configuration (needed for RFC 8414 / OAuth 2.0 Authorization Server Metadata)
curl https://<your-mcp-ngrok-subdomain>.ngrok-free.app/.well-known/openid-configuration | jq

# JWKS should be non-empty when RS256 is configured
curl https://<your-mcp-ngrok-subdomain>.ngrok-free.app/.well-known/jwks.json | jq

# MCP probe should return text/event-stream
curl -i https://<your-mcp-ngrok-subdomain>.ngrok-free.app/mcp -H 'Accept: text/event-stream'
```

If ChatGPT shows `OAuth linked, action discovery failed`:

1. Re-check that `RUSHDB_PUBLIC_URL` (platform/core) matches `RUSHDB_OAUTH_ISSUER` (MCP server runtime).
2. Ensure `/.well-known/jwks.json` contains keys (not an empty array).
3. Remove and recreate the ChatGPT connector to force a fresh OAuth token.
4. Restart both platform/core and MCP server after env changes.

---

## 6. Revoke a Connection

Connected applications can be revoked from the **Workspace Settings** page (`/workspace-settings`) in the dashboard under the "Connected Applications" section.

---

## Troubleshooting: stdio (`npx`) server fails with `-32000` inside the monorepo

If you wire the **stdio** server into an MCP client (Claude Code, Cursor, …) using an `.mcp.json` that lives **inside this repo**, the client may fail to connect with `Failed to reconnect to rushdb: -32000`, and launching the command by hand prints:

```
sh: rushdb-mcp: command not found
```

**Cause.** `@rushdb/mcp-server` is a pnpm workspace package (`pnpm-workspace.yaml` → `packages/*`). When an **unversioned** `npx @rushdb/mcp-server` runs with a working directory inside the monorepo, npx resolves it to the local workspace copy instead of the registry, finds its declared `rushdb-mcp` bin, but that bin is not linked in `node_modules/.bin` — so the process exits before the MCP handshake and the client surfaces `-32000`. Adding `-p` does **not** help: it still resolves the workspace package. Outside the repo the bare command downloads `@rushdb/mcp-server` from npm and works as documented.

**Fix.** Pin a **version** so npx fetches from the registry instead of the workspace (works both inside and outside the repo):

```jsonc
{
  "mcpServers": {
    "rushdb": {
      "command": "npx",
      // unversioned ["-y", "@rushdb/mcp-server"] resolves the workspace pkg and fails inside this repo.
      // A version tag forces registry resolution:
      "args": ["-y", "-p", "@rushdb/mcp-server@latest", "rushdb-mcp"],
      "env": { "RUSHDB_API_KEY": "<your-api-key>" }
    }
  }
}
```

Or run your local build directly (uses in-repo code; run `pnpm --filter @rushdb/mcp-server build` first):

```jsonc
{
  "mcpServers": {
    "rushdb": {
      "command": "node",
      "args": ["packages/mcp-server/build/index.js"],
      "env": { "RUSHDB_API_KEY": "<your-api-key>" }
    }
  }
}
```

After editing `.mcp.json`, reconnect from the client (`/mcp` in Claude Code) or restart it. Note that `.mcp.json` is **not** gitignored — never commit a real API key.

---

## Environment Variable Reference

| Variable                                                   | Description                                               | Example                            |
| ---------------------------------------------------------- | --------------------------------------------------------- | ---------------------------------- |
| `MCP_TRANSPORT`                                            | Transport mode. Use `http` for OAuth/remote clients       | `http`                             |
| `RUSHDB_API_URL`                                           | Base URL of the RushDB REST API                           | `http://localhost:3000/api/v1`     |
| `RUSHDB_OAUTH_ISSUER`                                      | OAuth issuer — must match the platform origin             | `http://localhost:3000`            |
| `MCP_RESOURCE_URL`                                         | Public URL of this MCP server (used in OAuth metadata)    | `https://xxxx.ngrok-free.app`      |
| `RUSHDB_JWT_PRIVATE_KEY` / `RUSHDB_JWT_PRIVATE_KEY_BASE64` | RS256 private key used to sign OAuth JWTs                 | `-----BEGIN PRIVATE KEY-----...`   |
| `RUSHDB_JWT_PUBLIC_KEY` / `RUSHDB_JWT_PUBLIC_KEY_BASE64`   | RS256 public key exposed in JWKS                          | `-----BEGIN PUBLIC KEY-----...`    |
| `RUSHDB_JWT_KID`                                           | Key id included in JWT header and JWKS entries            | `rushdb-mcp-rs256`                 |
| `RUSHDB_AES_256_ENCRYPTION_KEY`                            | HS256 fallback secret for local/self-hosted compatibility | `32SymbolStringForTokenEncryption` |
