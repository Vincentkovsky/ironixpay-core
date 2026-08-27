# Payment Exception Handler + Resolution Service

> 📍 [返回架构目录](../README.md)

捕获并记录无法正常入账的支付，确保系统地址上的所有资金可追溯；为运营人员提供手动处理异常支付的能力。

---

## Payment Exception Handler (支付异常处理器)

- **职责**: 捕获并记录无法正常入账的支付，确保系统地址上的所有资金可追溯。
- **异常类型 (`ExceptionType`)**:

    | 类型 | 触发条件 | 说明 |
    | :--- | :--- | :--- |
    | `SessionExpired` | 支付到达时 Session 已过期 | 用户迟到付款 |
    | `NoActiveSession` | 地址从未关联过活跃 Session | 用户误转到空闲地址 |
    | `SessionAlreadyCompleted` | Session 状态为 Paid/Overpaid | 重复支付 |
    | `DustPayment` | 金额低于阈值 (1 USDT) | 垃圾转账，自动忽略 |
    | `RiskBlocked` | AML 检测到高风险资金来源 | 资金冻结，仅允许 `manual_transfer` |
    | `UnderpaidExpired` | Session 过期时有部分付款 (≥0.1 USDT) | 残值回收记录 |
    | `Unknown` | 其他无法分类的情况 | 兜底类型 |

- **每种 Exception 的可用操作**:

    | Exception Type | Accept | Attach | Transfer | 说明 |
    | :--- | :--- | :--- | :--- | :--- |
    | `SessionExpired` | ✅ | ❌ | ✅ | 迟到付款，可接受入账或退款 |
    | `UnderpaidExpired` | ✅ | ❌ | ✅ | 部分付款，可接受入账或退款 |
    | `NoActiveSession` | ❌ | ✅ | ✅ | 无对应 Session，需绑定或退款 |
    | `SessionAlreadyCompleted` | ❌ | ✅ | ✅ | 重复付款，需绑定其他 Session 或退款 |
    | `RiskBlocked` | ❌ | ❌ | ✅ | AML 风控，仅允许退款（禁止转入商户 `collection_address`，大小写不敏感比较以防 EVM 地址绕过） |
    | `DustPayment` | ❌ | ❌ | ❌ | 自动 Ignored |
    | `Unknown` | ❌ | ❌ | ✅ | 仅退款 |

- **处理流程**:
    ```
    ┌─────────────────────────────────────────────────────────────────┐
    │  1. Indexer 检测到转账至系统地址                                  │
    │  2. 查询是否有活跃 Session (仅 Pending/Underpaid)                 │
    │     ├─ 有 → 正常入账流程 (transactions 表)                       │
    │     └─ 无 → 异常入账流程 (含 Session 已过期的情况)                │
    │           ├─ 金额 < 1 USDT → exception_type=DustPayment          │
    │           │                 → status=Resolved, resolution=Ignored│
    │           └─ 金额 >= 1 USDT → 写入 payment_exceptions 表         │
    │                             → 更新地址 usdt_balance + status=Detected │
    │                             → 进入 Resolution Center 等待处理    │
    └─────────────────────────────────────────────────────────────────┘
    ```

- **Exception 状态 (`ExceptionStatus`)**:
    | 状态 | 说明 |
    | :--- | :--- |
    | `Pending` | 待人工处理 |
    | `Processing` | 正在执行操作 (如 Sweep/Transfer 广播中) |
    | `Resolved` | 已解决 (配合 resolution 字段确定解决方式) |
    | `Failed` | 操作失败 (保留字段，当前未使用) |

- **Resolution 类型 (`Resolution`)** - 仅在 status=Resolved 时设置:
    | 类型 | 说明 | 触发操作 |
    | :--- | :--- | :--- |
    | `Accepted` | 接受过期付款 | `accept_expired_session` |
    | `Attached` | 绑定到其他 Session | `attach_session` |
    | `Transferred` | 转账到外部地址 (退款) | `manual_transfer` |
    | `Swept` | 归集到商户收款地址 | `manual_sweep` |
    | `Ignored` | 忽略 (尘埃金额自动处理) | 自动 |

---

## Resolution Service (异常解决中心)

- **职责**: 为运营人员提供手动处理异常支付的能力，支持多种解决方案。
- **核心操作**:

    | 操作 | 适用场景 | 2FA | 计费 | 说明 |
    | :--- | :--- | :--- | :--- | :--- |
    | `accept_expired_session` | SessionExpired, UnderpaidExpired | ❌ | ✅ | 接受迟到/部分付款，入账商户余额 (Lazy Credit) |
    | `attach_session` | NoActiveSession, SessionAlreadyCompleted | ❌ | ✅ | 将付款绑定到指定的其他 Session，按差额入账 |
    | `manual_sweep` | 任意 | ❌ | ✅ (via Sweeper) | 归集资金到平台 Treasury |
    | `manual_transfer` | 任意 | ✅ | ✅ | 转账到任意外部地址 (需 2FA) |

- **Accept 逻辑 (Lazy Credit 模式)**:
    `accept_expired_session` 统一处理两种异常，但语义不同：

    | | SessionExpired | UnderpaidExpired |
    |:---|:---|:---|
    | **含义** | 支付在过期后到达 | Session 过期时有部分付款，Lazy Credit 未入账 |
    | **credit_amount** | `session.amount_received + exception.amount` | `session.amount_received` (已在 Session 上) |
    | **修改 amount_received** | += exception.amount | 不修改 |
    | **修改 Session status** | -> Paid/Overpaid | 保持 Expired |
    | **修改 fee/net_amount** | 设置 | 设置 |
    | **入账余额** | +net | +net |

    > [!IMPORTANT]
    > UnderpaidExpired accept **不修改 Session 状态**（保持 `Expired`）。`Expired` 是终态，若回退为 `Underpaid` 会导致 Expiry Worker 重入和 Webhook 状态不一致。Accept 的语义是"接受部分付款入账"，而非重新激活 Session。

    两者均按 `credit_amount` 总额一次性计算手续费并入账。

- **Billing 集成 (Ledger 模式)**:
    - **`manual_sweep`**: 调用 `SweeperService.execute_sweep_logic`，归集到平台 Treasury。
    - **`manual_transfer`**: 多链支持的手动转账（需 2FA），Gas 成本由平台承担：
        - **TRON 路径**: Energy Manager 资源准备 → TRC20 Transfer 构建 → SHA256 签名 → TRON 广播
        - **EVM 路径 (BSC/ETH)**: BNB Gas Funding (gas sponsor → 子地址) → ERC20 Transfer 构建 (U256 精度转换: 6位微单位 × 10^12 → 18位) → RLP 编码 + secp256k1 签名 (coin_type=60) → EVM 广播
        - **依赖注入**: `ResolutionService` 通过 `EvmTransferConfig` (Optional) 获取 EVM 基础设施，未配置时返回清晰错误。
    - **费用说明**: Lazy Credit 模式下，手续费在 Session 终态入账或 Accept 操作时一次性扣除（见 [payment-processing.md](payment-processing.md)）。

- **安全机制**:
    - **乐观锁 (Optimistic Locking)**: `Pending → Processing` 使用 `UPDATE ... WHERE status = 'Pending'` 防止并发双花。
    - **原子性 (Write-ahead Pattern)**: `manual_transfer` 先创建 `Preparing` 记录，签名后、首次广播前加密保存完整签名交易。广播结果不确定时保持 `BroadcastUnknown` 并重播同一载荷。
    - **链上确认**: 所有出站交易（AutoSweep/ManualSweep/ManualTransfer）统一由 [Sweeper](sweeper.md) `confirmation_cycle` 监控确认，19 区块确认后标记 `Resolved`。
    - **失败回滚**: 仅在链上执行失败，或交易已被证明过期/被替换时回滚；普通超时不触发退款。
    - **tx_hash 生成**: TRON 使用 `SHA256(raw_data)` 计算交易 ID；EVM 使用 `keccak256(rlp_signed_tx)` 由 `assemble_signed_tx` 自动生成。
    - **AML 自转防护**: `RiskBlocked` 异常转账时，使用 `eq_ignore_ascii_case` 大小写不敏感比较目标地址与商户 `collection_address`，防止 EVM 地址 casing 绕过。

- **UnderpaidExpired 异常处理**:
    - 当 Session 过期时仍有部分付款（≥ 0.1 USDT），自动创建 `UnderpaidExpired` 类型的 PaymentException。
    - **tx_hash 策略**: 使用最后一笔关联交易的真实 tx_hash（而非 synthetic hash），方便链上核查。
    - **notes 审计**: 记录所有关联交易的 tx_hash 列表，格式为 `Transactions: [hash1, hash2, ...]`。
    - 若无交易记录，tx_hash 设为 `no_transactions:{session_id}`。

- **异步操作模式 (Async Operation Mode)**:
    - `manual_sweep` 和 `manual_transfer` 采用**异步执行**模式，API 立即返回 `status: "processing"`，后台任务执行实际操作。
    - **启动时重置**: 服务启动时自动将所有 `Processing` 状态的异常重置为 `Pending`，防止服务器崩溃导致的"僵尸状态"。
    - **双击防护**: 状态从 `Pending → Processing` 的原子更新在 spawn 之前完成，防止并发重复提交。
    - **失败回滚**: 后台任务失败时自动将状态回滚为 `Pending`，允许用户重试。

- **统一出站记账 (Unified Outbound Ledger)**:
    - 所有出站资金（自动归集、手动归集、手动转账）统一记录于 `outbound_transactions` 表。
    - **operation_type** 区分业务来源，**purpose** 区分主 token transfer 与 gas/energy funding。
    - `session_id`、`exception_id`、`payout_id`、`withdrawal_id` 关联业务父单，`parent_transaction_id` 关联辅助链交易。
    - 该设计确保所有出站资金可追溯，简化对账与审计。

- **Webhook 触发**:
    Resolution 操作与 Webhook 的关系：

    | 操作 | 触发 Webhook? | 说明 |
    | :--- | :--- | :--- |
    | Accept (SessionExpired) | ✅ `session.resolved` | Session → Paid，通知商户订单已恢复 |
    | Accept (UnderpaidExpired) | ✅ `session.resolved` | Session 保持 Expired，商户可通过 status 字段判断 |
    | Attach | ✅ `session.resolved` | 目标 Session 状态可能变更，通知商户 |
    | Transfer | ❌ | 仅退款，无 Session 状态变更 |

    > [!NOTE]
    > Accept/Attach 操作触发 `session.resolved` 事件（独立于 `session.completed`），遵循 Transactional Outbox 模式在事务内队列化。商户通过 Payload 中的 `status` 字段区分场景（如 `Paid` vs `Expired`）。Transfer 操作不触发 Webhook。

- **业务价值**:
    1. **资产守护**: 任何转入系统地址的资金都被捕获，不会"失踪"。
    2. **对账完整性**: 支持商户查询所有异常支付记录。
    3. **灵活处理**: 运营可根据业务场景选择最合适的解决方案。
