FROM node:23.11-slim AS node_base
WORKDIR /app

FROM node_base AS builder
COPY package.json package-lock.json tsconfig.json ./
RUN --mount=type=cache,target=/root/.npm npm ci --ignore-scripts --omit-dev

COPY . ./
RUN --mount=type=cache,target=/root/.npm npm run build

FROM node_base
COPY package.json package-lock.json ./
COPY --from=builder /app/build ./build
ENV NODE_ENV=production
# MCP_TRANSPORT: "stdio" (default) | "http"
# Override with -e MCP_TRANSPORT=http to enable the Streamable HTTP server
ENV MCP_TRANSPORT=stdio
# HTTP mode: server listens on this port (only relevant when MCP_TRANSPORT=http)
EXPOSE 3001
RUN --mount=type=cache,target=/root/.npm npm ci --ignore-scripts --omit=dev
ENTRYPOINT ["node", "/app/build/index.js"]
