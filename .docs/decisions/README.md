# Architecture Decision Records (ADR)

> 记录重要的架构决策。每个 ADR 回答三个问题：**为什么做这个决策？做了什么决策？有什么后果？**

## 索引

| # | 标题 | 状态 | 日期 |
|---|------|------|------|
| 001 | [Treasury 模型与出金架构](001-treasury-model.md) | Accepted | 2026-02-24 |
| 002 | [退款从 Payment Address 发起](002-refund-from-payment-address.md) | Accepted | 2026-03-17 |
| 003 | [Lazy Credit 入账机制](003-lazy-credit.md) | Accepted | 2026-02 |
| 004 | [单进程单网络隔离](004-single-process-per-network.md) | Accepted | 2026-01 |
| 005 | [Agent 代理费率模型](005-agent-fee-model.md) | Accepted | 2026-03-18 |

## ADR 模板

```markdown
# ADR-NNN: [标题]

> 日期: YYYY-MM-DD
> 状态: Proposed / Accepted / Deprecated / Superseded by ADR-NNN

## Context
为什么需要做这个决策？（问题背景、约束、痛点）

## Decision
做了什么决策？（具体方案、关键取舍）

## Consequences
- ✅ 好处
- ⚠️ 代价 / 需要注意的事
- 🔮 未来可能的演进
```
