# Idempotency

在 `POST` 请求头加上 `Idempotency-Key`，即可安全重试而不会重复创建资源：

```http
Idempotency-Key: order-123-attempt-1
```

## 行为规则

- **相同 Key + 相同请求体** → 返回首次请求的缓存结果
- **相同 Key + 不同请求体** → `409 Conflict`（`idempotency_conflict`）
- **Key 24 小时后过期**

## 示例

```bash
# 首次请求 — 创建 Session
curl -X POST https://api.ironixpay.com/v1/checkout/sessions \
  -H "Authorization: Bearer sk_test_..." \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: order_12345_v1" \
  -d '{"pricing_amount": "10.50", "pricing_currency": "USDT", "currency": "USDT", "network": "TRON", "success_url": "https://example.com/success", "cancel_url": "https://example.com/cancel"}'

# 同一 Key 重试 — 返回相同 Session（不会重复创建）
curl -X POST https://api.ironixpay.com/v1/checkout/sessions \
  -H "Authorization: Bearer sk_test_..." \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: order_12345_v1" \
  -d '{"pricing_amount": "10.50", "pricing_currency": "USDT", "currency": "USDT", "network": "TRON", "success_url": "https://example.com/success", "cancel_url": "https://example.com/cancel"}'
```

## 最佳实践

1. **用有意义的 Key** — 建议包含订单 ID + 版本后缀（如 `order_12345_v1`）
2. **不要跨操作复用** — 每个独立操作用自己的 Key
3. **处理 409** — 收到 `idempotency_conflict` 说明你用了相同 Key 但请求体变了

> [在 API Reference 中查看创建 Session 端点 →](https://api.ironixpay.com/docs#tag/checkout/POST/v1/checkout/sessions)
