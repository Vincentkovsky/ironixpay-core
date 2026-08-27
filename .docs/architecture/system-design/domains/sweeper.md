# Sweeper Service + Energy Manager + Transaction Monitor

> 📍 [返回架构目录](../README.md)

将用户支付到临时地址的资金归集到平台 Treasury 地址。管理 TRON 能量/带宽资源，处理 EVM Gas 赞助和 Solana ATA 回收，标准化链上交易确认流程。

---

## Sweeper Service (自动扫账/归集服务)

- **职责**: 将用户支付到 Checkout 临时地址的资金归集到 **平台 Treasury 地址**。Session 状态为 `Paid/Overpaid` 时触发归集。

    > [!IMPORTANT]
    > **Ledger 模式**: 归集目标统一为平台 Treasury 地址，不再转入商户的 `collection_address`。商户余额通过 [BillingService](billing.md) 在 Session 终态时入账（见 [payment-processing.md](payment-processing.md) Lazy Credit），链上资金由平台统一管理。

- **运行模式**: **Background Polling Service** (后台轮询服务)。
    - **Broadcast Cycle**: 广播归集交易。
    - **Confirmation Cycle**: 确认链上状态。
    - **Recycle Cycle**: 回收冷却地址 (`Cooling → Idle`)。
    - **Expired Assigned Recycle Cycle**: 回收过期 Session 的 `Assigned` 地址 (余额为 0 时直接 `Assigned → Idle`)。
- **依赖服务**:
    - **EnergyManager**: 处理 TRON 链的资源评估、带宽补充及能量租赁。
    - **交易确认**: 各 `SweepExecutor` 实现自行处理链上确认逻辑 — `TronSweepExecutor` 使用 `TransactionMonitor` (TRON 专用)，`EvmSweepExecutor` 使用 `EvmClient::get_transaction_info` 轮询，`SolanaSweepExecutor` 使用 `SolanaClient::get_signature_statuses` 轮询。

    以下流程主要描述 **Production** 环境的 Energy Rental 逻辑（由 `EnergyManager` 封装）：

    ```
    ┌──────────────────────────────────────────────────────────────────────┐
    │  Step 1: 资源准备 (EnergyManager::ensure_resources)                   │
    │  ├─ 检查 Bandwidth: 若不足且 TRX 余额为 0，Sponsor 地址转入 0.35 TRX   │
    │  │   └─ 该步骤确保 Sandbox/Mainnet 新激活账户也能正常发起交易           │
    │  ├─ 估算 Energy: 使用 estimate_energy 模拟交易，获取精确 Energy 消耗     │
    │  │   └─ 包含目标地址 USDT 余额为 0 时的翻倍消耗逻辑                     │
    │  └─ 委托 Energy: 若自身 Energy 不足，调用 Provider 进行租赁/代理        │
    ├──────────────────────────────────────────────────────────────────────┤
    │  Step 2: 广播归集交易                                                 │
    │  └─ Sweeper 构建并签名交易，将 USDT 全额转出至平台 Treasury 地址       │
    ├──────────────────────────────────────────────────────────────────────┤
    │  Step 3: 确认 (Per-Chain Confirmation)                               │
    │  ├─ TRON: TransactionMonitor 轮询检查交易确认数 (默认 19 Block)       │
    │  ├─ EVM: EvmClient.get_transaction_info 轮询 receipt + 确认数        │
    │  ├─ Solana: SolanaClient.get_signature_statuses 轮询确认状态         │
    │  ├─ 更新 outbound_transactions 状态 → Confirmed                        │
    │  ├─ 更新 checkout_sessions.settlement_status → Settled               │
    │  └─ 记录实际 Gas 成本 (energy/gas fee/SOL) 用于内部审计              │
    └──────────────────────────────────────────────────────────────────────┘
    ```
    > [!NOTE]
    > **Ledger 模式**: Sweep 确认时**不再收取手续费或更新商户余额**。手续费已在 Session 终态入账时扣除（见 [payment-processing.md](payment-processing.md) Lazy Credit）。`finalize_sweep_success` 仅记录链上实际 Gas 成本供内部偿付能力监控。

- **资源配置参数**:

    | 参数 | 默认值 | 说明 |
    | :--- | :--- | :--- |
    | `energy_estimate` | 65,000 | 基础估算值 (Fallback) |
    | `bandwidth_trx_amount` | 0.35 TRX | 覆盖约 345 bytes 交易的 Bandwidth 费用 |
    | `confirmation_blocks` | 19 | 等待链上确认区块数 |
    | `max_sweep_attempts` | 3 | 归集最大重试次数 |
    | `max_concurrent_sweeps` | 5 | 并发归集信号量 (防止 RPC 过载) |
    | `stuck_timeout_seconds` | 300 | 长时间 Pending 的告警阈值 (5分钟)，不改变交易状态 |
    | `cooling_period_seconds` | 900 | 地址冷却期 (15分钟) |
    | `platform_treasury_address` | (必须配置) | **所有地址**的统一归集目标 |

    > [!WARNING]
    > **TRON 能量消耗规则**: 若接收地址当前 USDT 余额为 **0**，EnergyManager 会自动处理翻倍的能量需求 (~131,000 Energy)。

    > [!CAUTION]
    > **归集前置校验**: Sweeper 在广播前必须**再次查询**关联 Session 的最新状态，确认其为 `Paid/Overpaid/Expired`。禁止仅依赖 Address 的 `Detected` 状态触发归集。

- **容灾机制 (Fallback Strategy)**:
    - `EnergyManager` 内置降级策略：若 `EnergyProvider` 调用失败，会自动降级为 **TRX Burning (直接燃烧)** 模式（需 Sponsor 钱包有充足 TRX）。

- **失败分类与重试策略**:

    | 失败类型 | 处理策略 |
    | :--- | :--- |
    | **NetworkError** (广播前网络/RPC 错误) | Rollback 后在下个周期重试 |
    | **ResourceFailed** (资源不足) | Rollback 并标记 Failed, 下次周期重试 |
    | **BroadcastAmbiguous** (广播响应错误/超时) | 保持 `BroadcastUnknown`，重播数据库中同一份签名载荷；禁止按超时回滚 |
    | **ChainReverted** (链上执行失败) | 标记为 `Reverted`，地址回到 Detected，允许生成新尝试 |
    | **Expired/Replaced** | 仅在链规则证明原交易不可能再上链后释放业务状态并允许重试 |
    | **LongPending** | 保持 `Pending` 并告警，不根据墙上时间推断失败 |

    Sweep confirmation worker 只消费 `auto_sweep/manual_sweep/manual_transfer` 的 root `token_transfer`。`payout/withdrawal` 由 PayoutService 独占处理，gas/energy 子交易由辅助交易恢复流程处理；journal 终态与 Address、Session、Exception 状态在同一数据库事务中提交。

---

## Solana Sweep 特殊机制

- **SolanaSweepExecutor** (`services/solana/sweep_executor.rs`): 实现 `SweepExecutor` trait。
- **双签名交易**: 每笔 Sweep 交易需要两个签名：
    | 签名方 | 角色 | HD 路径 |
    | :--- | :--- | :--- |
    | 子地址 owner | Token 授权 (TransferChecked + CloseAccount) | `m/44'/501'/{account}'/0/{path}` |
    | Treasury | Fee payer (SOL 支付交易费) | `m/44'/501'/0'/0/0` |
- **指令组合** (单笔原子交易):
    1. `SetComputeUnitLimit(100_000)` + `SetComputeUnitPrice` — Compute Budget
    2. `CreateAssociatedTokenAccountIdempotent` — 确保 Treasury ATA 存在（幂等，已存在则 no-op）
    3. `TransferChecked` — SPL Token 全额转至 Treasury ATA
    4. `CloseAccount` — 关闭子地址的 ATA，回收 rent (~0.002 SOL) 至 Treasury
- **无 Gas Funder**: Solana 的 fee payer 机制天然支持由 Treasury 代付交易费，无需像 EVM 的 `GasFunder` 预先向子地址转入原生代币。
- **无 Energy Manager**: Solana 没有 Energy/Bandwidth 概念，交易费为固定 SOL 费用 (~5000 lamports base + priority fee)。
- **ATA Rent 回收**: `CloseAccount` 指令将 ATA 的 rent-exempt reserve (~0.00203 SOL) 返还给 Treasury，实现零残留。

---

## Energy Manager Service (能量管理服务)

- **职责**: 统一管理 TRON 网络的所有资源消耗（Bandwidth/Energy），确保交易（Sweeping/Manual Transfer）顺利执行且成本最低。
- **核心逻辑 (`ensure_resources`)**:
    1. **检测**: 检查发起地址是否激活、TRX 余额是否充足以及接收方 USDT 状态。
    2. **带宽补充**: 若发起地址带宽不足，自动从 Sponsor 钱包转入小额 TRX (0.35 TRX)。
    3. **能量估算**: 使用 `alloy_sol_types` 本地构建交易并调用 `estimate_energy` 获取精确消耗。
    4. **委托**: 自动计算现有能量，通过 `EnergyRentalProvider` 接口调用具体实现（如 Netts），仅委托缺口部分（Reuse 策略）。
    5. **降级**: 若租赁服务不可用，回退至 Sponsor 转入足额 TRX 进行直接燃烧（需 Provider 支持或手动干预）。

---

## Transaction Monitor Service (交易监控服务)

- **职责**: 标准化链上交易的确认流程，消除不同模块（扫账、手工打款）对交易状态判断的不一致。
- **状态判断 (`check_tx_status`)**:
    - **Confirmed**: 交易成功且达到指定区块确认数 (e.g., 19 blocks)。
    - **Reverted**: 交易上链但执行失败 (如 Out of Energy)。
    - **Pending**: 交易未上链或未达到确认数。
- **价值**: 提供统一的 `TransactionStatus` 枚举，简化上层业务逻辑。
