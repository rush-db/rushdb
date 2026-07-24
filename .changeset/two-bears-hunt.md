---
'rushdb-core': minor
'rushdb-docs': minor
'@rushdb/javascript-sdk': minor
'@rushdb/mcp-server': minor
'@rushdb/skills': minor
'rushdb-dashboard': minor
---

Loosen datetime detection on import: `YYYY-MM-DD` (date-only) strings are now automatically typed as `datetime`, not just full ISO 8601 timestamps. This means values like `2026-07-23` work the same as `2026-07-23T12:00:00Z` — they get datetime comparisons, time-based aggregations, and the correct `__proptypes` entry — without any extra configuration.
