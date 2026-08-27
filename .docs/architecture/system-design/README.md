# IronixPay 系统架构

> **Canonical source of truth** for all architecture decisions.
> 本目录采用渐进式披露 (Progressive Disclosure) 组织 — 从本页概览出发，按需深入各领域文档。

---

## 核心设计理念

| 理念 | 说明 |
|------|------|
| **Judge-Executor** | Checkout (judge) 验证判定；Indexer/Sweeper (executor) 独立执行链上操作 |
| **Transactional Outbox** | `payment_events` 表作为事件队列，保证业务状态与副作用的原子一致性 |
| **Sync-Async Split** | 即时 DB 写入 (sync) + 延迟链上操作 (async via event queue) |
| **DashMap Cache** | Indexer 使用 `DashMap` 实现 O(1) 地址查找，启动注水 + LISTEN/NOTIFY 实时同步 |
| **HD Derivation** | `m/44'/{coin_type}'/{account_index}'/0/{path_index}` — 每商户一个 account，地址自动派生 |

---

## 模块地图

### 业务领域 (`domains/`)

| 模块 | 角色 | 职责 | 详情 |
|------|------|------|------|
| **Merchant** | — | 商户注册/登录/API Key/2FA/Email | [merchant.md](domains/merchant.md) |
| **Checkout** | The Brain | Session 生命周期 + 支付判定 + SDK 嵌入 + 过期处理 | [checkout.md](domains/checkout.md) |
| **Indexer** | The Eyes | 链上扫描 + 支付检测 + AML 风控 | [indexer.md](domains/indexer.md) |
| **Payment Processor** | The Hands | 事件驱动状态机 + Lazy Credit 入账 | [payment-processing.md](domains/payment-processing.md) |
| **Sweeper** | — | 资金归集到 Treasury + 能量管理 + 交易确认 | [sweeper.md](domains/sweeper.md) |
| **Webhook & SSE** | — | 回调投递 + 实时推送 | [webhook-and-sse.md](domains/webhook-and-sse.md) |
| **Resolution** | — | 异常支付捕获 + 多种解决方案 | [resolution.md](domains/resolution.md) |
| **Billing** | — | Per-chain 余额管理 + 计费流水 + 价格服务 | [billing.md](domains/billing.md) |
| **Payout** | — | 提现 + 出金 API + 审批流 + 自动提现 | [payout.md](domains/payout.md) |
| **Chain Abstraction** | — | 多链 trait + EVM Client + chains.toml 配置 | [chain-abstraction.md](domains/chain-abstraction.md) |

### 基础设施 (`infrastructure/`)

| 模块 | 职责 | 详情 |
|------|------|------|
| **Network Isolation** | 单进程单网络 + 数据库级隔离 + 多链 enum | [network-isolation.md](infrastructure/network-isolation.md) |
| **Reliability** | 优雅停机 + Supervisor 容错 + 健康监控 | [reliability.md](infrastructure/reliability.md) |
| **Security** | 加密策略 + Master Key (KMS) + 密钥管理 | [security.md](infrastructure/security.md) |
| **Observability** | Sentry 错误追踪 + Slack 业务告警 | [observability.md](infrastructure/observability.md) |
| **Operations** | Rate Limiting + 数据留存 + Secret 轮换 | [operations.md](infrastructure/operations.md) |

### 横切关注点

| 文档 | 内容 |
|------|------|
| [state-machines.md](state-machines.md) | Checkout Session + Address 状态流 (含 Mermaid 图) |
| [flows.md](flows.md) | 端到端四阶段业务流程 (Ingestion → Processing → Sweeping → Finalization) |
| [database-schema.md](database-schema.md) | ER Diagram + 所有 Entity 定义 + 索引策略 + 一致性约束 |
| [api-design.md](api-design.md) | 3-Tier 路由架构 + 幂等性 + 前瞻性 API 设计 |
| [reconciliation.md](reconciliation.md) | 对账数据模型 + SQL 示例 |
| [testing.md](testing.md) | 测试金字塔：单元 / 属性 / E2E |

### 其他架构文档

| 文档 | 内容 |
|------|------|
| [../environment.md](../environment.md) | 部署策略讨论：Solo Developer 的 Sandbox vs Staging 选择 |
| [Decision Records](../../decisions/README.md) | ADR — 关键架构决策记录 (Treasury 模型, Lazy Credit, 退款路径等) |
| [../goal.md](../goal.md) | 项目目标 |
| [../merchant_schema.md](../merchant_schema.md) | 商户 Schema 设计 |

---

## 支持的链

| Network | ChainFamily | coin_type | 状态 |
|:--------|:------------|:----------|:-----|
| Tron | Tron | 195 | ✅ Production |
| BSC | EVM | 60 | ✅ Production |
| Ethereum | EVM | 60 | ✅ Production |
| Polygon | EVM | 60 | ✅ Production |
| Arbitrum | EVM | 60 | ✅ Production |
| Base | EVM | 60 | ✅ Production |
| Optimism | EVM | 60 | ✅ Production |
| Solana | Solana | 501 | ✅ Production |

---

## 快速导航

- 🔍 **想了解某个模块？** → 上方模块地图表
- 🔀 **想看状态流转？** → [state-machines.md](state-machines.md)
- 🗃️ **想查数据库设计？** → [database-schema.md](database-schema.md)
- 🔌 **想看 API 设计？** → [api-design.md](api-design.md)
- 🏗️ **想看部署/环境隔离？** → [../environment.md](../environment.md)
