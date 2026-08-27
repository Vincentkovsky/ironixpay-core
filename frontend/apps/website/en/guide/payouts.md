# Payouts

Send USDT or USDC from your merchant balance to any on-chain address via the Payout API.

## Create a Payout

`POST /v1/payouts`

### Request

```bash
curl -X POST https://api.ironixpay.com/v1/payouts \
  -H "Authorization: Bearer sk_test_..." \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: payout_order_123" \
  -d '{
    "amount": "10",
    "currency": "USDT",
    "network": "TRON",
    "to_address": "TJn9bXhJMVn1Do3PfFHg5J3YNYN9hPQBqA",
    "description": "Affiliate commission for January"
  }'
```

> [View in API Reference →](https://api.ironixpay.com/docs#tag/Payouts/POST/v1/payouts)

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `amount` | string | ✅ | Human-readable decimal amount (e.g., `"10.50"` = 10.50 USDT/USDC) |
| `currency` | string | ✅ | Settlement token: `"USDT"` or `"USDC"` |
| `network` | string | ✅ | Blockchain network: `"TRON"`, `"BSC"`, etc. See [Supported Networks](/en/guide/networks) |
| `to_address` | string | ✅ | Destination on-chain address |
| `description` | string | ❌ | Human-readable description |
| `metadata` | object | ❌ | Custom JSON metadata (max 4KB) |

::: warning Idempotency-Key Required
Every Create Payout request must include an `Idempotency-Key` header to prevent duplicate payouts on network retries. See [Idempotency](/en/guide/idempotency).
:::

<!-- sync: backend/src/api/dtos/payouts.rs CreatePayoutBody -->

### Response

```json
{
  "id": "po_a1b2c3d4e5f6",
  "livemode": false,
  "status": "Pending",
  "amount": "10",
  "fee": "1.5",
  "net_amount": "8.5",
  "currency": "USDT",
  "network": "TRON",
  "to_address": "TJn9bXhJMVn1Do3PfFHg5J3YNYN9hPQBqA",
  "description": "Affiliate commission for January",
  "tx_hash": null,
  "created_at": "2026-02-27T08:30:00+00:00",
  "completed_at": null
}
```

<!-- sync: backend/src/api/dtos/payouts.rs PayoutResponse -->

## Retrieve a Payout

`GET /v1/payouts/:id`

```bash
curl https://api.ironixpay.com/v1/payouts/po_a1b2c3d4e5f6 \
  -H "Authorization: Bearer sk_test_..."
```

## List Payouts

`GET /v1/payouts`

```bash
curl "https://api.ironixpay.com/v1/payouts?page=1&page_size=20" \
  -H "Authorization: Bearer sk_test_..."
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `page` | integer | 1 | Page number |
| `page_size` | integer | 20 | Items per page (max 100) |

## Payout Lifecycle

```
┌─────────┐  Worker picks up  ┌────────────┐  On-chain confirmed  ┌───────────┐
│ Pending │───────────────────▶│ Processing │─────────────────────▶│ Completed │
└─────────┘                    └────────────┘                      └───────────┘
                                    │
                                    │ Broadcast / on-chain failure
                                    ▼
                               ┌────────┐
                               │ Failed │   ← funds returned to balance
                               └────────┘
```

### Status Reference

| Status | Description |
|--------|-------------|
| `Pending` | Created, waiting for background worker |
| `Processing` | Transaction broadcast to chain, awaiting confirmation |
| `Completed` | On-chain confirmation received, payout complete |
| `Failed` | Broadcast or on-chain execution failed; deducted amount returned to merchant balance |

## Fees

A flat fee is deducted from each payout, varying by network:

```
amount = fee + net_amount
```

| Network | Fee |
|---------|-----|
| TRON | 1.5 USDT |
| BSC / Ethereum / Polygon / Arbitrum / Base / Optimism / Solana | 0.5 USDT |

- `amount` must exceed the network fee, otherwise an `amount_too_small` error is returned
- The full `amount` (including fee) is deducted from balance; `net_amount` is sent on-chain

## Important Notes

### Balance Requirement

Payouts are deducted from the merchant's chain account balance. If the balance is insufficient, an `insufficient_balance` error is returned.

### Amount Limits

- **Single payout max**: **10,000 USDT/USDC**
- **Daily limit**: Each merchant has a daily (UTC) cumulative payout limit of **50,000 USDT/USDC**
- Exceeding either limit returns an `invalid_amount` error with the current day's total

### AML Check

All destination addresses undergo an AML risk check before payout. Addresses flagged as high-risk or blacklisted will receive an `invalid_address` error.

### Self-Transfer Protection

The destination address cannot be the platform treasury address. This returns a `self_transfer` error.

### Webhook Notifications

Upon completion or failure, IronixPay sends a `payout.completed` or `payout.failed` webhook event. See [Webhooks](/en/guide/webhooks).
