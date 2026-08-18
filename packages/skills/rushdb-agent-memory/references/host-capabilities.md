# Native Host Capabilities

## OpenClaw

Install `@rushdb/openclaw-memory` for additive native memory.

The plugin:

- recalls before prompt construction and supplements the native memory corpus;
- captures the latest successful user/assistant pair at `agent_end`;
- writes through a durable local outbox;
- authorizes direct/private conversations from host session metadata;
- fails closed for sandboxed, group, channel, cron, hook, and subagent contexts;
- leaves OpenClaw Markdown and SQLite memory active.

The plugin currently projects completed turns to `EPISODE`; it does not expose a model-callable canonical fact writer. For "remember this," use OpenClaw's native local memory path. Use MCP for intentional domain records such as `DECISION` or `PREFERENCE`, but do not fabricate canonical scope fields for `MEMORY_FACT`.

## Hermes Agent

Install `rushdb-hermes-memory` and select the `rushdb` provider through `hermes memory setup`.

The provider:

- prefetches and recalls before inference;
- projects completed primary-agent turns to `EPISODE`;
- projects supported native memory add/replace events to `MEMORY_FACT`;
- handles pre-compression, session switch/end, and shutdown;
- suppresses subagent/non-primary automatic writes;
- uses a profile-local durable outbox under `HERMES_HOME`.

Use Hermes' native memory-write facility for explicit facts so the provider can attach trusted profile and participant scope. MCP remains useful for deliberate graph operations beyond the provider lifecycle.

## Capability Rule

Do not assume hosts expose identical hooks. Detect the actual lifecycle and explicit-write surfaces, then route each operation to the layer that can supply trusted metadata and durability.
