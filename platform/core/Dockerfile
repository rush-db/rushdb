###################
# DEVELOPMENT
###################
FROM node:18.14.1-bullseye-slim As development

WORKDIR /app

RUN apt-get update && apt-get install -y procps

# Copy package.json and yarn.lock to the container
COPY package*.json yarn.lock tsconfig*.json nest-cli.json ./

# Install development dependencies
RUN yarn install --frozen-lockfile --production=false

# Copy the rest of the application code
COPY src ./src

# Build the application (optional)
RUN yarn build

###################
# PRODUCTION
###################
FROM node:18.14.1 As production

# Set working directory
WORKDIR /app

RUN apt-get update && apt-get install -y procps

# Copy package.json and yarn.lock to the container
COPY package.json yarn.lock ./

# Install only production dependencies
RUN yarn install --frozen-lockfile --production=true

# Copy built files from the development stage
COPY --from=development /app/dist ./dist

# Set the command to start the application
CMD ["node", "dist/main.js"]
