# Transaction Indexer + AML Service

> 📍 [返回架构目录](../README.md)

**链上之眼 (The Eyes)**。扫描区块链，监听所有系统地址的入账交易，并通过两层 AML 机制拦截高风险资金。

---

## Transaction Indexer (链上索引器 - The Eyes)

- **职责**: 扫描区块链，监听**所有系统地址**的入账交易（TRON/EVM 的 USDT/USDC，Solana 的 SPL Token）。
- **架构**: **Block-Based Event Scanning (O(1) per block)**
    - **Unified Cache (统一缓存)**: 使用 `Arc<DashMap<String, MonitoredAddressInfo>>` 无锁并发缓存作为**单一事实来源**，消除 Split-Brain 问题。
    - **O(1) 过滤**: 扫描 Block Events 时，仅需 O(1) 检查地址是否在 Unified Cache 中。
    - **Stateless Status**: Cache 中仅存储必要静态元数据（如 `merchant_id`），**不存储动态 Status**，避免 Stale Data 误导。
    - **API 压力与监控地址数量无关**（监控 1,000 vs 1,000,000 地址成本相同）。
- **地址缓存同步策略 (Real-time Sync)**:
    - **Hydrate Cache (启动注水)**: 服务启动时，通过 `hydrate_address_cache` 预热加载全量地址。
    - **LISTEN/NOTIFY 实时更新 (Primary)**:
        - PostgreSQL 触发器在 `addresses` 表 INSERT 时发送 `address` 负载。
        - Indexer 收到通知后，立即**回查 DB** 获取完整元数据（`merchant_id`）并写入 Unified Cache。
        - 延迟 < 10ms，实现"写后即读"的一致性体验。
    - **Fallback 安全兜底 (Secondary)**: 每 60 秒增量查询 `created_at > NOW() - 5m` 的地址，防止通知丢失。
- **状态判定逻辑**:
    - **Cache 仅用于存在性过滤**。
    - **Session 实时查询**: 确定地址存在后，**必须**实时查询 `checkout_sessions` 表获取最新 Session 状态和有效期，**彻底避免缓存过期导致的"假异常"问题**。
    - **仅查询活跃状态**: `fetch_active_session` 仅匹配 `Pending` 和 `Underpaid` 状态。`Expired` 状态的 Session **不被视为活跃**，付款将直接进入异常路径。
- **状态持久化**: `indexer_state` 表存储 `last_processed_block`，确保重启后从正确位置恢复。
- **支付分类**:
    - **正常支付**: 地址存在活跃 Session (Pending/Underpaid) → 写入 `transactions` 表。
    - **异常支付**: 无活跃 Session (含已过期) → 写入 `payment_exceptions` 表（见 [resolution.md](resolution.md)）。
    - **无宽限期 (No Grace Period)**: Session 一旦过期，任何新到的付款立即进入异常路径，进入 Resolution Center 由商户处理。TRON 出块仅 3 秒，不存在"差几秒过期"的实际问题；若用户在过期前提交交易，ExpiryWorker 的 CAS 检查会保护已付款的 Session 不被覆盖。
- **粉尘过滤阈值**:
    | 场景 | 阈值 | 处理方式 |
    | :--- | :--- | :--- |
    | 活跃 Session | 0.01 USDT | 静默忽略 |
    | 闲置地址 (≥1 USDT) | 1 USDT | 记录为异常支付 |
    | 闲置地址 (<1 USDT) | 任意金额 | 记录为 `dust_payment` 异常 |
- **区块抓取容错 (Exponential Backoff)**:
    - **永不放弃策略**: 对于网络超时、RPC 不可用等瞬态错误，无限重试。
    - **指数退避**: 1s → 2s → 4s → ... → 30s (封顶)，带 ±10% 抖动防止惊群。
    - **致命错误快速失败**: 数据解析错误、API 认证失败等立即返回错误。
    - **告警阈值**: 连续失败 10+ 次后，日志级别从 WARN 升级为 ERROR。
- **链上确认配置**:
    | 链族 | 确认数 | 约等待时间 |
    | :--- | :--- | :--- |
    | TRON | 19 blocks | ~57s (Solid Block) |
    | EVM | 视链配置 | 因链而异 |
    | Solana | `confirmed` commitment | ~2-5s (400ms slot × supermajority) |

- **Solana Indexer 特殊设计**:
    - **扫描模式**: 基于 `getSignaturesForAddress` 按 Mint ATA 扫描，而非基于区块的全量扫描。每个监控地址实时派生其 USDT/USDC ATA，针对 ATA 地址查询 `Transfer`/`TransferChecked` 指令。
    - **Slot-based 进度**: 使用 `last_processed_slot` 追踪扫描进度（Solana 使用 slot 而非 block number）。
    - **交易解析**: 逐笔调用 `getTransaction`，从 inner instructions 中提取 SPL Token 转账的 mint、金额、from/to 信息。
    - **Native SOL**: 当前仅监控 SPL Token 转账，不监控 native SOL 转账。
- **Reorg 防护 (Ghost Transaction Detection)**:
    - 交易达到 19 确认时，**必须通过 RPC 回查链上状态**，验证交易仍然存在且成功。
    - 若链上查不到交易（Reorg 导致被丢弃），标记为 `Reorged` 状态，**绝不入账**。
    - 若链上交易存在但失败（如 `OUT_OF_ENERGY`），同样标记为 `Reorged`。
- **健壮性解析 (Robust Parsing)**:
    - `TransactionInfoResponse` 结构体所有字段使用 `#[serde(default)]` 标注。
    - 应对 TRON API 返回不完整响应的场景（如刚确认的交易可能缺少 `fee` 字段）。
- **Outbox Pattern**: 确认交易存在且成功后，向 `payment_events` 表写入 `PaymentConfirmed` 事件。

---

## AML Service (反洗钱服务 - The Gatekeeper)

- **职责**: 在支付入账前检测高风险资金来源，保护商户主钱包免受黑产资金污染。
- **架构**: **两层检测机制 (Two-Layer Checking)**
    ```
    ┌──────────────────────────────────────────────────────────────────┐
    │   L1: 内存黑名单 (In-Memory Blacklist)                            │
    │   └─ DashSet<address> 缓存 OFAC SDN 制裁地址                      │
    │   └─ O(1) 检测延迟，命中即拦截，无需 API 调用                      │
    ├──────────────────────────────────────────────────────────────────┤
    │   L2: GoPlus API + DB Cache (Multi-Chain)                         │
    │   └─ 实时查询 GoPlus Security API 获取地址风险评分                 │
    │   └─ 结果缓存 24h，减少 API 调用成本                               │
    │   └─ 10+ 风险信号: honeypot_related, blacklist, phishing, etc.   │
    │   └─ 多链 chain_id 映射:                                         │
    │       TRON → "tron", BSC → "56", Ethereum → "1",                  │
    │       Polygon → "137", Solana → "solana"                          │
    └──────────────────────────────────────────────────────────────────┘
    ```
- **多链支持**: `check_address(address, network)` 接受 `network` 参数，内部通过 `network_to_goplus_chain_id()` 映射到 GoPlus API 对应的 `chain_id`。确保 BSC 地址使用 `chain_id=56` 查询，TRON 地址使用 `chain_id=tron` 查询。
- **Gatekeeper 模式**: 在 [PaymentEventProcessor](payment-processing.md) 入账前调用 AML 检测：
    1. **Safe**: 地址安全，正常入账，继续 Session 状态变更。
    2. **Blocked**: 检测到高风险，执行原子操作：
        - 将 Session 状态更新为 `Blocked`。
        - 将地址状态更新为 `Locked`。
        - 创建 `RiskBlocked` 类型的 `PaymentException`。
        - 入队 `session.blocked` Webhook 通知商户。
- **Fail-Open 策略**: 若 GoPlus API 超时或不可用，系统默认放行以保障支付转化率。API 错误会被记录但不阻断业务。
- **地址归一化**: TRON 地址保持 Base58Check 原样（大小写敏感），EVM 地址保留原始格式（0x 前缀），Solana 地址保持 Base58 原样（大小写敏感）。查询时 trim 空白字符。
- **黑名单种子**: 提供 `seed_blacklist` CLI 工具，从 OFAC SDN 列表等权威来源导入初始黑名单。
