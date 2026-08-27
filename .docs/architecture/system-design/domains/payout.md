# Payout Service (出金服务)

> 📍 [返回架构目录](../README.md)

统一处理商户出金：余额扣减 → 链上转账 → 确认/回滚。服务内部管理两条独立的出金路径。

---

## 架构: Semaphore-gated Background Worker (双路径统一调度)

### 两条出金路径

| | Withdrawal (提现) | Payout (出金 API) |
| :--- | :--- | :--- |
| **触发方** | 商户在 Dashboard 手动发起 | 商户后端通过 Public API 程序化调用 |
| **认证** | JWT + 强制 2FA | API Key (`sk_live_`/`sk_test_`) |
| **目标地址** | 商户自己的 `collection_address` | 任意外部地址 (由请求指定) |
| **幂等性** | 无 (Dashboard 操作) | `Idempotency-Key` header (UNIQUE 约束) |
| **Webhook** | ❌ (商户在 Dashboard 直接观察状态) | ✅ `payout.completed` / `payout.failed` |
| **AML 检查** | ❌ (目标是商户自己地址) | ✅ 强制 (目标为任意地址) |
| **限额** | 无 | 单笔 10,000 USDT / 日累 50,000 USDT |
| **ID 前缀** | `wd_` | `po_` |
| **数据表** | `withdrawals` | `payouts` |

> [!NOTE]
> Withdrawal 不发送 Webhook 是有意的设计决策。Withdrawal 是商户从 Dashboard UI 发起的操作，商户可以直接在前端观察状态变化，不需要异步通知。`completed_event_type()`/`failed_event_type()` 中保留了 `wd_` 前缀分支，作为未来扩展预留。

### 统一 Worker Loop

```
┌──────────────────────────────────────────────────────────┐
│  PayoutService.start()  (每 30s 轮询)                      │
│  ├─ reset_processing_on_startup      (withdrawals)       │
│  ├─ reset_processing_payouts_on_startup (payouts)        │
│  └─ loop:                                                 │
│     ├─ Phase 0: auto_expire_stale_approvals() (24h)     │
│     ├─ Phase 1: confirm_processing_*()                   │
│     ├─ Phase 2: process_pending_withdrawals()            │
│     └─ Phase 2: process_pending_payouts()                │
└──────────────────────────────────────────────────────────┘
```

> [!NOTE]
> Phase 0 (auto-expire) 先于广播执行，确保过期的 PendingApproval 记录在广播前被清理。
> `process_pending_*` 仅查询 `status=Pending`，`PendingApproval` 记录不会被广播。

两条路径共享：
- **Broadcast Semaphore**: 最大并发 5，仅限广播阶段持有，确认轮询在 permit 释放后运行。
- **Per-chain Broadcast Lock**: Mutex 序列化同一链族的 nonce 查询 → 签名 → 广播，防止 EVM nonce 冲突。
- **PayoutExecutor trait**: 链无关的执行器接口 (`TronPayoutExecutor` / `EvmPayoutExecutor` / `SolanaPayoutExecutor`)。
- **Treasury HD 路径**: `m/44'/{coin_type}'/0'/0/0` — account_index=0, path_index=0。

---

## Withdrawal 路径 (Dashboard)

```
┌────────────────────────────────────────────────────────┐
│  1. request_withdrawal (API, 同步, JWT + 2FA)            │
│  ├─ FOR UPDATE 锁定 merchant_chain_accounts              │
│  ├─ Flat Fee 计算 (No-Loss: net ≥ 1 USDT)               │
│  ├─ Risk Control → Pending 或 PendingApproval           │
│  ├─ BillingService.process_withdrawal_debit (Withdrawal) │
│  └─ INSERT withdrawals (status=Pending|PendingApproval)  │
├──────────── if PendingApproval ────────────────────────┤
│  1a. approve/reject (手动, JWT + TOTP)                   │
│  ├─ approve → CAS: PendingApproval → Pending            │
│  ├─ reject  → CAS: PendingApproval|Pending → Cancelled  │
│  │   + refund_cost (原子退款)                             │
│  └─ auto-expire (24h) → ApprovalExpired + refund        │
├────────────────────────────────────────────────────────┤
│  2. execute_broadcast (后台, Semaphore 限制)              │
│  ├─ CAS: Pending → Processing                           │
│  ├─ PayoutExecutor.execute_payout                        │
│  │   (Treasury → merchant collection_address)            │
│  └─ 记录 tx_hash                                        │
├────────────────────────────────────────────────────────┤
│  3. poll_confirmation (后台, 无 Semaphore, 无 Webhook)    │
│  ├─ 轮询链上确认 (30 blocks, 超时 10min)                  │
│  ├─ Confirmed → status=Completed + upsert_trusted_addr  │
│  └─ Failed/Timeout → fail_and_refund (原子退款)          │
└────────────────────────────────────────────────────────┘
```

- **安全机制**:
    | 机制 | 说明 |
    | :--- | :--- |
    | **强制 2FA** | API 层强制要求 TOTP 码，未启用 2FA 则拒绝提现 (M12) |
    | **FOR UPDATE 锁** | 事务内锁定 chain_account 行，防止并发双花 |
    | **amount > 0** | API 层验证提现金额必须为正 (M11) |
    | **Brute-force** | 2FA 验证 5 次失败/5 分钟锁定 |

---

## Payout 路径 (Public API)

```
┌────────────────────────────────────────────────────────┐
│  1. create_payout (API, 同步, API Key)                   │
│  ├─ 地址校验 (TRON Base58 / EVM hex decode / Solana Base58 验证)      │
│  ├─ Self-transfer 防护:                                              │
│  │   ├─ EVM: 大小写不敏感比较 (to_lowercase) 防 EIP-55 绕过          │
│  │   └─ TRON/Solana: 大小写敏感比较 (Base58 原生大小写敏感)          │
│  ├─ AML 检查 (强制, 目标为任意地址)                       │
│  ├─ 费率计算 (per-chain outbound_fee 覆盖全局 flat_fee)   │
│  ├─ 限额检查:                                            │
│  │   ├─ 单笔 ≤ 10,000 USDT                              │
│  │   └─ 日累 ≤ 50,000 USDT (SQL SUM 聚合, 排除 Failed)   │
│  ├─ Risk Control → Pending 或 PendingApproval            │
│  ├─ FOR UPDATE 锁定 chain_account → 余额扣减              │
│  ├─ billing_logs (BillingType=Payout, amount_change=-N)   │
│  └─ INSERT payouts (status=Pending|PendingApproval)       │
├──────────── if PendingApproval ────────────────────────┤
│  1a. approve/reject (Dashboard, JWT + TOTP)              │
│  ├─ approve → CAS: PendingApproval → Pending            │
│  ├─ reject  → CAS → Cancelled + refund                  │
│  └─ auto-expire (24h) → ApprovalExpired + refund        │
├────────────────────────────────────────────────────────┤
│  2. execute_payout_broadcast (后台, Semaphore 限制)       │
│  ├─ CAS: Pending → Processing                           │
│  ├─ PayoutExecutor.execute_payout                        │
│  │   (Treasury → 商户指定的任意 to_address)               │
│  └─ 记录 tx_hash                                        │
├────────────────────────────────────────────────────────┤
│  3. poll_payout_confirmation (后台, Transactional Outbox) │
│  ├─ Confirmed → status=Completed + queue webhook         │
│  │   + upsert_trusted_address                            │
│  │   → Post-commit: trigger_delivery()                   │
│  └─ Failed → fail_and_refund_payout (原子退款 + webhook)  │
└────────────────────────────────────────────────────────┘
```

- **幂等性**: `UNIQUE(merchant_id, environment, idempotency_key)`。冲突时返回已有 payout（Idempotent Replay），而非 409。
- **AML**: 使用 `AmlService.check_address()` 对目标地址做黑名单 + GoPlus 二层检查。
- **EVM 地址校验**: 验证 `0x` 前缀 + 42 字符 + `hex::decode` 有效性，防止非法字符通过。
- **Solana 地址校验**: 验证 Base58 解码成功 + 32 字节 public key 长度。
- **Self-transfer 防护**:
    - **EVM**: 使用 `to_lowercase()` 比较，防止 EIP-55 checksummed 地址绕过。
    - **TRON/Solana**: 直接大小写敏感比较（Base58 天然大小写敏感）。

- **Webhook 事件**:
    | 事件类型 | 触发条件 | Outbox 模式 |
    | :--- | :--- | :--- |
    | `payout.completed` | 链上确认成功 | ✅ 同一事务 |
    | `payout.failed` | 广播失败/链上 Revert/超时 | ✅ 同一事务 |

---

## 共享机制

### 费用策略 (Flat Fee)

```
fee = chain_outbound_fee ?? flat_payout_fee   // per-chain 覆盖 → 全局兜底 (1.5 USDT)
net = amount - fee                            // 实发金额
```
> [!IMPORTANT]
> 出金必须保证 `net > 0`，若金额不足以覆盖手续费，拒绝。

### 失败回滚

- 广播失败、链上 Revert 或超时 → 自动退款全额 (`Refund` BillingType) 回商户 chain_account 余额。
- 退款 + 状态更新在同一 DB 事务中确保原子性。
- Payout 路径额外在同一事务中 queue `payout.failed` webhook（Outbox 模式）。

### 启动恢复

| 状态 | 处理 |
| :--- | :--- |
| `Processing` + `tx_hash IS NULL` | 重置为 `Pending` (未广播，可安全重试) |
| `Processing` + `tx_hash IS NOT NULL` | 保留不动 + 告警运维 (可能已上链，需人工核实) |

---

## API 端点汇总

**Withdrawal (Dashboard, JWT)**:
| 端点 | 方法 | 2FA | 说明 |
| :--- | :--- | :--- | :--- |
| `/api/internal/merchants/withdrawals` | POST | ✅ | 发起提现 |
| `/api/internal/merchants/withdrawals` | GET | ❌ | 查询提现历史 |
| `/api/internal/merchants/withdrawals/:id` | GET | ❌ | 查询单笔提现详情 |

**Payout (Public API, API Key)**:
| 端点 | 方法 | 幂等性 | 说明 |
| :--- | :--- | :--- | :--- |
| `/v1/payouts` | POST | `Idempotency-Key` header | 创建 payout |
| `/v1/payouts` | GET | — | 分页查询 payout 列表 |
| `/v1/payouts/:id` | GET | — | 查询单笔 payout 详情 |

**Payout (Dashboard Read-Only, JWT)**:
| 端点 | 方法 | 说明 |
| :--- | :--- | :--- |
| `/api/internal/merchants/payouts` | GET | Dashboard 查看 payout 历史 |
| `/api/internal/merchants/payouts/:id` | GET | Dashboard 查看 payout 详情 |

**Approval Flow (Dashboard, JWT + TOTP)**:
| 端点 | 方法 | 2FA | 说明 |
| :--- | :--- | :--- | :--- |
| `/api/internal/merchants/settings/payout` | GET | ❌ | 获取 Risk Control 设置 |
| `/api/internal/merchants/settings/payout` | PUT | ❌ | 更新设置 (Owner only) |
| `/api/internal/merchants/payouts/:id/approve` | POST | ✅ | 审批 payout |
| `/api/internal/merchants/payouts/:id/reject` | POST | ✅ | 拒绝 payout (含退款) |
| `/api/internal/merchants/withdrawals/:id/approve` | POST | ✅ | 审批提现 |
| `/api/internal/merchants/withdrawals/:id/reject` | POST | ✅ | 拒绝提现 (含退款) |

---

## Risk Control & Approval Flow (审批流)

- **数据模型**:
    | 表 | 用途 |
    | :--- | :--- |
    | `payout_settings` | 商户级风控配置 (每商户一行，懒创建) |
    | `payout_trusted_addresses` | 已成功出金的地址白名单 (链上确认后自动学习) |
    | `withdrawals.requested_by/reviewed_by/reviewed_at` | 提现审批审计字段 |
    | `payouts.reviewed_by/reviewed_at` | Payout 审批审计字段 |

- **风控规则** (`should_require_approval`):
    | 规则 | 条件 | 效果 |
    | :--- | :--- | :--- |
    | 新地址审批 | `require_new_address_approval=true` 且地址不在 trusted list | → PendingApproval |
    | 金额阈值 | `approval_threshold > 0` 且 `amount > threshold` | → PendingApproval |
    | Owner 豁免 | Withdrawal 发起人为 Owner | 跳过所有审批 → Pending |
    | API Payout | 无 user identity，不适用 Owner 豁免 | 按规则判定 |

- **状态机**:
    ```
    PendingApproval ──approve──→ Pending ──broadcast──→ Processing ──confirm──→ Completed
         │                         │                        │
         ├──reject───→ Cancelled   ├──reject──→ Cancelled   ├──failed──→ Failed
         │            (+ refund)   │           (+ refund)    │           (+ refund)
         └──24h timeout──→ ApprovalExpired (+ refund + alert)
    ```

- **安全机制**:
    | 机制 | 说明 |
    | :--- | :--- |
    | TOTP 2FA | approve/reject 均需 TOTP 验证码 |
    | Self-approval 防护 | `requested_by == approver` 时拒绝 (仅 Withdrawal) |
    | CAS | approve/reject 使用 `update_many` + status filter 防并发 |
    | Auto-expire | 24h 未审批 → ApprovalExpired + 原子退款 + Critical 告警 |
    | Trusted address | 链上确认后自动 upsert，下次同地址不再触发审批 |
    | Threshold 校验 | threshold >= 0 强制校验，防止负值导致所有出金被拦截 |

---

## Auto-Withdraw Background Task (自动提现)

- **职责**: 定时检查启用了自动提现的商户，当链上余额超过阈值时，自动发起提现（跳过风控审批）。
- **入口**: `services/payout/auto_withdraw.rs`，每 5 分钟执行一次。
- **架构**: Multi-Chain × Multi-Currency 独立检查

```
┌─────────────────────────────────────────────────────────┐
│  AutoWithdrawTask.tick()  (每 5 分钟)                      │
│  ├─ SELECT FROM payout_settings                          │
│  │   WHERE auto_withdraw_enabled = true                  │
│  └─ FOR EACH merchant:                                   │
│     ├─ SELECT merchant_chain_accounts (本环境)            │
│     └─ FOR EACH chain_account:                           │
│        ├─ Check USDT balance vs threshold                │
│        │   ├─ In-flight? (Pending/Processing/Approval)   │
│        │   │   → Skip                                    │
│        │   └─ Balance ≥ threshold?                       │
│        │       → request_withdrawal(skip_risk_control)   │
│        ├─ Check USDC balance vs threshold                │
│        │   └─ (same logic)                               │
│        └─ Errors isolated per (chain, currency) bucket   │
└─────────────────────────────────────────────────────────┘
```

- **设计决策**:
    | 决策 | 说明 |
    | :--- | :--- |
    | 全局阈值 | 单一 `auto_withdraw_threshold` 适用于所有链和币种，低复杂度高 ROI |
    | 多链多币独立检查 | 每个 (chain, currency) 组合独立判断，而非汇总余额 |
    | In-flight 去重 | 查询 `withdrawals` 表是否存在 `(merchant_id, network, currency)` × `Pending/Processing/PendingApproval`，防止重复提现 |
    | 跳过风控 | `skip_risk_control = true`，自动提现不触发审批流 |
    | 错误隔离 | `try_auto_withdraw` helper 将每个 bucket 的错误隔离，单个链失败不影响其他链 |

- **前端**: Dashboard「钱包」页仅展示开关 + 阈值输入框，无链/币种选择器（系统自动检查所有链）。

---

## Frontend Smart Polling (前端数据刷新)

- **Composable**: `composables/useSmartPolling.ts`
- **机制**:
    | 特性 | 说明 |
    | :--- | :--- |
    | Visibility-aware | `document.visibilitychange` — tab 隐藏时暂停轮询，切回立刻刷新 + 恢复 |
    | 并发保护 | `isRunning` guard — 上一次 fetch 未完成时跳过本次 tick |
    | Silent 模式 | 后台轮询传 `silent=true`，不触发 loading spinner；用户操作 (搜索/刷新) 正常显示 loading |
    | 统一间隔 | 全部页面 15 秒 |
    | 自动清理 | `onUnmounted` 停止轮询 |

- **接入页面**: Dashboard, Funds, Billing, Sessions (列表+详情), Payouts (列表+详情), Resolution Center, Developer (logs)。
