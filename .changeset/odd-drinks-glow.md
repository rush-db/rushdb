---
'@rushdb/javascript-sdk': patch
'@rushdb/mcp-server': patch
'rushdb-dashboard': patch
'@rushdb/skills': patch
'rushdb-core': patch
'rushdb-docs': patch
---

Rework `$cycle` and stabilize relationship/query intelligence

- **`$cycle` is now a record-level operator**: `where: { $cycle: { type, direction, hops } }` — the value IS the traversal spec. Replaces the previous `KEY: { $cycle: true, $relation: {...} }` block form entirely (breaking: the old form now throws). Compiles to an `EXISTS` subquery so the engine stops at the first cycle found per record instead of enumerating every path — avoids exponential blowup on densely connected graphs. Default traversal hop cap (`RUSHDB_MAX_TRAVERSAL_HOPS`) lowered from 25 to 10 to keep worst-case queries within the transaction budget.
- **Relationship pattern suggestions are now verified against live data**: candidates are proposed from schema names/types and LLM semantic judgment, then confirmed with a real graph probe before being surfaced — sampled schema values (which carry no signal on high-cardinality data) no longer gate or reject a suggestion.
- SmartSearch prompts and specs (core, MCP server, skills, docs) made more data-agnostic and kept in sync across all four DSL mirrors.
- Fix: CSV import no longer fails when a column parses to a JS `Date` (folded back to ISO string).
- Fix: dashboard record search box text no longer leaks between project switches.
