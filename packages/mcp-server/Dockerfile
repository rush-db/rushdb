FROM node:23.11-slim AS base
RUN npm install -g pnpm@10.1.0
WORKDIR /app

FROM base AS builder
# Copy workspace manifests for layer caching
COPY pnpm-workspace.yaml pnpm-lock.yaml package.json ./
COPY packages/javascript-sdk/package.json ./packages/javascript-sdk/
COPY packages/mcp-server/package.json ./packages/mcp-server/
RUN pnpm install --frozen-lockfile --ignore-scripts

COPY packages/javascript-sdk ./packages/javascript-sdk
COPY packages/mcp-server ./packages/mcp-server
RUN pnpm --filter @rushdb/javascript-sdk build
RUN pnpm --filter @rushdb/mcp-server build

FROM node:23.11-slim
WORKDIR /app
COPY --from=builder /app/packages/mcp-server/build ./build
COPY --from=builder /app/packages/mcp-server/package.json ./package.json
ENV NODE_ENV=production
# MCP_TRANSPORT: "stdio" (default) | "http"
ENV MCP_TRANSPORT=http
EXPOSE 3001
ENTRYPOINT ["node", "/app/build/index.js"]
