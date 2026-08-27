# Reliability Architecture + Supervisor + Health Monitoring

> 📍 [返回架构目录](../README.md)

优雅停机、后台任务容错隔离、多层次系统健康检查。

---

## Reliability Architecture (高可用架构)

### Graceful Shutdown (优雅停机)
- **零中断**: 接收到 `SIGINT`/`SIGTERM` 信号时，使用全局 `CancellationToken` 通知所有组件。
- **安全退出流程**:
    1. HTTP Server 停止接收新请求。
    2. `Indexer`: 停止新区块扫描，保存当前状态。
    3. `Sweeper`/`Webhook`: 完成当前正在执行的任务（如归集广播、回调投递）后再退出，防止数据不一致。
    4. 主进程等待所有后台 `JoinSet` 任务安全结束 (Drain) 后才最终退出，默认超时 10s。

### Fail-Fast (快速失败)
- **Panic Hook**: 全局捕获任何线程的 Panic，并强制进程以非零状态码退出 (Exit Code 1)，触发 Docker/K8s 的自动重启机制。
- **JoinSet 传播**: 所有后台服务 (Indexer, Sweeper, etc.) 托管于 `JoinSet`。任何一个服务的异常退出都会导致 `JoinSet` 立即终止，并传播错误至 `main`，触发全系统重启。

### Configuration Hardening (配置加固)
- **Secret Protection**: 使用 `secrecy` crate (`Secret<String>`) 封装敏感字段 (DB URL, Private Keys, JWT Secret)。防止在 `Debug` 日志或 Panic 信息中由于意外打印而泄露私钥。
- **Key Security**: `encryption_key` (32 bytes) 存储为 `Secret<String>` (Hex Encoded) 以支持 `Clone` 派生。内存中使用 `Secret` 包裹的 Hex 字符串，仅在加解密操作时不安全地暴露 (Expose) 并临时解码，最大程度减少密钥明文在内存中的停留时间。
- **Strict CORS**: 生产环境仅允许配置的 `CHECKOUT_BASE_URL` 访问。
- **No Dotenv**: 生产环境强制禁用 `.env` 文件加载，杜绝配置漂移。

### Durable Outbound Broadcast Recovery
- **问题模型**: 节点接收交易与 HTTP/JSON-RPC 响应不是原子操作。任何链都可能已经传播交易，但客户端只收到超时、连接错误或不一致响应。
- **Write-ahead journal**: sweep、manual transfer、payout、withdrawal、EVM gas funding 与 TRX funding 在首次广播前，将确定性 tx hash 和完整签名载荷加密写入 `outbound_transactions`。外部 TRON energy rental 记录 provider reference 与 delegation tx hash。
- **状态机**: `Preparing → Signed → BroadcastUnknown/Pending → Confirmed/Reverted`。`BroadcastUnknown` 不是失败状态，也不会触发退款。
- **Root/child 隔离**: 业务转账只使用一个业务来源外键；gas/energy 子交易只设置 `parent_transaction_id`，不会复制 `session_id/exception_id/payout_id/withdrawal_id`。数据库禁止同一业务订单存在多个活跃 root attempt。
- **原子终态**: journal 的终态 CAS、业务订单状态、余额退款和 webhook outbox 在同一数据库事务中提交。CAS 失败时不得执行任何业务副作用。
- **同载荷重播**: 恢复 Worker 只重播数据库中的相同签名字节，不生成新 nonce、txID、signature 或 blockhash。
- **EVM 裁决**: receipt 失败证明 `Reverted`；所有已配置 RPC 均成功返回原 hash 不存在，且每个 RPC 的 confirmed nonce 都已大于记录 nonce，才候选为 `Replaced`。
- **TRON 裁决**: 所有已配置 full node 均返回原 hash 不存在，且交易 expiration 加缓冲已过，才候选为 `Expired`。
- **Solana 裁决**: 所有已配置 RPC 的历史签名查询均不存在，且每个 RPC 的 confirmed block height 都超过 `lastValidBlockHeight`，才候选为 `Expired`。
- **双重观察**: `Expired/Replaced` 候选必须经过至少 30 秒，并重新完成一次跨 RPC hash 与 nonce/height/expiration 检查后才能进入终态。任一 RPC 不可用时不退款。
- **退款规则**: 只有 `Reverted/Expired/Replaced` 等有链上证据的终态允许退款或业务重试；墙上时间超时只能告警。

---

## Supervisor Service (后台任务容错隔离)

- **职责**: 为所有后台服务（Indexer、Sweeper、Webhook Recovery、Payout Worker 等）提供统一的故障隔离与自动重启机制。
- **代码位置**: `services/supervisor.rs`
- **核心设计**:
    - **`supervisor_loop`**: 泛型包装器，接受 `task_factory` 闭包，在任务崩溃时自动重启。
    - **指数退避**: 初始 5s → 最大 60s，防止快速循环消耗资源。
    - **退出条件**: 仅当 `CancellationToken` 触发（优雅停机）或任务返回 `Ok(())` 时退出。
    - **错误永不传播**: Supervisor 内部捕获所有错误并重启，不会导致 `JoinSet` 级联失败。
- **健康集成**:
    - **链相关任务**: 传入 `Some((ChainHealthRegistry, Network))`，Supervisor 在重启时标记 `Starting`，崩溃时标记 `Unhealthy`。
    - **非链任务** (Webhook, Payout): 传入 `None`，仅重启 + 告警。
- **告警**: 每次崩溃通过 `AlertingService` 发送 Critical 告警（含去重 key）。

---

## Health Monitoring (健康监控)

### Chain Health Registry (`services/chain_health.rs`)
- **实时 RPC 健康状态**: 由 Indexer 在成功扫描区块后调用 `mark_healthy()` 上报，Supervisor 在任务崩溃时调用 `mark_unhealthy()`。
- **状态**: `Starting` → `Healthy` / `Unhealthy`。超过 `STALE_TIMEOUT` (5 分钟) 未上报自动转为 `Stale`。
- **存储**: `DashMap<Network, ChainStatus>`，无锁并发读写。

### Service Health Registry (`services/service_health.rs`)
- **后台服务心跳**: Sweeper、PaymentProcessor、WebhookService、PayoutService 定期上报心跳。
- **状态**: 与 ChainHealth 类似的 `Starting` → `Healthy` / `Unhealthy` / `Stale` 状态机。
- **用途**: `/ready` 端点的辅助信息（不影响 HTTP 状态码），Admin Console 展示。

### Health Endpoints
| 端点 | 认证 | 用途 |
| :--- | :--- | :--- |
| `GET /health` | 无 | Liveness probe — 进程存活即返回 200 |
| `GET /ready` | 无 | Readiness probe — 检查 DB、所有启用链健康、磁盘、连接池 |

`/ready` 严格语义：所有启用链必须为 `Healthy` 才返回 200，否则返回 503 + 详细 `details` 对象（含 `chains`, `services`, `disk_usage_percent`, `db_pool_active` 等）。
