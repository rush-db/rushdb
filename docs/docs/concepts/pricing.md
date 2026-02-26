---
sidebar_position: 9
---

# Pricing

## Simple, Predictable Pricing

RushDB pricing is based on [Knowledge Units (KU)](./knowledge-units.md) — a single unit that represents the structured knowledge created and maintained from your data. No infrastructure tiers, no node counts, no storage pricing.

```
You pay for knowledge created. Nothing else.
```

## Plans

### Free

- **100,000 KU / month** included
- Up to 2 projects
- Self-hosted support
- Bring Your Own Cloud (BYOC) — connect to your own Neo4j or Aura instance
- Community support
- No credit card required

Perfect for prototypes, side projects, and getting started.

### Pro — $29/month

- **10,000,000 KU / month** included
- Overage at **$3 per additional million KU** — no hard stop, apps keep running
- Unlimited projects
- Priority support
- Team members (up to 3, then $10/member)
- Bring Your Own Cloud (BYOC) — connect to your own Neo4j or Aura instance

Ideal for production applications and growing teams. Predictable base cost, pay-as-you-go beyond the included allowance.

### Scale — from $99/month

- **Usage-based** — $99 platform fee + **$2 per million KU** consumed
- No included KU bundle — cheaper per-KU rate than Pro at volume
- SLA guarantee
- Advanced support
- Unlimited team members
- Bring Your Own Cloud (BYOC) — connect to your own Neo4j or Aura instance

For high-volume or highly variable workloads where you want the lowest per-KU rate without worrying about tiers. The $2/M KU rate on Scale is 33% cheaper than Pro's overage rate.

### Enterprise

- **Platform license** — flat fee, unlimited KU
- Bring Your Own Cloud (BYOC)
- Embedded / OEM use
- Dedicated support and SLA
- Custom contract

For organisations embedding RushDB into their products or needing full data sovereignty.

## Estimating Your KU Usage

Use this formula to estimate your monthly KU consumption:

```
estimated KU ≈ records_per_day × 30 × avg_fields_per_record × nesting_factor
```

**Example:**
- 1,000 records/day
- 10 fields per record on average
- Flat structure (nesting factor ≈ 1.0)

```
1,000 × 30 × 10 × 1.0 = 300,000 KU/month → Pro plan
```

The interactive KU Calculator on the [pricing page](https://rushdb.com/pricing) can help you get a more precise estimate.

## Self-Hosted

Running RushDB on your own infrastructure? Self-hosted mode is **free and unlimited** — no KU limits, no billing. See the [self-hosting guide](../get-started/quick-tutorial) to get started.

## FAQ

**Can I exceed my plan's KU limit?**
On the Free plan, writes are blocked when the limit is reached — reads always continue. On Pro, overage is billed at $3 per million KU beyond the 10M included. On Scale there is no hard limit — you pay $2 per million KU consumed on top of the $99/month base.

**Does deleting data reduce my KU usage?**
KU from creation operations is never reversed. However, once data is deleted, its ongoing stored footprint stops contributing to KU from that point forward.

**Do read operations consume KU?**
Standard read and search operations do not consume KU. Heavy analytical operations (multi-hop traversals, vector similarity search at scale) may consume a small amount of KU.

**Is there a free trial for paid plans?**
Yes — start on the Free plan with no credit card. Upgrade at any time and your remaining free KU carries over for the rest of the billing period.
