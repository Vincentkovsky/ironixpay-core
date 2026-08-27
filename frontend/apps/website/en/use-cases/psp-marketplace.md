---
title: PSP & Marketplace — Sub-Merchant Mode — IronixPay
description: One API key to manage multiple sub-merchants with isolated accounting, independent address pools, and per-merchant reporting. Built for PSPs, marketplaces, and SaaS platforms.
---

# PSP & Marketplace — Sub-Merchant Mode

One merchant account, multiple sub-merchants — every transaction attributed automatically, reconciliation built in, zero data leakage between tenants.

## The Problem

Payment aggregators (PSPs) and multi-vendor marketplaces face **multi-tenant management challenges**:

- **Reconciliation chaos** — All incoming payments land in a single account with no per-merchant breakdown
- **Registration overhead** — Each downstream merchant needs their own signup, API keys, and onboarding — operational costs scale linearly
- **Poor data isolation** — Sub-merchants may access each other's transaction data
- **High build cost** — Self-building multi-tenancy means handling address allocation, fee splitting, and webhook routing
- **Compliance reporting** — Impossible to export transaction history per sub-merchant for audits

## How IronixPay Solves This

| Challenge | IronixPay Solution |
|---|---|
| Reconciliation chaos | **Auto-attribution** — Every transaction is tagged with `sub_merchant_code`; filter in Dashboard and API |
| Registration overhead | **One API key for all** — Switch context via `X-Sub-Merchant-Code` header; no extra credentials |
| Data isolation | **Independent address pools** — Each sub-merchant gets its own HD-derived addresses, isolated on-chain and in the database |
| Build cost | **Out of the box** — CRUD API to create sub-merchants; integrate in a few lines of code |
| Compliance reporting | **Per-merchant export** — Billing page supports sub-merchant filtering with CSV export |

## Typical Usage

### Platform Integration Flow

<FlowChart>
  <Step icon="api" title="PSP registers an IronixPay merchant account" />
  <Step icon="chain" title="Create sub-merchants via API">POST /v1/sub-merchants</Step>
  <Step icon="order" title="Create Checkout Session">Header: X-Sub-Merchant-Code</Step>
  <Step icon="webhook" title="Customer pays → Webhook fires">Payload includes sub_merchant_code</Step>
  <Step icon="check" title="PSP reconciles by sub_merchant_code" />
</FlowChart>

## Key Features

- **Sub-Merchant CRUD** — `POST / GET / PATCH /v1/sub-merchants` to create, list, and update sub-merchants with `active` / `suspended` status management
- **Context switching** — Set `X-Sub-Merchant-Code` header to route transactions — no extra API keys needed
- **Independent HD addresses** — Each sub-merchant gets its own HD-derived address pool across all 8 supported chains
- **Mixed-view Dashboard** — Filter by "All", "Self only", or a specific sub-merchant in Sessions, Billing, Payouts, and Resolution pages
- **Unified billing** — Fees are deducted from the parent PSP's balance; sub-merchants don't need independent balances
- **Webhook attribution** — `session.completed` and other events automatically include the `sub_merchant_code` field
- **CSV export** — Sessions and Billing pages support per-sub-merchant filtering and CSV export for reconciliation and compliance

## Platform Types That Benefit Most

| Platform Type | Why Sub-Merchant Mode Helps |
|---|---|
| **Payment Aggregators (PSP)** | Single API integration; downstream merchants are unaware of the gateway; centralized management |
| **E-commerce Marketplaces** | One sub-merchant per seller; payments auto-attributed; platform handles settlement |
| **SaaS Collection** | Collect payments on behalf of clients; segment by `sub_merchant_code` for clear reporting |
| **Agent Networks** | Agents manage multiple terminal merchants with hierarchical views and exports |

## API Quick Reference

### Create a Sub-Merchant

```bash
curl -X POST https://api.ironixpay.com/v1/sub-merchants \
  -H "Authorization: Bearer $IRONIXPAY_SECRET_KEY" \
  -H "Content-Type: application/json" \
  -d '{"sub_merchant_code": "shop_tokyo", "display_name": "Tokyo Branch"}'
```

### Create a Payment for a Sub-Merchant

```bash
curl -X POST https://api.ironixpay.com/v1/checkout/sessions \
  -H "Authorization: Bearer $IRONIXPAY_SECRET_KEY" \
  -H "X-Sub-Merchant-Code: shop_tokyo" \
  -H "Content-Type: application/json" \
  -d '{
    "pricing_amount": "50",
    "pricing_currency": "USDT",
    "currency": "USDT",
    "network": "TRON",
    "success_url": "https://example.com/success",
    "cancel_url": "https://example.com/cancel"
  }'
```

### List Sub-Merchants

```bash
curl https://api.ironixpay.com/v1/sub-merchants \
  -H "Authorization: Bearer $IRONIXPAY_SECRET_KEY"
```

## Get Started

- [Quick Start](/en/guide/quickstart) — Create your account and get API keys
- [Checkout Sessions](/en/guide/checkout) — Understand the payment session lifecycle
- [Webhooks](/en/guide/webhooks) — Automate payment notification and reconciliation
- [API Reference](https://api.ironixpay.com/docs) — Full sub-merchant API documentation
- [Testing Guide](/en/guide/testing) — Test the sub-merchant flow on Sandbox
