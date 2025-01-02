# Dockerfile
FROM node:18-alpine AS builder

# Install pnpm
RUN npm install -g pnpm

# Set working directory
WORKDIR /app
ENV NODE_OPTIONS="--max-old-space-size=4096"

# Copy package files
COPY ../../package.json ../../pnpm-lock.yaml ../../pnpm-workspace.yaml ./
COPY ./platform/dashboard/package.json ./platform/dashboard/package.json
COPY ./platform/core/package.json ./platform/core/package.json
COPY ./packages/javascript-sdk/package.json ./packages/javascript-sdk/

# Install dependencies
RUN pnpm install --frozen-lockfile --shamefully-hoist

# Copy source files excluding node_modules
COPY ./packages/javascript-sdk ./packages/javascript-sdk
COPY ./platform/dashboard/src ./platform/dashboard/src
COPY ./platform/dashboard/config ./platform/dashboard/config
COPY ./platform/dashboard/public ./platform/dashboard/public
COPY ./platform/dashboard/index.html ./platform/dashboard/
COPY ./platform/dashboard/tsconfig.json ./platform/dashboard/
COPY ./platform/dashboard/tsconfig.node.json ./platform/dashboard/
COPY ./platform/dashboard/vite.config.ts ./platform/dashboard/
COPY ./platform/dashboard/tailwind.config.ts ./platform/dashboard/
COPY ./platform/dashboard/postcss.config.js ./platform/dashboard/
COPY ./platform/dashboard/global.d.ts ./platform/dashboard/

COPY ./platform/core/src ./platform/core/src
COPY ./platform/core/tsconfig.json ./platform/core/
COPY ./platform/core/tsconfig.build.json ./platform/core/
COPY ./platform/core/nest-cli.json ./platform/core/

# Build javascript-sdk first
RUN cd packages/javascript-sdk && pnpm build

# Build dashboard
ENV VITE_BACKEND_BASE_URL=""
RUN cd platform/dashboard && pnpm build

# Create public directory in core and copy dashboard build
RUN mkdir -p platform/core/public
RUN cp -r platform/dashboard/dist/* platform/core/public/

# Build core
RUN cd platform/core && pnpm build

# Production image
FROM --platform=$TARGETPLATFORM node:18-alpine

ENV NODE_ENV=production \
    RUSHDB_PORT=3000 \
    RUSHDB_AES_256_ENCRYPTION_KEY=32SymbolStringForTokenEncryption \
    RUSHDB_DASHBOARD_URL=/ \
    RUSHDB_SELF_HOSTED=true \
    RUSHDB_LOGIN=admin \
    RUSHDB_PASSWORD=password

WORKDIR /app

# Copy package files
COPY ../../package.json ../../pnpm-lock.yaml ../../pnpm-workspace.yaml ./
COPY ./platform/core/package.json ./platform/core/package.json

# Install pnpm and production dependencies
RUN npm install -g pnpm
RUN pnpm install --prod --frozen-lockfile --no-optional

# Copy built files from builder
COPY --from=builder /app/platform/core/dist ./platform/core/dist
COPY --from=builder /app/platform/core/public ./platform/core/public

# Set working directory to core
WORKDIR /app/platform/core

# Expose port
EXPOSE 3000

# Start the application
CMD ["node", "dist/main.js"]