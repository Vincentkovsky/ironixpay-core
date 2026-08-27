# Authentication

IronixPay 使用 **API Key** 认证请求，通过 `Authorization` Header 传递：

```http
Authorization: Bearer sk_test_abc123...
```

## API Key 类型

| 前缀 | 环境 | 说明 |
|------|------|------|
| `sk_live_` | 生产 | TRON、Solana、BSC、ETH、Polygon、Arbitrum、Optimism、Base，真实资金 |
| `sk_test_` | 沙盒 | TRON Nile 测试网，无真实资金 |

::: tip
开发阶段请用 test key。沙盒的行为和生产完全一致，只是跑在测试网上。
:::

::: warning 数据隔离
Test 和 Live 环境的数据完全隔离，test key 无法访问 live 的 Session，反之亦然。
:::

## 管理 API Key

在 [Merchant Dashboard](https://app.ironixpay.com) 中创建和管理 API Key。

- 每个环境可同时拥有多个有效 key
- 先创建新 key 再撤销旧 key，可以零停机轮换
- 不要在客户端代码中暴露 secret key

## 安全建议

1. **环境变量存储** — 不要把 key 写进代码或提交到 Git
2. **开发用 test key** — `sk_live_` 只在生产环境使用
3. **定期轮换** — 定期更换 key，疑似泄露时立即更换
4. **最小权限** — 控制团队中谁能查看和管理 API Key
