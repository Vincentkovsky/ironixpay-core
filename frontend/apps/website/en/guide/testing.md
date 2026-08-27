# Testing

IronixPay provides a sandbox environment on the TRON Nile testnet for risk-free development and testing.

## Sandbox vs Production

| | Sandbox | Production |
|---|---------|------------|
| **API Endpoint** | `https://sandbox.ironixpay.com` | `https://api.ironixpay.com` |
| **API Key Prefix** | `sk_test_` | `sk_live_` |
| **Networks** | TRON Nile (testnet only) | TRON, Solana, BSC, ETH, Polygon, Arbitrum, Optimism, Base |
| **Tokens** | Test USDT (no real value) | Real USDT / USDC |
| **Behavior** | Identical to production | — |
| **Data Isolation** | Completely separate | Completely separate |

## Getting Test Tokens

To make test payments, you need TRON Nile testnet USDT:

**Get test TRX and test USDT** from the [Nile Faucet](https://nileex.io/join/getJoinPage)

::: tip
You need a small amount of TRX for transaction fees, even on testnet. Get TRX from the faucet first.
:::

## Testing a Complete Payment Flow

### Step 1: Create a test session

```bash
curl -X POST https://api.ironixpay.com/v1/checkout/sessions \
  -H "Authorization: Bearer sk_test_..." \
  -H "Content-Type: application/json" \
  -d '{
    "pricing_amount": "1",
    "pricing_currency": "USDT",
    "currency": "USDT",
    "network": "TRON",
    "success_url": "https://example.com/success",
    "cancel_url": "https://example.com/cancel"
  }'
```

### Step 2: Send USDT to the payment address

Use TronLink or any TRON wallet to send 1 USDT to the `pay_address` returned in the response.

### Step 3: Wait for confirmation

IronixPay monitors the blockchain and will:
1. Detect the incoming transaction
2. Update the session status to `Completed`
3. Send a `session.completed` webhook to your endpoint

### Step 4: Verify the webhook

Check your webhook endpoint received the event and the signature is valid.

## Testing Webhooks Locally

Use a tunneling service to expose your local server:

```bash
# Using ngrok
ngrok http 3000

# Set your webhook URL to the ngrok URL
# https://abc123.ngrok.io/webhooks/ironixpay
```

## Testing Edge Cases

| Scenario | How to Test |
|----------|-------------|
| **Underpayment** | Send less than the session amount |
| **Overpayment** | Send more than the session amount |
| **Expiration** | Create a session and wait for it to expire |
| **Multiple payments** | Send multiple transactions to the same address |

## Limitations

- Sandbox and production data are completely isolated
- Test API keys cannot access production sessions
- Webhook endpoints are configured separately per environment
