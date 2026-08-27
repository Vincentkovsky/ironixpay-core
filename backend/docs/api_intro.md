# IronixPay API

Welcome to the **IronixPay API**. Build crypto payment integrations with our REST API.

## Base URL
- **Production**: `https://api.ironixpay.com`
- **Sandbox**: `https://sandbox.ironixpay.com`

## Authentication
Include your API key in the `Authorization` header:
```
Authorization: Bearer sk_live_your_api_key
```

## Supported Tokens & Networks

| Token | Networks |
|-------|----------|
| USDT  | TRON, BSC, Ethereum, Polygon, Arbitrum, Base, Optimism |
| USDC  | BSC, Ethereum, Polygon, Arbitrum, Base, Optimism |

> **Sandbox**: TRON Nile testnet (USDT only).

## Amount Convention

All amounts across the API are represented as **human-readable decimal strings**.

| Example | Meaning |
|---------|---------|
| `"10"` | 10 USDT/USDC |
| `"10.5"` | 10.5 USDT/USDC |
| `"0.01"` | Minimum precision (0.01) |

- **Minimum**: `0.01`
- **Maximum**: `10,000,000`

## Webhooks
Configure a webhook URL to receive real-time notifications:
- `checkout_session.paid` — Payment confirmed
- `checkout_session.expired` — Session expired
- `checkout_session.underpaid` — Partial payment detected
- `checkout_session.overpaid` — Excess payment detected
- `payout.completed` — Payout confirmed on-chain
- `payout.failed` — Payout failed

All webhook payloads include an `X-Webhook-Signature` header for verification.
