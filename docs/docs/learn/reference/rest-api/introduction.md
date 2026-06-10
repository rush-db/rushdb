---
title: Introduction
sidebar_position: 0
---

# REST API

Base URL: `https://api.rushdb.com/api/v1`
Auth: `Authorization: Bearer YOUR_TOKEN`

```bash
curl -X POST https://api.rushdb.com/api/v1/records \
  -H "Authorization: Bearer $RUSHDB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"label":"MOVIE","data":{"title":"Inception","rating":8.8}}'
```

Interactive docs: [Swagger UI](https://api.rushdb.com/api) · [OpenAPI JSON](https://api.rushdb.com/api-json)

## Endpoints

| Group                                                                     | Description                                          |
| ------------------------------------------------------------------------- | ---------------------------------------------------- |
| [Records](/learn/reference/rest-api/records/create-records)               | Create, read, update, delete, search, import, export |
| [Relationships](/learn/reference/rest-api/relationships)                  | Attach and detach edges between records              |
| [Labels](/learn/reference/rest-api/labels)                                | Query which types exist and their counts             |
| [Properties](/learn/reference/rest-api/properties)                        | Inspect field names, types, and value ranges         |
| [Transactions](/learn/reference/rest-api/transactions)                    | Atomic multi-step operations                         |
| [AI & Semantic Search](/learn/reference/rest-api/ai-and-vectors/overview) | Schema export + vector similarity search             |
| [Raw Queries](/learn/reference/rest-api/raw-queries)                      | Cypher pass-through (cloud only)                     |
