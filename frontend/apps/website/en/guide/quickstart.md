# Quick Start

Get started with IronixPay in 3 steps: create a session, redirect your customer, and handle the webhook.

## 1. Create a Checkout Session

Make a `POST` request to create a new payment session:

```bash
curl -X POST https://api.ironixpay.com/v1/checkout/sessions \
  -H "Authorization: Bearer sk_test_..." \
  -H "Content-Type: application/json" \
  -d '{
    "pricing_amount": "10.50",
    "pricing_currency": "USDT",
    "currency": "USDT",
    "network": "TRON",
    "success_url": "https://example.com/success",
    "cancel_url": "https://example.com/cancel"
  }'
```

> [View in API Reference →](https://api.ironixpay.com/docs#tag/checkout/POST/v1/checkout/sessions)

Response:

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

## 2. Redirect Your Customer

Send the customer to the returned `url`. They will see a checkout page with the payment address and QR code.

```javascript
// Example: redirect in your server-side handler
res.redirect(303, session.url);
```

## 3. Handle the Webhook

When payment is received on-chain, IronixPay sends a `session.completed` webhook to your configured endpoint:

```json
{
  "id": "evt_abc123...",
  "event_type": "session.completed",
  "created": 1739246160,
  "data": {
    "session_id": "cs_abc123def456",
    "livemode": false,
    "status": "Paid",
    "amount_expected": "10.5",
    "amount_received": "10.5",
    "currency": "USDT",
    "pay_address": "TQFEyGNzHZAJmebJUvsoZvJghHm2yNhXAD",
    "tx_count": 1
  }
}
```

Verify the signature and fulfill the order. See [Webhooks](/en/guide/webhooks) for details.

## Amount Convention

::: tip
All amount fields across IronixPay APIs use **human-readable decimal strings** (e.g. `"10.50"` = 10.50 USDT). No microunit conversion needed.
:::

## Next Steps

::: tip Want to embed the checkout in your page?
This guide uses **Redirect Mode** (the simplest approach). If you'd rather keep customers on your site, check out **Embed Mode** with our JavaScript SDK → [Frontend Integration](/en/guide/integration)
:::

- [Authentication](/en/guide/authentication) — API key management
- [Checkout Sessions](/en/guide/checkout) — Full API details
- [Payouts](/en/guide/payouts) — On-chain payout API
- [Testing](/en/guide/testing) — Sandbox testing guide
