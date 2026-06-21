---
title: Billing Model
description: How RushDB Cloud bills for Knowledge Units, applies plan limits, and treats self-hosted deployments.
sidebar_position: 1
---

# Billing Model

## Overview

RushDB Cloud uses a **Knowledge Units (KU)** billing model. You pay for the structured knowledge RushDB creates and maintains from your data — not for infrastructure, nodes, edges, storage, or compute.

```
Total KU consumed in billing period × price per KU = your bill
```

This model aligns cost with value: you pay more only when RushDB is doing more work to organize and serve your data.

## Monthly Billing Cycle

KU consumption resets at the start of each billing period. Your plan includes a base KU allowance. Consumption above the included allowance is billed as overage (Pro and above).

```
Bill = included KU (flat rate) + overage KU × overage rate
```

## Plan Comparison

|                         | Free          | Start         | Pro       | Scale       | Enterprise |
| ----------------------- | ------------- | ------------- | --------- | ----------- | ---------- |
| Included KU / month     | 100,000       | 250,000       | 1,000,000 | Usage-based | Custom     |
| Overage                 | Not available | Not available | Per KU    | Per KU      | Negotiated |
| Projects                | 2             | 2             | 10        | 100         | Custom     |
| Team members            | 1             | 1             | 3         | 10          | Custom     |
| Self-hosted support     | ✓             | ✓             | ✓         | ✓           | ✓          |
| SLA                     | —             | —             | —         | ✓           | ✓          |
| BYOC (own Neo4j / Aura) | ✓             | ✓             | ✓         | ✓           | ✓          |

## One Metric

KU is the only number that matters for billing. RAM, CPU, storage, node counts, edge counts, and query time are infrastructure concerns — RushDB absorbs them. Your bill reflects the knowledge work done, nothing else.

## Soft and Hard Limits

- **Soft limit**: RushDB sends an alert when you reach 80% of your monthly KU allowance.
- **Hard limit**: Write operations are blocked when your allowance is exhausted. Read operations continue unaffected.
- You can configure alerts and caps from the **KU Usage** page in your dashboard.

## Self-Hosted Billing

Self-hosted deployments (`RUSHDB_SELF_HOSTED=true`) have no RushDB Cloud KU limits and no cloud billing. The self-hosted platform runs without quota enforcement.

## Changes to Limits Mid-Period

- Upgrading your plan: new KU limit takes effect immediately.
- Downgrading your plan: new KU limit takes effect at the start of the next billing period.
- Cancelling: your current allowance remains active until the end of the paid period.

## KU Usage Dashboard

Your KU consumption is visible in real-time on the **KU Usage** page in your workspace dashboard:

- Total KU consumed this month
- Remaining KU
- Estimated end-of-month cost
- Daily consumption trend chart
- Ingestion and query spike timeline

## Related Pages

- [Knowledge Units (KU)](/rushdb-cloud/knowledge-units)
- [Multitenancy & Project Isolation](/rushdb-cloud/project-isolation)
- [Licensing & Third-Party Services](/rushdb-cloud/licensing-and-services)
