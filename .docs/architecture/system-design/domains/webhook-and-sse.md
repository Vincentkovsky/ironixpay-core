# Webhook Service + SSE Real-time Push

> 📍 [返回架构目录](../README.md)

负责将支付结果推送给商户（Webhook 回调 + SSE 实时推送），记录完整的投递轨迹。

---

## Webhook Service (回调通知服务)

- **职责**: 负责将支付结果推送给商户配置的各个 Webhook 端点，并记录完整的投递轨迹。
- **架构**: 采用 **异步队列机制 (Asynchronous Queue)**。
    - [Checkout Service](checkout.md) 在状态变更时，查询该商户所有启用的 `webhook_endpoints`。
    - 为每个端点生成独立的任务并写入任务队列（即 `webhook_events` 表）。
    - **双层并发控制 (Double Semaphore Architecture)**:
        | 信号量 | 容量 | 作用 |
        | :--- | :--- | :--- |
        | `task_semaphore` | 1000 | 限制 `tokio::spawn` 总数，防止 OOM |
        | `delivery_semaphore` | **50** | 限制同时进行的 HTTP 投递数，防止耗尽文件描述符或 DDoS 商户 |
        - **`trigger_delivery`**: 先 `acquire_owned` 再 `spawn`，实现背压 (Backpressure)。
        - **`spawn_delivery`**: 使用 `try_acquire_owned`（非阻塞），若信号量满则跳过，由 Recovery Loop 兜底。
    - **双重投递模式**:
        - **即时投递**: 事务提交后立即尝试一次投递。
        - **后台恢复**: 独立线程每 60s 扫描待处理任务。
- **核心价值**:
    - **交付证明 (Proof of Delivery)**: 记录每次尝试的 HTTP 状态码与时间，不读取、不存储目标服务器的响应正文。
    - **手动重发 (Manual Resend)**: 支持商户在服务器修复后手动触发历史记录的重发。
- **逻辑**:
    - **指数退避重试 (Exponential Backoff)**: 0s (Initial), 15s, 1m, 5m, 1h, 6h, 24h。
    - **恢复机制 (Recovery Loop)**:
        - 自动重试 `Failed` 且到达 `next_retry_at` 的任务。
        - **僵尸任务救援**: 自动重置状态为 `Processing` 但 `last_attempt_at` 超过 **5 分钟** 的任务（处理节点崩溃场景）。
    - **安全验签 (Signature)**:
        - **Header**: `X-Signature`, `X-Timestamp`
        - **算法**: `HMAC-SHA256(Webhook_Secret, Timestamp + "." + Payload_JSON)`
        - **重放防护**: 校验 `X-Timestamp` 偏差 < 5 分钟。
    - **SSRF 防护**:
        - 仅接受 HTTPS URL，禁止 URL 内嵌用户名或密码。
        - 每次投递前解析全部 A/AAAA 记录；只要包含任一非公网地址，整次投递即被拒绝并禁用端点。
        - 将验证后的 IP 固定到该次 HTTP 客户端连接，避免 DNS rebinding 的检查/使用时间差。
        - 禁用系统代理和自动重定向，3xx 仅作为失败状态记录，不访问 `Location`。
        - 安全策略拒绝属于永久失败，不进入重试队列。
    - **死信队列 (Dead Letter Queue)**:
        - 超过最大重试次数（默认 6 次）标记为 `Giving_Up`。
        - **致命错误快速失败**: 若遇到 **Decryption Failed** (密钥解密失败) 等不可恢复错误，立即标记为 `Giving_Up`，不进行重试。
- **事件类型 (Event Types)**:

    | 事件类型 | 触发时机 | 原子性保证 | 说明 |
    | :--- | :--- | :--- | :--- |
    | `session.completed` | 支付达标 (Paid/Overpaid) | ✅ Transactional Outbox | 与支付状态更新在同一事务内入库 |
    | `session.expired` | Session 过期 (TTL 到期) | ✅ Transactional Outbox | 批量事务入库，提交后统一投递 |
    | `session.blocked` | AML 检测到高风险资金 | ✅ Transactional Outbox | 风控拦截，资金冻结，需人工处理 |

    > [!NOTE]
    > 系统不发送 `session.created` 和 `payment.received` 事件。0-conf 交易状态通过 **SSE 实时推送** 或前端轮询 `GET /session/{id}` API 获取，避免向商户暴露中间态复杂性。

- **Payload 结构 (SessionEventPayload)**:
    ```json
    {
      "event_type": "session.completed",
      "timestamp": "2026-02-04T16:00:00Z",
      "data": {
        "session_id": "cs_xxx",
        "merchant_id": "mer_xxx",
        "amount_expected": 10000000,
        "amount_received": 10500000,
        "currency": "USDT",
        "network": "Tron",
        "status": "Overpaid",
        "pay_address": "TXxx...",
        "client_reference_id": "order_123",
        "tx_count": 2,
        "transactions": [
          {"tx_hash": "abc123...", "amount": 5000000, "confirmations": 25},
          {"tx_hash": "def456...", "amount": 5500000, "confirmations": 19}
        ]
      }
    }
    ```
    - **`tx_count`**: 入账交易总数。
    - **`transactions`**: 完整交易历史，按 `block_timestamp` 升序排列。
    - 对于 `session.expired`，若存在部分支付 (Underpaid→Expired)，`transactions` 数组包含所有已入账交易，便于商户处理退款。

- **Transactional Outbox 模式**:
    ```
    ┌───────────────────────────────────────────────────────┐
    │  BEGIN TRANSACTION                                     │
    │    1. UPDATE session status (Paid/Expired)            │
    │    2. INSERT INTO webhook_events (Outbox)             │
    │  COMMIT                                                │
    └───────────────────────────────────────────────────────┘
                              │
                              ▼
    ┌───────────────────────────────────────────────────────┐
    │  AFTER COMMIT: Trigger async delivery                  │
    │    → POST to merchant endpoint                         │
    │    → Recovery Loop 处理失败重试                         │
    └───────────────────────────────────────────────────────┘
    ```
    确保业务状态与 webhook 记录的强一致性 —— 事务回滚时 webhook 不会发送。

---

## SSE Real-time Push Service (SSE 实时推送服务)

- **职责**: 为托管支付页面提供实时状态更新，替代传统的轮询机制。
- **架构**: **Server-Sent Events (EventSource)**
    ```
    ┌─────────────┐  EventSource   ┌─────────────┐  broadcast   ┌──────────────────┐
    │  Checkout   │◄──────────────│   Axum SSE  │◄────────────│ PaymentProcessor │
    │  Frontend   │                │   Endpoint  │              │   (on payment)   │
    └─────────────┘                └─────────────┘              └──────────────────┘
                                          │
                                    SseBroadcaster
                               (in-memory tokio::broadcast)
    ```
- **核心组件**:
    - **SseBroadcaster**: 单例 `Arc<SseBroadcaster>`，管理所有 Session 的广播频道。
    - **Channel 管理**: 使用 `DashMap<session_id, broadcast::Sender>` 实现无锁并发。
    - **流合并**: 使用 `BroadcastStream` + `IntervalStream` + `merge` 组合事件流和心跳流。
- **事件类型**:
    | 事件 | 触发时机 | 数据格式 |
    | :--- | :--- | :--- |
    | `session_updated` | 支付状态变更 | `{session_id, status, amount_received}` |
    | `comment: keep-alive` | 每 30s 心跳 | SSE 注释 (防止连接超时) |
- **前端集成**:
    ```typescript
    const es = new EventSource(`/v1/checkout/sessions/${id}/events`);
    es.onmessage = (e) => {
        const data = JSON.parse(e.data);
        if (data.status === 'Paid') { redirect(successUrl); }
    };
    ```
- **降级策略**:
    - SSE 连接失败时，前端自动降级为 3 秒轮询模式。
    - 3 次重连失败后永久降级，避免频繁重试。
- **端点**: `GET /v1/checkout/sessions/:id/events` (公开，无需认证)
- **资源清理**:
    - 后台任务每 **60 秒** 清理无订阅者 (`receiver_count == 0`) 的空闲频道，防止 DashMap 内存泄漏。
    - 清理任务纳入 `JoinSet` 监控，支持 `CancellationToken` 优雅关闭。
