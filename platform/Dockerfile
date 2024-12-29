# Base image for Node.js
FROM node:18-alpine

# Set environment variables
ENV NODE_ENV=production

# Create directories for the apps
WORKDIR /usr/src/platform

# Copy root package manager files and install dependencies
COPY package.json pnpm-lock.yaml ./
RUN npm install -g pnpm && pnpm install --frozen-lockfile --filter ./core... ./dashboard...

# Copy source files for both apps
COPY ./core ./core
COPY ./dashboard ./dashboard

# Build both apps
RUN cd core && pnpm build
RUN cd ../dashboard && pnpm build

# Expose ports for both apps
EXPOSE 3000 5173

# Start both services using a process manager
RUN npm install -g concurrently
CMD concurrently "pnpm --prefix core start" "pnpm --prefix dashboard start"