---
'rushdb-dashboard': patch
'rushdb-core': patch
---

Dashboard: reworked project Getting Started into a "Connect this project" page with a connection panel (API key, URL, status) and SDK-first setup tabs; neutral, non-shifting feedback for copy/test-connection buttons; unified JSON + language tabs in the Search Query modal.

Core: fixed flaky post-import side effects — record/property counts, schema recalculation, and relationship suggestions could be skipped until records were touched again. Side effects now run on a guaranteed post-commit hook (no polling), with recount → schema recompute → relationship analysis correctly ordered and isolated so one failure no longer cascades.
