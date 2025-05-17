<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# 🚀 RushDB SDK for JavaScript and TypeScript

### The Modern Graph Database Client for Web & Node.js

[![NPM Version](https://img.shields.io/npm/v/%40rushdb%2Fjavascript-sdk)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![NPM License](https://img.shields.io/npm/l/%40rushdb%2Fjavascript-sdk)](#license "Go to license section")
[![NPM Downloads](https://img.shields.io/npm/dw/%40rushdb%2Fjavascript-sdk)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![npm bundle size](https://img.shields.io/bundlephobia/minzip/%40rushdb%2Fjavascript-sdk)](https://bundlephobia.com/package/@rushdb/javascript-sdk)

[![Made with Node](https://img.shields.io/badge/dynamic/json?label=node&query=%24.engines%5B%22node%22%5D&url=https%3A%2F%2Fraw.githubusercontent.com%2Frush-db%2Frushdb%2Fmain%2Fpackage.json)](https://nodejs.org "Go to Node.js homepage")
[![Package - Typescript](https://img.shields.io/github/package-json/dependency-version/rush-db/rushdb/dev/typescript?logo=typescript&logoColor=white)](https://www.npmjs.com/package/typescript "Go to TypeScript on NPM")

[🌐 Website](https://rushdb.com) • [📖 Documentation](https://docs.rushdb.com) • [☁️ Cloud Platform](https://app.rushdb.com) • [🔍 Examples](https://github.com/rush-db/examples)
</div>

## ✨ Highlights

---

- **🔌 No Configuration Needed**: Plug-and-play design requires minimal setup to get started
- **🛡️ Automatic Type Inference**: Enjoy seamless type safety with automatic TypeScript inference
- **🔄 Isomorphic Architecture**: Fully compatible with both server and browser environments
- **⚡ Zero Dependencies**: Lightweight (just 6.9KB gzipped) and efficient with no external dependencies


## 📦 Installation

---
```bash
# NPM
npm install @rushdb/javascript-sdk

# YARN
yarn add @rushdb/javascript-sdk

# PNPM
pnpm add @rushdb/javascript-sdk
```


## 🚀 Usage

---

1. **Obtain API Token**: Grab one from the [RushDB Cloud Dashboard](https://app.rushdb.com).
2. **Build anything**: Easily push, search, and manage relationships within your data.

### Quick Example
```ts
import RushDB from '@rushdb/javascript-sdk';

// Setup SDK
const db = new RushDB("API_TOKEN");
// Default URL is "https://api.rushdb.com". See SDK Configuration for more options.

// Push any data, and RushDB will automatically flatten it into Records
// and establish relationships between them accordingly.
await db.records.createMany({
  label: "COMPANY",
  payload: {
    name: 'Google LLC',
    address: '1600 Amphitheatre Parkway, Mountain View, CA 94043, USA',
    foundedAt: '1998-09-04T00:00:00.000Z',
    rating: 4.9,
    DEPARTMENT: [{
      name: 'Research & Development',
      description: 'Innovating and creating advanced technologies for AI, cloud computing, and consumer devices.',
      PROJECT: [{
        name: 'Bard AI',
        description: 'A state-of-the-art generative AI model designed for natural language understanding and creation.',
        active: true,
        budget: 1200000000,
        EMPLOYEE: [{
          name: 'Jeff Dean',
          position: 'Head of AI Research',
          email: 'jeff@google.com',
          dob: '1968-07-16T00:00:00.000Z',
          salary: 3000000
        }]
      }]
    }]
  }
})


// Find Records by specific criteria
const matchedEmployees = await db.records.find({
  labels: ['EMPLOYEE'],
  where: {
    position: { $contains: 'AI' },
    PROJECT: {
      DEPARTMENT: {
        COMPANY: {
          rating: { $gte: 4 }
        }
      }
    }
  }
})

const company = await db.records.findUniq({
  labels: ['COMPANY'],
  where: {
    name: 'Google LLC'
  }
})

// Manage relationships
await company.attach(matchedEmployees, { type: "WORKING_AT" })
```

## ⚙️ SDK Configuration
---
The SDK is designed to be flexible. You can provide configuration options when initializing `RushDB` to customize its behavior, such as the API endpoint or request timeouts.

```typescript
import RushDB from '@rushdb/javascript-sdk';

// Example: Initialize with a custom URL and timeout
const db = new RushDB('API_TOKEN', {
  url: 'http://your-custom-rushdb-server.com', // Default is 'https://api.rushdb.com'
  timeout: 30000 // 30 seconds
});
```

The `RushDB` constructor accepts an optional second argument for configuration. Key options include:
- `url`: The complete URL to the RushDB API.
- Alternatively, `host`, `port`, and `protocol` can be specified individually.
- `timeout`: Request timeout in milliseconds (default: 10000).
- `httpClient`: For advanced use cases, provide a custom HTTP client implementation.
- `logger`: Implement a custom logger for debugging or monitoring.
- `options.allowForceDelete`: Set to `true` to enable deleting all records without specifying criteria (defaults to `false` for safety).

For a comprehensive guide on all available configuration options, please refer to the [SDK Configuration Documentation](https://docs.rushdb.com/typescript-sdk/introduction#sdk-configuration-options).

<div align="center">
<b>You're Awesome!</b>  🚀
</div>

---

<div align="center" style="margin-top: 20px">

> 📚 Check the [Complete Documentation](https://docs.rushdb.com/typescript-sdk/introduction) and [Examples Repository](https://github.com/rush-db/examples) to learn more!

</div>


## 🤝 Contributing

---
We welcome contributions! See our [CONTRIBUTING.md](CONTRIBUTING.md) guide for details.

