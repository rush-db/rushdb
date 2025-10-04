---
'@rushdb/javascript-sdk': minor
'rushdb-dashboard': minor
'rushdb-core': minor
'rushdb-website': minor
'rushdb-docs': minor
---

## Summary
Adds first-class grouping support to the Search API (`groupBy`) across core, JavaScript SDK, dashboard, website, and docs. Also standardizes terminology (`uniq` -> `unique`), refines aggregation semantics, and updates documentation with a dedicated grouping concept page.

---
## ✨ New Feature: `groupBy` Clause
You can now pivot / summarize search results by one or more keys. Keys reference an alias + property (root alias is implicitly `$record`).

Example (JS SDK):
```ts
const dealsByStage = await db.records.find({
	labels: ['HS_DEAL'],
	aggregate: {
		count: { fn: 'count', alias: '$record' },
		avgAmount: { fn: 'avg', field: 'amount', alias: '$record' }
	},
	groupBy: ['$record.dealstage'],
	orderBy: { count: 'desc' }
});
// → rows like: [{ dealstage: 'prospecting', count: 120, avgAmount: 3400 }, ...]
```

Key capabilities:
* Multiple grouping keys: `groupBy: ['$record.category', '$record.active']`
* Group by related aliases (declare alias in `where` traversal first)
* Works with all existing aggregation functions (count, sum, avg, min, max, collect, similarity, etc.)
* Ordering applies to aggregated rows when `groupBy` is present
* Requires at least one aggregation entry to take effect

Result shape when using `groupBy`: each row contains only the grouping fields plus aggregated fields (raw record bodies are not returned unless you also aggregate them via `collect`).

---
## 🔄 Aggregation & Semantics Updates
* `collect` results are unique by default. Set `unique: false` to retain duplicates.
* Aggregation entries now consistently use the `unique` flag (replacing legacy `uniq`).
* Distinct handling for grouped queries unified under the `unique` option.
* Improved Cypher generation: clearer alias usage and property quoting; vector similarity function formatting tightened.
* Added internal `PROPERTY_WILDCARD_PROJECTION` support (enables future selective projections) – not yet a public API, but impacts generated queries.

---
## 💥 Breaking Changes
| Area | Change | Action Required |
|------|--------|-----------------|
| Schema field definitions | `uniq` key renamed to `unique` | Rename all occurrences (`{ uniq: true }` → `{ unique: true }`). |
| Aggregation definitions | Aggregator option `uniq` renamed to `unique` | Update custom aggregation objects (`uniq: false` → `unique: false`). |
| Result shape (when using `groupBy`) | Raw record objects no longer returned automatically | If you previously expected full records, add a `collect` aggregation (e.g. `rows: { fn: 'collect', alias: '$record' }`). |
| Default uniqueness for `collect` | Now unique by default | Add `unique: false` if you require duplicates. |
| Internal alias constant | `DEFAULT_RECORD_ALIAS` → `ROOT_RECORD_ALIAS` | Only relevant if you referenced internal constants (avoid relying on these). |

If any code or saved JSON queries still send `uniq`, they will now fail unless a compatibility shim exists (none added in this release). Treat this as a required migration.

---
## 🛠 Migration Guide
1. Rename all schema property options:
	 * Before: `email: { type: 'string', uniq: true }`
	 * After:  `email: { type: 'string', unique: true }`
2. Update aggregation specs:
	 * Before: `names: { fn: 'collect', field: 'name', alias: '$user', uniq: true }`
	 * After:  `names: { fn: 'collect', field: 'name', alias: '$user' }` (omit `unique` if true)
3. Reintroduce duplicate collection (if needed): add `unique: false`.
4. When adopting `groupBy`, ensure at least one aggregation is defined; queries with only `groupBy` are invalid.
5. Adjust consumer code to handle aggregated row shape instead of full record instances.
6. For hierarchical drill‑downs: group at the parent level; use nested `collect` for children instead of adding child keys to `groupBy`.

### Example Migration (JS)
```diff
 aggregate: {
-  employeeNames: { fn: 'collect', field: 'name', alias: '$employee', uniq: true },
+  employeeNames: { fn: 'collect', field: 'name', alias: '$employee' },
 }
```

### Adding Grouping
```ts
const deptProjects = await db.records.find({
	labels: ['DEPARTMENT'],
	where: { PROJECT: { $alias: '$project' } },
	aggregate: {
		projectCount: { fn: 'count', alias: '$project' },
		projects: { fn: 'collect', field: 'name', alias: '$project', unique: true }
	},
	groupBy: ['$record.name'],
	orderBy: { projectCount: 'desc' }
});
```

---
## 📘 Documentation
* Added dedicated concept page: `concepts/search/group-by` centralizing all grouping patterns (multi-key, alias-based, nested, uniqueness nuances, limitations).
* Updated Python, REST, and TypeScript SDK "Get Records" guides with concise grouping sections linking to the concept page.
* Refactored Aggregations doc to avoid duplication and point to new grouping guide.
* Standardized examples to use `unique` terminology.

---
## 🧪 Tests & Internal Refactors
* Extended aggregate & query builder tests to cover `groupBy` permutations (single key, multi-key, alias grouping, collect uniqueness flags).
* Parser adjustments for: property quoting, alias resolution, vector similarity formatting, optional matches, and root alias constant rename.
* Introduced `AggregateContext` enhancements to track grouping state.

---
## ⚠️ Edge Cases & Notes
* An empty `groupBy` array is ignored; supply at least one key.
* Supplying a group key for a property that does not exist yields rows with `null` for that column (consistent with underlying graph behavior) – validate upstream if needed.
* Ordering by an aggregation that isn't defined will be rejected; always define the aggregate you sort by.
* To sort by a group key, just reference it in `orderBy` using the property name (without alias prefix) after grouping.

---
## ✅ Quick Checklist
| Task | Done? |
|------|-------|
| Renamed all `uniq` → `unique` in schema & aggregations | |
| Reviewed any `collect` aggregations for unintended de-duplication | |
| Added `unique: false` where duplicates are required | |
| Updated UI / API consumers for aggregated row shape under `groupBy` | |
| Added `collect` fields if raw record snapshots are still needed | |
| Added / validated ordering under grouped queries | |

---
## Feedback
Please report any unexpected behavior with grouped queries (especially multi-key or alias-based grouping) so we can refine edge case handling in upcoming releases.

---
## TL;DR
Use `groupBy` + `aggregate` to pivot results; rename `uniq` → `unique`; `collect` is now unique by default; aggregated queries return row sets, not raw records.