# Checkout Sessions

Checkout Sessions are the core payment object in IronixPay. Each session represents a single payment intent with a dedicated on-chain address.

## Create a Session

`POST /v1/checkout/sessions`

Creates a new checkout session and returns a payment URL.

### Request

```bash
curl -X POST https://api.ironixpay.com/v1/checkout/sessions \
  -H "Authorization: Bearer sk_test_..." \
  -H "Content-Type: application/json" \
  -d '{
    "pricing_amount": "10.50",
    "pricing_currency": "USDT",
    "currency": "USDT",
    "network": "TRON",
    "client_reference_id": "order_12345",
    "success_url": "https://example.com/success",
    "cancel_url": "https://example.com/cancel"
  }'
```

::: tip Fiat Pricing
`pricing_currency` also accepts fiat codes (`USD`, `CNY`, `EUR`, etc.). The system converts to the settlement token at the real-time exchange rate:

```json
{
  "pricing_amount": "10.50",
  "pricing_currency": "USD",
  "currency": "USDT",
  "network": "TRON",
  "success_url": "https://example.com/success",
  "cancel_url": "https://example.com/cancel"
}
```
:::

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pricing_amount` | string | ✅ | Amount in human-readable format, e.g. `"10.50"`. Crypto: min 1.00, max 10,000,000.00, precision 0.01. Fiat: must convert to ≥ 1 crypto equivalent |
| `pricing_currency` | string | ✅ | Pricing currency. Crypto (`USDT`, `USDC`) = direct pricing, must equal `currency`. Fiat (`USD`, `CNY`, `EUR`, `GBP`, `JPY`, etc.) = auto-converted |
| `currency` | string | ✅ | On-chain settlement token: `USDT` or `USDC` |
| `network` | string | ✅ | Blockchain network: `TRON`, `SOLANA`, `BSC`, `ETHEREUM`, `POLYGON`, `ARBITRUM`, `OPTIMISM`, `BASE`. See [Supported Networks](/en/guide/networks) |
| `success_url` | string | ✅ | Redirect URL after successful payment |
| `cancel_url` | string | ✅ | Redirect URL when session expires or is cancelled |
| `client_reference_id` | string | ❌ | Your internal order ID for reconciliation |

> For full schema details, see [API Reference](https://api.ironixpay.com/docs).
>
> [Open in API Reference →](https://api.ironixpay.com/docs#tag/checkout/POST/v1/checkout/sessions)

<!-- sync: backend/src/api/dtos/checkout.rs CreateSessionBody -->

### Response

```json
{
  "id": "cs_abc123def456",
  "livemode": false,
  "url": "https://checkout.ironixpay.com/cs_abc123def456",
  "status": "Pending",
  "amount_expected": "10.5",
  "amount_received": "0",
  "currency": "USDT",
  "network": "TRON",
  "merchant_name": "Acme Store",
  "pay_address": "TQFEyGNzHZAJmebJUvsoZvJghHm2yNhXAD",
  "client_reference_id": "order_12345",
  "pricing": {
    "currency": "USDT",
    "amount": "10.5",
    "exchange_rate": "1.00000000"
  },
  "success_url": "https://example.com/success",
  "cancel_url": "https://example.com/cancel",
  "expires_at": "2026-02-11T04:26:00Z",
  "created_at": "2026-02-11T03:56:00Z"
}
```

::: details Fiat pricing response example
When `pricing_currency` is fiat (e.g. USD), the `pricing` object contains the original pricing info and exchange rate:

```json
{
  "pricing": {
    "currency": "USD",
    "amount": "10.50",
    "exchange_rate": "1.00050000"
  }
}
```

`exchange_rate` indicates 1 USDT = 1.0005 USD. The system uses this rate to calculate `amount_expected`.
:::

## Retrieve a Session

`GET /v1/checkout/sessions/:id`

Retrieves the current state of an existing checkout session.

```bash
curl https://api.ironixpay.com/v1/checkout/sessions/cs_abc123 \
  -H "Authorization: Bearer sk_test_..."
```

## Session Lifecycle

```
┌─────────┐  full payment  ┌───────────────┐
│ Pending │───────────────▶│ Paid/Overpaid │
└─────────┘                └───────────────┘
     │                            ▲
     │ partial                    │ additional payment
     ▼                            │
┌───────────┐ ────────────────────┘
│ Underpaid │
└───────────┘
     │
     │ timeout
     ▼
┌─────────┐
│ Expired │
└─────────┘
```

> **AML**: Any incoming payment may be blocked before it is applied. If the sender address fails the AML check, the session moves directly to `Blocked`.

- `Pending` → `Paid` / `Overpaid`: Full or excess payment, AML passed
- `Pending` → `Underpaid`: Partial payment received
- `Underpaid` → `Paid` / `Overpaid`: Additional payment fills or exceeds the gap
- `Underpaid` / `Pending` → `Expired`: Session times out

### Session Status

| Status | Description |
|--------|-------------|
| `Pending` | Session created, awaiting payment |
| `Underpaid` | Partial payment received, waiting for remaining amount or expiry |
| `Paid` | Full payment received on-chain (exact match) |
| `Overpaid` | Payment received exceeds the expected amount |
| `Expired` | Session timed out — if partially paid, routed to Resolution Center |
| `Blocked` | AML risk detected, funds held for review |

## Payment Matching

IronixPay uses the `amount_received` field (in the response) to indicate how much was actually paid:

- **Exact payment**: `amount_received == amount_expected` → status is `Paid`
- **Underpaid**: `amount_received < amount_expected` → status is `Underpaid` (if still active) or `Expired` (if timed out, routed to [Resolution Center](https://app.ironixpay.com))
- **Overpaid**: `amount_received > amount_expected` → status is `Overpaid`. Excess can be refunded via Resolution