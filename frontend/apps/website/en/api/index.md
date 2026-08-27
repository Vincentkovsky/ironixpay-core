# API Reference

IronixPay provides RESTful APIs for accepting and sending USDT & USDC payments across multiple chains.

## Base URLs

| Environment | Base URL |
|-------------|----------|
| **Sandbox (Testnet)** | `https://sandbox.ironixpay.com` |
| **Production (Mainnet)** | `https://api.ironixpay.com` |

## Authentication

All API requests require an API key in the `Authorization` header:

```bash
curl -H "Authorization: Bearer $IRONIXPAY_SECRET_KEY" \
  https://sandbox.ironixpay.com/v1/checkout/sessions
```

- Sandbox API keys start with `sk_test_`
- Production API keys start with `sk_live_`

Manage your API keys in the [Dashboard](https://app.ironixpay.com) under "API Keys".

## Endpoints

### Checkout Sessions
Create payment sessions to accept USDT/USDC from your customers.

### Payouts
Send USDT/USDC from your merchant balance to any on-chain address.

### Sub-Merchants
Manage sub-merchants for PSP and marketplace use cases.

---

> 💡 You can also test the API interactively using our [Scalar API Playground](https://api.ironixpay.com/docs).
