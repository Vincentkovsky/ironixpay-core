# ADR-005: Agent 代理费率模型

> 日期: 2026-03-18
> 状态: **Accepted**

## Context

IronixPay 考虑引入 Agent（代理）角色帮助拓展商户。Agent 负责获客和支持，平台需要设计 Agent 的分润模式。

**两个候选模型**:
1. **固定低费率**: Agent 从平台拿 0.1% 的底价，自由加价给商户
2. **利润分成**: Agent 与平台按比例分享商户产生的利润（如 Agent 拿 30%）

## Decision

**采用固定低费率模型（Agent Markup Model）。**

- 平台给 Agent 一个基础费率（如 0.1%），Agent 可以向商户自由定价（如 0.5%-1%）
- Agent 的利润 = 商户费率 - 平台基础费率
- 实现方式：`agent_profiles` 表记录 Agent 信息 + 佣金费率，`merchants` 表关联 `agent_id`
- 计费时：`agent_commission = amount × agent_rate`，从商户手续费中扣除

## Consequences

- ✅ 简单透明：Agent 清楚自己的成本和利润空间
- ✅ Agent 有定价自主权，可根据市场灵活调整
- ✅ 平台收入稳定（不受 Agent 定价影响）
- ⚠️ 需要防止 Agent 恶意低价（最低费率约束）
- 🔮 未来可扩展为阶梯费率：推荐量越大，Agent 基础费率越低
