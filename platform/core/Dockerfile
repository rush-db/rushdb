###################
# DEVELOPMENT
###################
FROM node:18.14.1-bullseye-slim As development

WORKDIR /app

RUN apt-get update && apt-get install -y procps

# Copy package.json and pnpm-lock.yaml to the container
COPY ./platform/core/package*.json pnpm-lock.yaml ./platform/core/tsconfig*.json ./platform/core/nest-cli.json ./

# Install development dependencies
RUN npm install -g pnpm && pnpm install --recursive --filter ./platform/core --no-frozen-lockfile

# Copy the rest of the application code
COPY ./platform/core/src ./src

# Build the application (optional)
RUN pnpm build --recursive --filter ./platform/core

###################
# PRODUCTION
###################
FROM node:18.14.1 As production

# Set working directory
WORKDIR /app

RUN apt-get update && apt-get install -y procps

# Copy package.json and pnpm-lock.yaml to the container
COPY ./platform/core/package.json pnpm-lock.yaml ./

# Install only production dependencies
RUN pnpm install --no-frozen-lockfile --production=true

# Copy built files from the development stage
COPY --from=development /app/dist ./dist

# Set the command to start the application
CMD ["node", "dist/main.js"]
