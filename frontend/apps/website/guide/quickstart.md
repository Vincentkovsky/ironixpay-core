# 快速开始

三步接入 IronixPay：创建 Session → 引导支付 → 接收 Webhook。

## 1. 创建 Checkout Session

向 API 发送 `POST` 请求：

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

> [在 API Reference 中查看 →](https://api.ironixpay.com/docs#tag/checkout/POST/v1/checkout/sessions)

响应：

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

## 2. 引导客户支付

把用户重定向到响应里的 `url`，页面上会显示收款地址和二维码：

```javascript
// 服务端重定向示例
res.redirect(303, session.url);
```

## 3. 接收 Webhook

链上确认收款后，IronixPay 会向你配置的端点推送 `session.completed` 事件：

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

验证签名后即可完成订单发货。详见 [Webhooks](/guide/webhooks)。

## 金额单位

::: tip
IronixPay 所有 API 的金额字段统一使用**人类可读小数字符串**（如 `"10.50"` = 10.50 USDT），无需任何微单位转换。
:::

## 下一步

::: tip 想把收银台嵌入你的页面？
本教程使用的是**跳转模式**（最简集成）。如果你希望用户留在你的网站内完成支付，可以使用 JavaScript SDK 的**嵌入模式** → [前端集成](/guide/integration)
:::

- [Authentication](/guide/authentication) — API Key 管理
- [Checkout Sessions](/guide/checkout) — 完整 API 说明
- [Payouts](/guide/payouts) — 链上出金 API
- [测试指南](/guide/testing) — 沙盒环境测试
