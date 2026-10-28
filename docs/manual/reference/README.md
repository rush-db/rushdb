# Reference

Terse lookup tables for RushDB: every setting, command, endpoint, error
code, metric, and limit. Task-oriented explanations live in
[Operations](../operations/README.md).

## Pages

1. [Configuration reference](configuration-reference.md) — every `rushdb serve`
   flag, environment variable, and `config.yml` key with type and default,
   plus startup validation rules.
2. [CLI reference](cli-reference.md) — the `rushdb` commands (`serve`, `up`,
   `health`), maintenance commands, and exit codes.
3. [HTTP endpoints](http-endpoints.md) — HTTP routes, the Protocol v1
   envelope, and every operation with its required permission.
4. [Error codes](error-codes.md) — the protocol error taxonomy and its HTTP,
   PostgreSQL SQLSTATE, gRPC, and Bolt mappings.
5. [Metrics](metrics.md) — every Prometheus metric with type, labels, and
   meaning.
6. [Limits](limits.md) — size caps, budgets, lifetimes, and background
   intervals.
7. [Glossary](glossary.md) — terms used in this manual.

## See also

- [Operations](../operations/README.md)
- [Connectivity](../connectivity/README.md)
