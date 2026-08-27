# ADR-003: Lazy Credit 入账机制

> 日期: 2026-02
> 状态: **Accepted**

## Context

商户余额何时入账？有两种选择：
1. **Per-Transaction Credit**: 每检测到一笔链上支付就立即计算费率并入账
2. **Session-Level Credit**: 等 Session 到达终态（Paid/Overpaid）后，按 `amount_received` 总额一次性入账

**Per-Transaction 的问题**:
- 少付场景（Underpaid）：用户分多笔转入，每笔单独计费会导致 `floor` 最低收费被重复扣取（如 3 笔 × 1 USDT floor = 3 USDT，但总额只需收 1 USDT）
- 退款/异常逻辑复杂：已入账的部分需要逐笔回滚

## Decision

**采用 Lazy Credit（惰性入账）**: 仅当 Session 状态到达 `Paid` 或 `Overpaid` 终态时，按 `amount_received` **总额** 一次性计算费率并入账到商户余额。

```
fee = max(floor, amount_received × fee_percentage)
net = amount_received - fee
merchant.balance += net
```

**实现细节**:
- `PaymentEventProcessor` 在 `apply_payment` 后检查 Session 是否到达终态
- 使用 `is_credited` 幂等性标志防止重复入账
- 费率、净额写入 `checkout_sessions.fee_amount` / `net_amount`

## Consequences

- ✅ 费率计算正确：总额一次计费，floor 只扣一次
- ✅ 逻辑简单：只有一个入账点，易于审计
- ✅ Underpaid → Paid 的状态转换自然触发入账
- ⚠️ 商户在 Session 完成前看不到余额变化（可接受，因为 Pending 状态的资金本身不确定）
- ⚠️ Expired（少付过期）的 Session 不入账 — 需要通过 Resolution (Accept) 手动处理
