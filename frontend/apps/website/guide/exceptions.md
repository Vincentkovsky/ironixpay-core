# 异常支付

当一笔支付无法匹配到活跃的 Checkout Session 时——无论是到账过晚、发送到闲置地址，还是触发了风控标记——IronixPay 会将其捕获为**异常支付 (Payment Exception)**。资金永远不会丢失。

你可以在 [Dashboard](https://app.ironixpay.com) 的 **Resolution Center** 中查看和处理异常。

## 异常类型

| 类型 | 触发条件 |
|------|----------|
| `session_expired` | 支付在 Session 的 30 分钟窗口**关闭后**才到账 |
| `underpaid_expired` | Session 过期时存在**部分付款**——客户在超时前支付的金额不足 |
| `no_active_session` | 支付发送到一个**没有绑定活跃 Session** 的收款地址 |
| `session_already_completed` | 支付发送到一个**已经完成付款**的地址 |
| `risk_blocked` | 支付来自被 **AML 风控筛查**标记的地址（制裁名单或风险情报） |
| `dust_payment` | 支付金额低于**灰尘阈值**（< 0.1 USDT）——自动忽略 |
| `unknown` | 不匹配任何已知模式的边界情况 |

### 生命周期

每个异常遵循简单的生命周期：

```
Pending → Processing → Resolved
                    ↘ Failed（可重试）
```

- **Pending** — 等待审核
- **Processing** — 处理操作已发起（如正在广播转账交易）
- **Resolved** — 操作成功完成
- **Failed** — 操作失败（如广播出错），可重试

## 处理操作

根据异常类型，你可以执行三种操作来处理异常：

### Accept（接受）

**将支付入账到原始 Session。**

系统会将该笔支付视为有效，Session 标记为已支付，资金自动归集到你的资金池。适用于你确认客户确实想为这笔订单付款，只是到账晚了或金额不足的情况。

::: tip 使用场景
一位老客户在 Session 过期 5 分钟后才到账。你仍然希望为其履行订单。
:::

**适用于：** `session_expired`、`underpaid_expired`

### Attach（关联）

**将支付绑定到另一个 Session。**

适用于资金到达了错误地址的场景。你选择目标 Session ID，支付会被入账到该 Session。

::: tip 使用场景
客户从交易历史中复制了一个旧地址。你可以将这笔支付关联到他们当前的 Session。
:::

**适用于：** `no_active_session`、`session_already_completed`

### Transfer（转账）

**将资金发送到任意 TRON 地址**（退款或手动结算）。

这将发起一笔链上 USDT 转账。出于安全考虑，需要 2FA 验证码。你可以转出全额或指定部分金额。

::: warning 注意
转账需要源地址上有足够的 TRX 支付网络费用。系统会优先尝试租赁能量以降低成本；如果租赁失败，则回退到燃烧 TRX。
:::

**适用于：** 除 `dust_payment` 外的所有异常类型

## 操作矩阵

各异常类型可用操作的快速参考：

| 异常类型 | Accept | Attach | Transfer |
|----------|:------:|:------:|:--------:|
| `session_expired` | ✅ | — | ✅ |
| `underpaid_expired` | ✅ | — | ✅ |
| `no_active_session` | — | ✅ | ✅ |
| `session_already_completed` | — | ✅ | ✅ |
| `risk_blocked` | — | — | ✅ |
| `dust_payment` | — | — | — |
| `unknown` | — | — | ✅ |

::: info 为什么 risk_blocked 不能执行 Accept 或 Attach？
AML 合规政策禁止将来自标记地址的资金入账到任何 Session。唯一的选项是将资金退款（Transfer）到发送方地址或其他指定地址。
:::

## Webhook 事件

当异常被创建时，IronixPay 会发送 Webhook 事件：

```json
{
  "event_type": "payment.exception",
  "data": {
    "id": "exc_abc123...",
    "exception_type": "session_expired",
    "amount": "10.5",
    "currency": "USDT",
    "tx_hash": "abc123...",
    "session_id": "cs_def456..."
  }
}
```

## 最佳实践

1. **配置 Webhook 监听** `payment.exception` 事件，确保团队能及时收到通知
2. **及时处理** — 在异常被处理前，资金会一直停留在收款地址中
3. **对合理的迟到支付使用 Accept**，而不是要求客户重新支付
4. **谨慎处理 risk_blocked 异常** — 在转账前请咨询你的合规团队
