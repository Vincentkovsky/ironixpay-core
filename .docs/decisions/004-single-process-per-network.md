# ADR-004: 单进程单网络隔离

> 日期: 2026-01
> 状态: **Accepted**

## Context

系统需要同时支持 Production（Mainnet）和 Sandbox（Testnet），且未来支持多条链。如何隔离不同环境防止数据污染和私钥误用？

**备选方案**：
1. **代码级隔离**: 一个进程内用 if/else 区分环境 — 容易出错，一个 bug 可能影响所有环境
2. **进程级隔离**: 每个环境运行独立进程 — 物理隔离，互不影响
3. **容器级隔离**: 每个环境独立容器 + 独立数据库 — 最强隔离但运维成本高

## Decision

**采用方案 2+3 的混合：Single Network per Process + Database-Level Isolation。**

- 每个后端进程启动时通过 `ENVIRONMENT` 环境变量指定唯一环境（Production 或 Sandbox）
- 进程启动时自动推导 Network、RPC 节点、USDT 合约地址
- 两个独立数据库 `ironixpay_prod` / `ironixpay_sandbox` 在同一 PostgreSQL 实例中
- Docker Compose 启动两个独立容器 `app-prod` / `app-sandbox`

## Consequences

- ✅ 物理隔离：Production 代码不可能意外操作 Sandbox 数据，反之亦然
- ✅ 独立部署：可以只重启 Sandbox 而不影响 Production
- ✅ 独立 Migration：两个数据库的 schema 变更互不阻塞
- ✅ JIT Shadow 机制解决跨环境身份问题（Dashboard 切换 Test Mode）
- ⚠️ 需要同时维护两个数据库的 migration
- ⚠️ 部署时需要注意两个容器的启动顺序（DB → app-prod → app-sandbox）
