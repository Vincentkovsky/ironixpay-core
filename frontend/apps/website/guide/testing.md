# 测试指南

IronixPay 提供基于 TRON Nile 测试网的沙盒环境，可以零成本跑完整支付流程。

## 沙盒 vs 生产

| | 沙盒 | 生产 |
|---|------|------|
| **API 节点** | `https://sandbox.ironixpay.com` | `https://api.ironixpay.com` |
| **API Key** | `sk_test_` | `sk_live_` |
| **网络** | 仅 TRON Nile 测试网 | TRON、Solana、BSC、ETH、Polygon、Arbitrum、Optimism、Base |
| **代币** | 测试 USDT（无价值） | 真实 USDT / USDC |
| **行为** | 和生产完全一致 | — |
| **数据** | 完全隔离 | 完全隔离 |

## 获取测试代币

测试支付需要 Nile 测试网上的 USDT：
**领取测试 TRX 和 USDT** — [Nile Faucet](https://nileex.io/join/getJoinPage)

::: tip
测试网交易也需要少量 TRX 作为 gas，记得先从 faucet 领 TRX。
:::

## 完整支付流程

### Step 1：创建 Test Session

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

### Step 2：向收款地址转账

用 TronLink 或任意 TRON 钱包，向响应中 `pay_address` 转入 1 USDT。

### Step 3：等待确认

IronixPay 自动监链，依次完成：
1. 检测到入账交易
2. Session 状态更新为 `Paid`
3. 向你的端点推送 `session.completed` Webhook

### Step 4：验证 Webhook

确认端点收到了事件，且签名验证通过。

## 本地调试 Webhook

用 tunnel 工具把本地服务暴露到公网：

```bash
# ngrok 示例
ngrok http 3000

# 把 Webhook URL 设为 ngrok 地址
# https://abc123.ngrok.io/webhooks/ironixpay
```

## 测试边界场景

| 场景 | 做法 |
|------|------|
| **少付** | 转入少于 Session 金额的 USDT |
| **多付** | 转入超过 Session 金额的 USDT |
| **超时** | 创建 Session 后等它过期 |
| **多笔付款** | 向同一地址发送多笔交易 |

## 注意

- 沙盒和生产的数据完全隔离
- Test key 无法操作 Live 环境的 Session
- Webhook 端点按环境独立配置
