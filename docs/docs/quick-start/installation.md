---
sidebar_position: 1
---
# Installation
Getting started with RushDB SDK is straightforward. This section will guide you through installing the SDK and setting up your first SDK instance.

## Step 1: Install the Package

To begin, you need to add the RushDB SDK to your project.

### TypeScript / JavaScript

Using npm:

```bash
npm install @rushdb/javascript-sdk
```

Using yarn: 
```bash
yarn add @rushdb/javascript-sdk
```

Using pnpm:
```bash
pnpm add @rushdb/javascript-sdk
```

The RushDB SDK is lightweight, coming in at just 6.9KB gzipped. Learn more about the package size [here](https://pkg-size.dev/@rushdb%2Fjavascript-sdk).

### Python

```bash
pip install rushdb
```

## Step 2: Initialize the SDK
Once the package is installed, you can create an instance of the RushDB SDK in your project.

### TypeScript / JavaScript

```typescript
import RushDB from '@rushdb/javascript-sdk';

const db = new RushDB('API_TOKEN');
```

### Python

```bash
from rushdb import RushDB

db = RushDB("API_TOKEN")
```

Replace `API_TOKEN` with your actual API token, which you can obtain from the [RushDB Dashboard](https://app.rushdb.com/).

## Next steps
To make full use of the SDK, you'll need a valid API token. In the [next section](/quick-start/configuring-dashboard), Configuring RushDB Dashboard, we'll guide you through the process of registering on the dashboard, creating a project, and generating your API token.
