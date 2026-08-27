# Billing Service + Price Service

> 📍 [返回架构目录](../README.md)

**核心会计模块**。管理商户 per-chain 余额，记录所有计费流水。它是唯一有权修改余额的服务。

---

## Billing Service (计费与审计服务)

- **职责**: **核心会计模块**。管理商户 **per-chain 余额**（`merchant_chain_accounts.balance`），记录所有计费流水。它是唯一有权修改余额的服务。
- **Ledger 模式**: 商户余额 = 平台代收资金的虚拟账本，资金由平台 Treasury 统一托管。

- **FeeConfig (统一费率配置)**:
    ```
    // 收款/退款: 百分比 + 最低门槛
    fee = max(floor, amount × fee_percentage)  // rust_decimal::Decimal 精度计算
    actual_fee = min(fee, amount)               // 费用不超过金额
    net = amount - actual_fee

    // 提现: 固定手续费
    fee = flat_payout_fee                      // 1.5 USDT, 与金额无关
    net = amount - fee
    ```
    | 参数 | 类型 | 默认值 | 说明 |
    | :--- | :--- | :--- | :--- |
    | `fee_percentage` | `Decimal` | 1% | 收款/退款统一费率 |
    | `floor_deposit` | `i64` | 1.0 USDT | 收款入账最低收费（可被 `chains.toml` 按链覆盖） |
    | `flat_payout_fee` | `i64` | 1.5 USDT | 提现固定手续费 (链上 Gas 成本固定，不按比例收费) |
    | `floor_refund` | `i64` | 1.5 USDT | 退款最低收费 (覆盖双向 Gas 成本) |

    > [!NOTE]
    > 费率字段使用 `rust_decimal::Decimal` 存储和计算（C1 Fix），避免 `f64` 浮点精度丢失。金额字段仍使用 `i64` 微单位，整数加减无精度问题。

- **Per-Merchant 费率覆盖 (Custom Fee Percentage)**:
    - `merchants.custom_fee_percentage` 字段 (`Decimal(5,4)`, 可空) 存储商户专属费率，如 `0.005` 表示 0.5%。
    - `NULL` 表示使用全局默认费率 (`fee_percentage`)。
    - **设置方式**: Admin API `PUT /api/admin/merchants/:id/fee` 设置/重置。
    - **查询方式**: Dashboard 通过 `GET /api/internal/config/fees` 获取生效费率（含 `effective_fee_percentage` 和 `deposit_floors` 字段）。
    - **费率优先级**: `custom_fee_percentage` > `fee_percentage` (全局默认)。
    - **Per-Chain 出金费用**: `chains.toml` 中的 `outbound_fee` 覆盖全局 `flat_payout_fee`，允许不同链不同出金手续费。
    - **Per-Chain 入金最低收费**: `chains.toml` 中的 `floor_deposit` 覆盖全局 `floor_deposit`。低 Gas 链（BSC/Polygon/L2/Solana）设为 0.1 USDT，高 Gas 链（TRON/ETH）保持 1 USDT。

- **核心操作**:
    | 操作 | 触发方 | 余额变动 | BillingType |
    | :--- | :--- | :--- | :--- |
    | **支付入账** | [PaymentEventProcessor](payment-processing.md) (Lazy Credit) | ➕ `+net_amount` | `PaymentCredit` |
    | **Accept 入账** | [ResolutionService](resolution.md).accept_expired_session | ➕ `+net_amount` | `PaymentCredit` |
    | **提现** | [PayoutService](payout.md) (Withdrawal) | ➖ `-amount` | `Withdrawal` |
    | **出金** | [PayoutService](payout.md) (Payout API) | ➖ `-amount` | `Payout` |
    | **退款** | BillingService.refund_cost | ➕ `+amount` | `Refund` |

- **原子性保证**:
    - `process_deposit` 接受泛型 `ConnectionTrait`，可嵌入调用方的事务中。
    - 使用 `SELECT ... FOR UPDATE` 悲观锁 (`get_profile_lock`) 防止并发余额更新。
    - `billing_logs` 记录 `previous_balance` 和 `balance_after`，确保 `SUM(amount_change)` = 当前余额。

- **审计日志 (`billing_logs`)**:
    - 每笔余额变动均写入 `billing_logs` 表。
    - `external_ref_id`: 关联原始交易 (如 `session_{id}`、`tx_hash` 或 `wd_{id}`)。
    - `description`: 包含详细上下文 (金额、手续费、净额、交易哈希)。

> [!NOTE]
> **§1.15 Merchant Deposit Service — 已移除**
> 商户充值模块（MerchantDeposit 地址、`/v1/merchants/deposit-address` 等端点）已在 Payout Service 重构中移除。商户余额通过 Checkout 支付入账（[Lazy Credit](payment-processing.md)）和 Resolution Accept 操作获得，提现通过 [Payout Service](payout.md) 执行。

---

## Price Service (价格服务)

- **职责**: 从 Binance API 获取实时价格，为 Checkout 页面提供法币金额展示。
- **代码位置**: `services/price/` (`mod.rs`, `binance.rs`)
- **支持的价格对**: 根据 `Network` 分派 — TRON/EVM 查询 USDT/USD，Solana 查询 SOL/USDT（用于 Gas 费成本换算）。
- **集成**: Checkout 前端在支付页面展示 USDT 对应的法币金额参考。
