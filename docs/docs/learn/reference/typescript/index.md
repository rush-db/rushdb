---
slug: /reference/typescript/
sidebar_position: 0
title: TypeScript / JavaScript SDK
---

# TypeScript / JavaScript SDK

The official TypeScript/JavaScript SDK for RushDB. Works in Node.js and browser environments.

## Installation

```bash
npm install @rushdb/javascript-sdk
```

## Quick Start

```typescript
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('RUSHDB_API_KEY')

// Store a record
await db.records.create({
  label: 'User',
  data: { name: 'Alice', role: 'engineer' }
})

// Query records
const { data } = await db.records.find({
  labels: ['User'],
  where: { role: 'engineer' },
  limit: 10
})
```

To connect to a self-hosted instance pass the `url` option:

```typescript
const db = new RushDB('RUSHDB_API_KEY', {
  url: 'https://your-rushdb-instance.com/api/v1'
})
```

## API Reference

| Class                                                                  | Description                                                                  |
| ---------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| [RushDB](/reference/typescript/RushDB)                                 | Main client — entry point for all operations                                 |
| [DBRecord](/reference/typescript/DBRecord)                             | Typed record class returned from queries                                     |
| [DBRecordInstance](/reference/typescript/DBRecordInstance)             | Active record with instance methods (`attach`, `detach`, `update`, `delete`) |
| [DBRecordsArrayInstance](/reference/typescript/DBRecordsArrayInstance) | Collection of records with bulk-operation methods                            |
| [Model](/reference/typescript/Model)                                   | Schema-bound model class for label-scoped operations                         |
| [Relationship Patterns](/reference/typescript/relationship-patterns)   | Review and apply AI-suggested relationships                                  |
| [Transaction](/reference/typescript/Transaction)                       | ACID transaction handle                                                      |
| [SearchQuery](/reference/typescript/SearchQuery)                       | Query builder types and interfaces                                           |
| [DBRecordTarget](/reference/typescript/DBRecordTarget)                 | Record reference type used in relationship calls                             |
| [RelationTarget](/reference/typescript/RelationTarget)                 | Relationship target reference type                                           |
