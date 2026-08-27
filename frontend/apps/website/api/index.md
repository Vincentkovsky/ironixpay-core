# API 参考

IronixPay 提供 RESTful API，支持 USDT 和 USDC 在多条链上的收款与出款。

## 基础信息

| 环境 | Base URL |
|------|----------|
| **Sandbox（测试网）** | `https://sandbox.ironixpay.com` |
| **Production（主网）** | `https://api.ironixpay.com` |

## 认证

所有 API 请求需要在 `Authorization` 请求头中携带 API Key：

```bash
curl -H "Authorization: Bearer $IRONIXPAY_SECRET_KEY" \
  https://sandbox.ironixpay.com/v1/checkout/sessions
```

- Sandbox API Key 以 `sk_test_` 开头
- Production API Key 以 `sk_live_` 开头

在 [控制台](https://app.ironixpay.com) 的「API Keys」页面管理你的密钥。

## API 端点

### 收银台会话 Checkout Sessions

创建支付会话，让你的客户通过 USDT/USDC 完成付款。

| 端点 | 说明 |
|------|------|
| [`POST /v1/checkout/sessions`](/api/operations/create_session) | 创建收银台会话 |
| [`GET /v1/checkout/sessions/:id`](/api/operations/get_session) | 获取收银台会话 |
| [`GET /v1/checkout/sessions`](/api/operations/list_sessions) | 列出收银台会话 |

### 出款 Payouts

从商户余额向任意链上地址发送 USDT/USDC。

| 端点 | 说明 |
|------|------|
| [`POST /v1/payouts`](/api/operations/create_payout) | 创建出款 |
| [`GET /v1/payouts/:id`](/api/operations/get_payout) | 获取出款详情 |
| [`GET /v1/payouts`](/api/operations/list_payouts) | 列出出款记录 |

### 子商户 Sub-Merchants

为 PSP 和聚合平台场景管理子商户。

| 端点 | 说明 |
|------|------|
| [`POST /v1/sub-merchants`](/api/operations/create_sub_merchant) | 创建子商户 |
| [`GET /v1/sub-merchants/:code`](/api/operations/get_sub_merchant) | 获取子商户详情 |
| [`GET /v1/sub-merchants`](/api/operations/list_sub_merchants) | 列出子商户 |
| [`PATCH /v1/sub-merchants/:code`](/api/operations/update_sub_merchant) | 更新子商户 |

---

> 💡 你也可以通过 [Scalar 交互式文档](https://api.ironixpay.com/docs) 在线测试 API，支持直接发送请求。
