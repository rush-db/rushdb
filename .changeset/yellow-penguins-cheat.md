---
'@rushdb/javascript-sdk': minor
'@rushdb/mcp-server': minor
'@rushdb/skills': minor
'rushdb-core': minor
'rushdb-docs': minor
'rushdb-dashboard': minor
---

Add variable-length (multihop) traversal and cycle detection to SearchQuery

**`$relation.hops`** — traverse a relationship pattern up to N hops in a single `where` block, without naming intermediate records:

```ts
db.records.find({
  labels: ['EMPLOYEE'],
  where: {
    EMPLOYEE: {
      $alias: '$manager',
      $relation: { type: 'REPORTS_TO', direction: 'out', hops: { min: 1, max: 4 } },
      name: { $contains: 'Alice' }
    }
  }
})
```

`hops` accepts an exact count (`hops: 3`) or a range (`{ min?, max? }`, `min` defaults to 1). `type` and `direction` apply to every hop; the nested label and its criteria constrain only the endpoint record. Omitting `type` traverses any relationship — RushDB's internal property metadata edges are automatically excluded, so untyped traversal never leaves the user's data model.

**`$cycle`** — find records sitting on a closed path back to themselves (fraud rings, circular ownership, dependency cycles):

```ts
db.records.find({
  labels: ['ACCOUNT'],
  where: {
    RING: {
      $cycle: true,
      $relation: { type: 'TRANSFERRED_TO', direction: 'out', hops: { min: 2, max: 6 } }
    }
  }
})
```

A `$cycle` block requires `$relation` with `hops` (`min` ≥ 2, the default) and accepts no other keys; its label key is a display name. Combine with `$not` to select records not on a cycle.

**Traversal depth policy** — `hops.max` is capped per deployment via `RUSHDB_MAX_TRAVERSAL_HOPS` (default 25) on the shared cloud connection. Self-hosted deployments (`RUSHDB_SELF_HOSTED=true`) and projects with a custom external Neo4j allow unbounded traversal (`max` omitted), guarded by the existing transaction timeout.

Also in this release:

- New SDK types: `TraversalHops` and `TraversalRelationOptions` (TypeScript); matching `TypedDict`s in the Python SDK.
- NL→query (smart search), the MCP server spec/tools, and the query-builder/data-modeling skills all understand `hops` and `$cycle`.
- Docs: full operator reference in Where Operators, updated hierarchy-modeling tutorial, and a new "Detecting Fraud Rings" tutorial.
