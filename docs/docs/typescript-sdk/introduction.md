---
sidebar_position: 1
title: Introduction
---

# RushDB TypeScript/JavaScript SDK

Welcome to the comprehensive guide on working with the RushDB SDK. This SDK provides a modern, flexible interface for managing your data, relationships, and metadata in RushDB through JavaScript and TypeScript applications.

## What is RushDB SDK?

The RushDB JavaScript/TypeScript SDK is a powerful client library that lets you interact with RushDB's features directly from your JavaScript or TypeScript applications. Whether you're building web applications, server backends, or automation scripts, this SDK gives you full access to RushDB's capabilities with an intuitive, type-safe API.

## Highlights

- **✨ No Configuration Needed**: Plug-and-play design requires minimal setup to get started
- **🤖 Automatic Type Inference**: Enjoy seamless type safety with automatic TypeScript inference
- **↔️ Isomorphic Architecture**: Fully compatible with both server and browser environments
- **🏋️ Zero Dependencies**: Lightweight (just 6.9KB gzipped) and efficient with no external dependencies

## Getting Started

### Installation

To begin using RushDB SDK, add it to your project with your preferred package manager:

```bash
# Using npm
npm install @rushdb/javascript-sdk

# Using yarn
yarn add @rushdb/javascript-sdk

# Using pnpm
pnpm add @rushdb/javascript-sdk
```

### Quick Setup

After installation, create an instance of the RushDB SDK in your project:

```typescript
import RushDB from '@rushdb/javascript-sdk';

const db = new RushDB('API_TOKEN');
```

Replace `API_TOKEN` with your actual API token from the [RushDB Dashboard](https://app.rushdb.com/).

### Usage Example

```typescript
import RushDB from '@rushdb/javascript-sdk'

// Setup SDK
const db = new RushDB("API_TOKEN");

// Push any data, and RushDB will automatically flatten it into Records
// and establish relationships between them accordingly.
await db.records.createMany({
  label: "COMPANY",
  data: {
    name: 'Google LLC',
    address: '1600 Amphitheatre Parkway, Mountain View, CA 94043, USA',
    foundedAt: '1998-09-04T00:00:00.000Z',
    rating: 4.9,
    DEPARTMENT: [{
      name: 'Research & Development',
      description: 'Innovating and creating advanced technologies for AI, cloud computing, and consumer devices.',
      // Nested relationships are automatically created
      PROJECT: [{
        name: 'Bard AI',
        // ... more properties
      }]
    }]
  }
})

// Find Records by specific criteria
const employees = await db.records.find({
  labels: ['EMPLOYEE'],
  where: {
    position: { $contains: 'AI' }
  }
})
```

## SDK Configuration Options

The RushDB SDK is designed to be flexible and configurable. When initializing the SDK, you can provide configuration options to customize its behavior.

### Constructor Parameters

```typescript
const db = new RushDB(token, config);
```

**Parameters:**

- `token` (`string`): Your API token from the RushDB Dashboard
- `config` (`SDKConfig`): Optional configuration object

### Configuration Object (`SDKConfig`)

The configuration object allows you to customize the SDK's behavior and connection details:

```typescript
type SDKConfig = {
  httpClient?: HttpClientInterface;
  timeout?: number;
  logger?: Logger;
  options?: {
    allowForceDelete?: boolean;
  }
} & ApiConnectionConfig;
```

Where `ApiConnectionConfig` is either:

```typescript
{
  host?: string;
  port?: number;
  protocol?: string;
}
```

Or:

```typescript
{
  url?: string;
}
```

### Configuration Options Explained

- **Connection settings**:
  - `url`: The complete URL to the RushDB API (e.g., `https://api.rushdb.com`)
  - **OR** the individual components:
    - `host`: The domain name or IP address (e.g., `api.rushdb.com`)
    - `port`: The port number (defaults to 80 for HTTP, 443 for HTTPS)
    - `protocol`: Either `http` or `https` (defaults to `https`)

- **Advanced options**:
  - `timeout`: Request timeout in milliseconds (default: 30000)
  - `httpClient`: Custom HTTP client implementation
  - `logger`: Custom logging function
  - `options.allowForceDelete`: When set to `true`, allows deleting all records without specifying criteria (defaults to `false` for safety)

### Example with Configuration

```typescript
import RushDB from '@rushdb/javascript-sdk';

const db = new RushDB('API_TOKEN', {
  url: 'http://localhost:3000',
  timeout: 5000,
  options: {
    allowForceDelete: false
  }
});
```

## Next Steps

To continue learning about the RushDB TypeScript SDK, explore these related sections:

- [Working with Records](../typescript-sdk/records/create-records.md)
- [Managing Relationships](../typescript-sdk/relationships)
- [Working with Properties](../typescript-sdk/properties)
- [Working with Labels](../typescript-sdk/labels)
- [Working with Transactions](../typescript-sdk/transactions)

Before you begin exploring the SDK features, make sure you have a valid API token. If you haven't set up your RushDB account yet, follow our guide to [registering on the dashboard and generating an API token](../get-started/quick-tutorial.mdx).
