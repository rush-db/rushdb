---
sidebar_position: 1
---
# Introduction to RushDB SDK
:::note
Welcome to the comprehensive guide on working with the RushDB SDK. This section provides an overview of initializing the RushDB SDK, a crucial first step for integrating RushDB into your applications. Understanding the initialization process is key to effectively manage and interact with your data through RushDB.
:::

## Table of Contents

- [Installation](#initializing-rushdb-sdk)
- [SDK Class Constructor](#constructor-parameters)

## Initializing RushDB SDK

The RushDB SDK is designed to be straightforward and easy to set up. You start by importing the SDK and then creating an instance with your API token and optional configuration settings. Here's the basic way to initialize the SDK:

```javascript
import RushDB from '@rushdb/javascript-sdk';

const db = new RushDB('API_TOKEN', {
  url: 'http://localhost:3000'
});
```

## Constructor Parameters
The RushDB constructor accepts two parameters:

- `token` (`string`): This is the API token issued from the RushDB Dashboard. It is essential for authenticating your application's requests to the RushDB API.
- `config` (`UserProvidedConfig`): This parameter allows for advanced configuration of the SDK to specify how it connects to the backend.

### Understanding `UserProvidedConfig` (currently not available)
This feature is currently unavailable and will be available soon. The UserProvidedConfig provides detailed control over how the SDK interacts with your backend. It consists of the following structure:

```typescript

type ApiConnectionConfig = {
  host: string;
  port: number;
  protocol: string;
  } | {
  url: string;
};

type CommonUserProvidedConfig = {
  httpClient?: HttpClientInterface;
  timeout?: number;
  validator?: Validator;
} & ApiConnectionConfig;

export type UserProvidedConfig = CommonUserProvidedConfig;
```

### Config Options

- `url` (`string`): The simplest form of configuration, just providing the URL where the RushDB backend is hosted.

- `host, port, protocol`: An alternative to `url`, providing a more granular way to define the connection parameters:
    - `host`: The domain name or IP address of the server.
    - `port`: The port number on which the server is listening.
    - `protocol`: The protocol used for the connection (e.g., `http`, `https`).

- `httpClient` (`HttpClientInterface`): Optional. Specifies a custom HTTP client interface for making network requests. Useful for environments where default HTTP clients do not meet specific needs.

- `timeout` (`number`): Optional. Defines a timeout period for each request in milliseconds.

- `validator` (`Validator`): Optional. Allows you to specify custom validation logic for outgoing data.

### When to Use Advanced Configuration

While the simple URL configuration suffices for most applications, advanced configuration options come into play in specific scenarios:

- **Custom environments**: If your application operates within a specific network environment or requires particular security protocols not covered by the default HTTP client.

- **Complex infrastructure setups**: When your RushDB backend is distributed across different domains or ports, especially in microservices architectures.

- **Special performance requirements**: If your application demands fine-tuned network behavior, such as custom timeout settings or specialized error handling.

By configuring these parameters, you can tailor the RushDB SDK to fit the precise needs of your application's infrastructure and operational requirements.

In the next sections, we'll delve into defining data models and performing CRUD operations using the RushDB SDK, laying the groundwork for building robust data-driven applications.
