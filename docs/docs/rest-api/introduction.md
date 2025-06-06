---
title: Introduction
sidebar_position: 0
---

# RushDB REST API

Welcome to the RushDB REST API documentation! The RushDB REST API provides a modern, flexible interface for managing your data, relationships, and metadata in RushDB. Whether you are building applications, automating workflows, or integrating with other systems, the API gives you full control over your graph data with simple HTTP requests.

## What is RushDB?

RushDB is an instant, cloud-native database built on top of Neo4j, designed for modern applications and data science/ML operations. It automates data normalization, manages relationships, and features automatic type inference, so you can focus on building features instead of managing data infrastructure.

## Key Features

- **Flexible Data Model**: Store structured, semi-structured, and nested data as records and relationships.
- **Relationship Management**: Easily create, query, and manage relationships between records.
- **Batch Operations**: Import and export data in bulk using JSON or CSV.
- **ACID Transactions**: Perform multiple operations atomically for data consistency.
- **Powerful Search**: Query records with advanced filters, ordering, and pagination.
- **Property & Label APIs**: Manage metadata, property types, and record labels.
- **Secure & Scalable**: Built for both cloud and self-hosted deployments, with robust authentication and access control.

## How to Use the API

- **Base URL**: The API is available at `https://api.rushdb.com/api/v1` for cloud users, or your custom URL for self-hosted deployments.
- **Authentication**: All endpoints require authentication via a token header. Get your API token from the [RushDB dashboard](https://app.rushdb.com).
- **Content-Type**: All requests and responses use JSON unless otherwise specified.

## API Specifications

The RushDB API is documented using OpenAPI (Swagger) specification for easy integration and exploration:

- **Swagger UI**: [Interactive API Documentation](https://api.rushdb.com/api)
- **OpenAPI JSON**: [JSON Schema Specification](https://api.rushdb.com/api-json)
- **OpenAPI YAML**: [YAML Specification](https://api.rushdb.com/api-yaml)

You can use these specifications to:
- Generate client libraries in your preferred programming language
- Import the API into tools like Postman, Insomnia, or SwaggerHub
- Understand request/response formats with machine-readable schemas

## Common Use Cases

- Create, update, and delete records
- Manage relationships between records
- Import/export data in bulk
- Search and filter records with complex queries
- Manage property types and labels
- Use transactions for atomic multi-step operations

## Getting Started

1. **Get an API Token**: Sign up at [app.rushdb.com](https://app.rushdb.com) or set up a self-hosted instance.
2. **Read the Endpoint Docs**: Explore the sidebar for detailed documentation on each API endpoint, including request/response formats and examples.
3. **Try It Out**: Use cURL, Postman, or your favorite HTTP client to interact with the API.

## Example: Create a Record

```http
POST /api/v1/records
Content-Type: application/json
token: RUSHDB_API_TOKEN

{
  "label": "Person",
  "data": {
    "name": "John Doe",
    "age": 30,
    "email": "john.doe@email.com"
  }
}
```

## Support & Resources

- [RushDB Documentation](https://docs.rushdb.com)
- [RushDB Homepage](https://rushdb.com)
- [Community & Support](https://rushdb.com/contact)

---

Browse the sidebar to learn more about each API endpoint, best practices, and advanced features!
