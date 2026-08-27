# ADR-002: 退款从 Payment Address 发起

> 日期: 2026-03-17
> 状态: **Accepted**

## Context

Resolution Service 处理异常支付时需要执行 ManualTransfer（退款/转账给用户）。最初设计是从 Platform Treasury 发起退款。

**问题**:
- Treasury 是全平台共享的资金池，如果退款从 Treasury 发起，需要复杂的余额对账逻辑
- Sweeper 可能在 Resolution 操作前就把资金归集走了，导致 payment address 余额为零
- 从 Treasury 退款需要额外的 gas 费用（Treasury → 用户 vs Payment Address → 用户）

## Decision

**所有 ManualTransfer 从 payment address（收款地址）直接发起，而非 Treasury。**

关键配套变更：
1. **Sweeper 豁免**: 当 payment address 有未解决的 `payment_exceptions` 时，Sweeper 跳过该地址，不归集
2. **余额预检**: `ManualTransfer` 前检查链上余额是否足够，对历史数据（资金已被归集）优雅失败
3. **失败回滚**: 任何发送失败的路径都正确回滚 exception 状态

## Consequences

- ✅ 资金流更简单：异常资金原路退回，无需经过 Treasury 中转
- ✅ 省 gas：只需一笔链上交易（payment address → 用户）
- ✅ Sweeper 与 Resolution 的交互更清晰（豁免规则）
- ⚠️ 对于历史已归集的异常支付，ManualTransfer 不可用（需人工从 Treasury 处理）
- ⚠️ Sweeper 需要额外查询 `payment_exceptions` 表判断是否跳过
