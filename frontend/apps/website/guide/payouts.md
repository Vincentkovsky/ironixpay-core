# Payouts

通过 Payout API 将商户余额中的 USDT 或 USDC 发送至任意链上地址。

## 创建 Payout

`POST /v1/payouts`

### 请求

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

> [在 API Reference 中查看 →](https://api.ironixpay.com/docs#tag/Payouts/POST/v1/payouts)

### 参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `amount` | string | ✅ | 人类可读小数金额（如 `"10.50"` = 10.50 USDT/USDC） |
| `currency` | string | ✅ | 结算代币：`"USDT"` 或 `"USDC"` |
| `network` | string | ✅ | 区块链网络：`"TRON"`、`"BSC"` 等。详见[支持的网络](/guide/networks) |
| `to_address` | string | ✅ | 目标链上地址 |
| `description` | string | ❌ | 可读描述（如"1月分佣"） |
| `metadata` | object | ❌ | 自定义 JSON 元数据（最大 4KB） |

::: warning 必须提供 Idempotency-Key
每次创建 Payout 请求需带 `Idempotency-Key` Header，防止网络重试导致重复出金。详见[幂等性](/guide/idempotency)。
:::

<!-- sync: backend/src/api/dtos/payouts.rs CreatePayoutBody -->

### 响应

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

## 查询 Payout

`GET /v1/payouts/:id`

```bash
curl https://api.ironixpay.com/v1/payouts/po_a1b2c3d4e5f6 \
  -H "Authorization: Bearer sk_test_..."
```

## 列表 Payouts

`GET /v1/payouts`

```bash
curl "https://api.ironixpay.com/v1/payouts?page=1&page_size=20" \
  -H "Authorization: Bearer sk_test_..."
```

| 参数 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `page` | integer | 1 | 页码 |
| `page_size` | integer | 20 | 每页条数（最大 100） |

## Payout 生命周期

```
┌─────────┐  Worker 拣取   ┌────────────┐  链上确认   ┌───────────┐
│ Pending │───────────────▶│ Processing │───────────▶│ Completed │
└─────────┘                └────────────┘            └───────────┘
                                │
                                │ 广播失败 / 链上失败
                                ▼
                           ┌────────┐
                           │ Failed │   ← 余额自动原路退回
                           └────────┘
```

### 状态说明

| 状态 | 说明 |
|------|------|
| `Pending` | 已创建，等待后台 Worker 处理 |
| `Processing` | 交易已广播至链上，等待确认 |
| `Completed` | 链上确认成功，出金完成 |
| `Failed` | 广播或链上执行失败，扣款金额已退回商户余额 |

## 费用

每笔 Payout 收取固定出金手续费，按网络不同：

```
amount = fee + net_amount
```

| 网络 | 手续费 |
|------|--------|
| TRON | 1.5 USDT |
| BSC / Ethereum / Polygon / Arbitrum / Base / Optimism / Solana | 0.5 USDT |

- `amount` 必须大于对应网络的手续费，否则返回 `amount_too_small` 错误
- 余额扣除 `amount`（含手续费），链上实际发送 `net_amount`

## 注意事项

### 余额要求

出金从商户的链上账户余额扣除。如余额不足，返回 `insufficient_balance` 错误。

### 金额限制

- **单笔上限**：**10,000 USDT/USDC**
- **每日上限**：每个商户每日（UTC）出金累计上限 **50,000 USDT/USDC**
- 超出限额时返回 `invalid_amount` 错误并提示当日已用额度

### AML 检查

所有目标地址在出金前会通过 AML 风控检查。如果地址命中黑名单或被风控服务标记为高风险，返回 `invalid_address` 错误。

### 自转账保护

目标地址不能是平台 Treasury 地址，否则返回 `self_transfer` 错误。

### Webhook 通知

出金完成或失败后，系统会推送 `payout.completed` 或 `payout.failed` 事件。详见 [Webhooks](/guide/webhooks)。
