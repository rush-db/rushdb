---
'rushdb-core': patch
'rushdb-docs': patch
---

Switch default chat model to `openai/gpt-5.6-luna`

The default model behind AI search query generation and relationship suggestions (`RUSHDB_LLM_MODEL`) is now `openai/gpt-5.6-luna` across all deploy templates, config examples, and docs — replacing `gemini-2.5-flash-lite` / `gpt-4.1-mini` (the latter is deprecated by OpenAI). The new model responds in about a second and improves structured-output quality, making dashboard AI search snappier and suggestions more reliable. Self-hosted deployments keep full control via `RUSHDB_LLM_MODEL` and can use any OpenAI-compatible provider as before.
