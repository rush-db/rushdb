# Agent Memory Integration Modes

## Decision Sequence

1. Detect whether the host has a native RushDB memory integration.
2. Detect whether RushDB MCP tools are available.
3. Detect whether application code owns the agent lifecycle.
4. Select exactly one owner for automatic recall and completed-turn persistence.

| Native | MCP      | Application lifecycle | Mode                                                     |
| ------ | -------- | --------------------- | -------------------------------------------------------- |
| Yes    | Any      | Any                   | Native; add MCP only for explicit graph operations       |
| No     | Yes      | No                    | MCP only                                                 |
| No     | Optional | Yes                   | Custom harness                                           |
| No     | No       | No                    | Configure an integration before promising durable memory |

## Native Runtime

Use native memory when the runtime exposes hooks before inference, after a completed turn, and at session or process boundaries. The native adapter must own automatic `EPISODE` capture.

Do not create one MCP record per turn. Do not run the MCP-only session bootstrap unless the user separately wants graph-visible session records.

## Native + MCP

Use this combination for two complementary planes:

- **Lifecycle plane:** native recall, bounded episode capture, compaction/session handling, retries.
- **Knowledge plane:** explicit domain records, graph traversal, schema discovery, audits, and administrative operations through MCP.

Before an MCP write, ask whether the native adapter already writes the same information. If yes, skip the duplicate. Store an explicit domain record only when it adds structured meaning beyond the captured episode.

## MCP Only

Require `@rushdb/mcp-server` and `RUSHDB_API_KEY`. Call `getSchemaMarkdown` before other queries. Use explicit records and relationships; the model controls when memory is recalled or written.

MCP-only behavior is not automatic lifecycle memory. It cannot guarantee recall before inference or persistence after every successful turn unless the surrounding host invokes it.

## Custom Harness

Use `@rushdb/agent-memory-contract` as the protocol boundary and a RushDB SDK as transport. Map host lifecycle hooks to recall, episode/fact construction, durable enqueue, flush, and shutdown.

Use the `rushdb-memory-adapter` skill when implementing or reviewing this mode.

## Failure Behavior

- Continue the host turn if remote recall times out or fails.
- Durably enqueue acknowledged background writes before returning from the capture hook.
- Replay the outbox after restart with deterministic event identity.
- Keep automatic memory additive to local host memory unless the host explicitly supports exclusive providers.
