# Checkout Sessions

Checkout Session 是 IronixPay 的核心支付对象。每次收款对应一个 Session，系统会为其分配独立的链上收款地址。

## 创建 Session

`POST /v1/checkout/sessions`

### 请求

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

::: tip 法币定价
`pricing_currency` 支持法币（`USD`、`CNY`、`EUR` 等），系统会根据实时汇率自动换算为 `currency` 指定的加密货币金额：

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

### 参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `pricing_amount` | string | ✅ | 金额（人类可读格式），例如 `"10.50"` 表示 10.50。加密货币：最小 1.00，最大 10,000,000.00，精度 0.01。法币：需换算后 ≥ 1 加密货币 |
| `pricing_currency` | string | ✅ | 定价币种。加密货币（`USDT`、`USDC`）= 直接定价，须等于 `currency`。法币（`USD`、`CNY`、`EUR`、`GBP`、`JPY` 等）= 系统自动换算 |
| `currency` | string | ✅ | 链上结算代币：`USDT` 或 `USDC` |
| `network` | string | ✅ | 区块链网络：`TRON`、`SOLANA`、`BSC`、`ETHEREUM`、`POLYGON`、`ARBITRUM`、`OPTIMISM`、`BASE`。详见[支持的网络](/guide/networks) |
| `success_url` | string | ✅ | 支付成功后重定向地址 |
| `cancel_url` | string | ✅ | 过期或取消时重定向地址 |
| `client_reference_id` | string | ❌ | 你的业务订单 ID，方便对账 |

> 完整字段说明见 [API Reference](https://api.ironixpay.com/docs)。
>
> [在 API Reference 中查看 →](https://api.ironixpay.com/docs#tag/checkout/POST/v1/checkout/sessions)

<!-- sync: backend/src/api/dtos/checkout.rs CreateSessionBody -->

### 响应

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

::: details 法币定价的响应示例
当 `pricing_currency` 为法币（如 USD）时，响应中的 `pricing` 对象包含原始定价信息和汇率：

```json
{
  "pricing": {
    "currency": "USD",
    "amount": "10.50",
    "exchange_rate": "1.00050000"
  }
}
```

`exchange_rate` 表示 1 USDT = 1.0005 USD，系统据此计算实际收款金额 `amount_expected`。
:::

## 查询 Session

`GET /v1/checkout/sessions/:id`

查询某个 Session 的当前状态：

```bash
curl https://api.ironixpay.com/v1/checkout/sessions/cs_abc123 \
  -H "Authorization: Bearer sk_test_..."
```

## Session 生命周期

```
┌─────────┐   全额付款    ┌───────────────┐
│ Pending │─────────────▶│ Paid/Overpaid │
└─────────┘              └───────────────┘
     │                          ▲
     │ 部分付款                   │ 追加付款
     ▼                          │
┌───────────┐ ──────────────────┘
│ Underpaid │
└───────────┘
     │
     │ 超时
     ▼
┌─────────┐
│ Expired │
└─────────┘
```

> **AML**：每笔入账在记入之前都会经过 AML 检查。如发送地址命中黑名单，Session 直接进入 `Blocked`。

- `Pending` → `Paid` / `Overpaid`：全额或超额到账，AML 通过
- `Pending` → `Underpaid`：到账金额不足
- `Underpaid` → `Paid` / `Overpaid`：追加到账补齐或超额
- `Underpaid` / `Pending` → `Expired`：超时未付清

### 状态说明

| 状态 | 说明 |
|------|------|
| `Pending` | 已创建，等待付款 |
| `Underpaid` | 到账不足，等待补款或超时 |
| `Paid` | 全额到账（精确匹配） |
| `Overpaid` | 到账金额超过预期 |
| `Expired` | 超时未付清 — 若有部分到账，转入 Resolution Center |
| `Blocked` | AML 命中风险，资金冻结待审 |

## 付款匹配

IronixPay 用 `amount_received` 字段反映实际到账金额：

- **精确匹配**：`amount_received == amount_expected` → `Paid`
- **不足**：`amount_received < amount_expected` → `Underpaid`（活跃时）或 `Expired`（超时后转入 [Resolution Center](https://app.ironixpay.com)）
- **超额**：`amount_received > amount_expected` → `Overpaid`，多出部分可通过 Resolution 退还