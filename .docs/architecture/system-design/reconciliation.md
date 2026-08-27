# 账务对账 (Reconciliation)

> 📍 [返回架构目录](README.md)

---

商户可通过 API 或管理后台查询每笔订单的完整资金流向，实现入账与归集的配对追溯。

## 对账数据模型

系统通过 `checkout_sessions`、`transactions`、`outbound_transactions` 三表关联，提供完整的资金链路：

```
┌─────────────────────────────────────────────────────────────────────────┐
│  checkout_sessions (订单)                                                │
│    ├─ transactions (入账记录)         → 用户支付的链上交易               │
│    └─ outbound_transactions (归集记录)   → 资金转入 platform_treasury_address │
│                                          (含自动归集、手动归集、手动转账) │
│                                                                         │
│  payment_exceptions (异常支付)                                           │
│    └─ outbound_transactions (解决记录)   → 通过 exception_id 关联          │
└─────────────────────────────────────────────────────────────────────────┘
```

## 对账查询示例

```sql
-- 商户对账视图：入账 → 归集配对
SELECT
    cs.id AS session_id,
    cs.client_reference_id AS order_id,
    cs.amount_expected,
    cs.amount_received,
    cs.status AS session_status,
    t.tx_hash AS payment_tx,
    t.amount AS payment_amount,
    t.block_timestamp AS payment_time,
    st.tx_hash AS sweep_tx,
    st.amount AS swept_amount,
    st.cost_in_usdt AS sweep_cost_usdt,
    st.confirmed_at AS sweep_time,
    bl.amount_change AS fee_charged,
    bl.description AS fee_details
FROM checkout_sessions cs
LEFT JOIN transactions t ON t.session_id = cs.id
LEFT JOIN outbound_transactions st
       ON st.session_id = cs.id
      AND st.purpose = 'token_transfer'  -- 排除 gas/energy funding 子交易
LEFT JOIN billing_logs bl ON bl.external_ref_id = st.id
WHERE cs.merchant_id = :merchant_id
ORDER BY cs.created_at DESC;
```

## 对账字段说明

| 字段 | 来源 | 说明 |
| :--- | :--- | :--- |
| `order_id` | `checkout_sessions` | 商户侧订单号 |
| `amount_expected` | `checkout_sessions` | 应收金额 |
| `amount_received` | `checkout_sessions` | 实收金额（累计） |
| `payment_tx` | `transactions` | 用户支付的链上交易哈希 |
| `payment_amount` | `transactions` | 单笔入账金额 |
| `sweep_tx` | `outbound_transactions` | 归集交易哈希 |
| `swept_amount` | `outbound_transactions` | 归集金额 |

> [!TIP]
> 一个 Session 可能对应多条 `transactions`（用户多次补款），也可能因重试或辅助 gas 交易对应多条 `outbound_transactions`；对账主转账时筛选 `purpose = 'token_transfer'`。
> 对于手动操作（ManualSweep/ManualTransfer），`outbound_transactions.exception_id` 关联来源异常，`session_id` 为空。
