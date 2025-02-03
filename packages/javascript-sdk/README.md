<div align="center">

![RushDB Logo](https://raw.githubusercontent.com/rush-db/rushdb/main/rushdb-logo.svg)

# RushDB SDK for JavaScript and TypeScript

[![NPM Version](https://img.shields.io/npm/v/%40rushdb%2Fjavascript-sdk)](https://www.npmjs.com/package/@rushdb/javascript-sdk)
[![NPM License](https://img.shields.io/npm/l/%40rushdb%2Fjavascript-sdk)](#license "Go to license section")

![NPM Downloads](https://img.shields.io/npm/dw/%40rushdb%2Fjavascript-sdk)
![npm bundle size](https://img.shields.io/bundlephobia/minzip/%40rushdb%2Fjavascript-sdk)


[![Made with Node](https://img.shields.io/badge/dynamic/json?label=node&query=%24.engines%5B%22node%22%5D&url=https%3A%2F%2Fraw.githubusercontent.com%2Frush-db%2Frushdb%2Fmain%2Fpackage.json)](https://nodejs.org "Go to Node.js homepage")
[![Package - Typescript](https://img.shields.io/github/package-json/dependency-version/rush-db/rushdb/dev/typescript?logo=typescript&logoColor=white)](https://www.npmjs.com/package/typescript "Go to TypeScript on NPM")

[Homepage](https://rushdb.com) — [Blog](https://rushdb.com/blog) — [Platform](https://app.rushdb.com) — [Docs](https://docs.rushdb.com) — [Examples](https://github.com/rush-db/examples)
</div>

## Highlights

---
> **✨ No Configuration Needed**: Plug-and-play design requires no setup or configuration.

> **🤖 Automatic Type Inference**: Enjoy seamless type safety with automatic TypeScript inference.

> **↔️ Isomorphic Architecture**: Fully compatible with both server and browser environments.

> **🏋️ Zero Dependencies**: Lightweight and efficient with no external dependencies.



## Installation

---
NPM:
```bash
npm install @rushdb/javascript-sdk
```

YARN:
```bash
yarn add @rushdb/javascript-sdk
```

PNPM:
```bash
pnmp add @rushdb/javascript-sdk
```


## Usage

---

1. **Obtain API Token**: Grab one from the [Dashboard](https://app.rushdb.com).
2. **Build anything**: Easily push, search, and manage relationships within your data.

### TLDR;
```ts
import RushDB, { Model } from '@rushdb/javascript-sdk'

// Setup SDK
const db = new RushDB("API_TOKEN", {
  // This is the default URL; no need to provide it unless overriding.
  url: "https://api.rushdb.com", 
});

// Push any data, and RushDB will automatically flatten it into Records 
// and establish relationships between them accordingly.
await db.records.createMany("COMPANY", {
  name: 'Google LLC',
  address: '1600 Amphitheatre Parkway, Mountain View, CA 94043, USA',
  foundedAt: '1998-09-04T00:00:00.000Z',
  rating: 4.9,
  DEPARTMENT: [{
    name: 'Research & Development',
    description:
        'Innovating and creating advanced technologies for AI, cloud computing, and consumer devices.',
    PROJECT: [{
      name: 'Bard AI',
      description:
          'A state-of-the-art generative AI model designed for natural language understanding and creation.',
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

const company = await db.records.findUniq('COMPANY', {
  where: {
    name: 'Google LLC'
  }
})

// Manage relationships
await company.attach(matchedEmployees, { type: "WORKING_AT" })
```

<div align="center">
<b>You're Awesome!</b>  🚀
</div>

---

<div align="center" style="margin-top: 20px">

> Check the [Docs](https://docs.rushdb.com) and [Examples Repository](https://github.com/rush-db/examples) to learn more 🤓


</div>


## Contributing

---
See [CONTRIBUTING.md](CONTRIBUTING.md).

