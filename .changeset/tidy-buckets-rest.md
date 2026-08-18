---
'rushdb-core': patch
---

Fix `select.$timeBucket` queries by converting RushDB's string-backed datetime values before reading calendar components and by checking datetime metadata on the referenced record alias.
