# Idempotency

Send an `Idempotency-Key` header on `POST` requests to safely retry without creating duplicates:

```http
Idempotency-Key: order-123-attempt-1
```

## How It Works

- **Same key + same body** → Returns the cached response from the original request
- **Same key + different body** → Returns `409 Conflict` with error code `idempotency_conflict`
- **Keys expire after 24 hours**

## Example

```bash
# First request — creates the session
curl -X POST https://api.ironixpay.com/v1/checkout/sessions \
  -H "Authorization: Bearer sk_test_..." \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: order_12345_v1" \
  -d '{"pricing_amount": "10.50", "pricing_currency": "USDT", "currency": "USDT", "network": "TRON", "success_url": "https://example.com/success", "cancel_url": "https://example.com/cancel"}'

# Retry with same key — returns the same session (no duplicate created)
curl -X POST https://api.ironixpay.com/v1/checkout/sessions \
  -H "Authorization: Bearer sk_test_..." \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: order_12345_v1" \
  -d '{"pricing_amount": "10.50", "pricing_currency": "USDT", "currency": "USDT", "network": "TRON", "success_url": "https://example.com/success", "cancel_url": "https://example.com/cancel"}'
```

## Best Practices

1. **Use meaningful keys** — Include your order ID and a version/attempt suffix (e.g., `order_12345_v1`)
2. **Don't reuse keys across different operations** — Each unique operation should have its own key
3. **Handle 409 errors** — If you get an `idempotency_conflict`, it means you're reusing a key with a different request body

> [View Create Session endpoint in API Reference →](https://api.ironixpay.com/docs#tag/checkout/POST/v1/checkout/sessions)
