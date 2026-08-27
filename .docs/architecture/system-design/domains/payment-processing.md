# Payment Event Processor

> 📍 [返回架构目录](../README.md)

**执行之手 (The Hands)**。消费 Outbox 事件，驱动 Checkout 状态变更，并在 Session 终态时一次性入账商户余额。

---

## Payment Event Processor (支付处理器 - The Hands)

- **职责**: 消费 `payment_events` 表中的事件，驱动 [Checkout Service](checkout.md) 执行状态变更，并在 **Lazy Credit 模式** 下于 Session 到达终态时一次性入账商户余额。
- **架构**: Transactional Outbox Consumer。
- **依赖**: `CheckoutService`, `BillingService`, `FeeConfig`, `WebhookService`, `AmlService`, `SseBroadcaster`。
- **流程**:
    1. **Fetch**: 锁定 `pending` 状态的事件 (SKIP LOCKED)。
    2. **Atomicity (核心原子性)**: 开启数据库事务，执行以下原子操作：
        - 检查 Transaction 是否已入账 (`is_credited` flag)。
        - [AML Gatekeeper](indexer.md) 检查。
        - **Terminal Guard** (详见下方)。
        - 调用 `CheckoutService::apply_payment_with_txn` 更新 Session。
        - 标记 `transactions` 为已入账 (`is_credited = true`)。
        - **Lazy Credit**: 仅当 Session 到达终态 (`Paid`/`Overpaid`) 时，对 `amount_received` 总额一次性计算手续费并入账。
        - 设置 `checkout_sessions.fee_amount` / `net_amount` (覆盖而非累加)。
        - 标记 `payment_events` 为已处理。
        - 队列化 Webhook 事件 (原子写入)。
    3. **Commit**: 提交事务。
    4. **Post-Commit Side Effects (非事务)**:
        - 触发 [WebhookService](webhook-and-sse.md) 投递。
        - 触发 `SseBroadcaster` (实时推送至前端)。
        - 若支付成功，触发 [Sweeper Service](sweeper.md) (归集到 Treasury)。

- **Terminal Guard (终态防护)**:
    **触发时机**: 在 `apply_payment_with_txn` 之前检查 Session 是否已处于终态。
    **场景**: 竞态条件 — Indexer 看到活跃 Session 并发出 `payment_event`，但在 Payment Processor 处理前，Session 被其他事务（另一笔付款、过期、AML 拦截）推入终态。
    ```
    pre_session_status = SELECT status FROM checkout_sessions WHERE id = session_id

    IF status IS TERMINAL (Paid/Overpaid/Expired/Blocked):
      1. 根据 status 确定 exception_type:
         ├─ Paid/Overpaid → session_already_completed
         ├─ Expired       → session_expired
         └─ Blocked       → session_already_completed
      2. INSERT INTO payment_exceptions (使用 JOIN checkout_sessions + transactions 填充字段)
      3. 标记 event 为 processed (不标记 tx.is_credited = true)
      4. RETURN — 不执行 apply_payment, 不入账
    ```
    - **安全保证**: 资金不会"黑洞" — 所有终态后的迟到付款都以 Exception 形式进入 [Resolution Center](resolution.md)。
    - **可操作性**: 商户可通过 `Attach` (绑定到其他 Session) 或 `Transfer` (退款) 解决。

- **Lazy Credit 入账逻辑 (Session-Level Credit)**:
    ```
    仅当 Session 达到 Paid/Overpaid 终态时执行:
    1. total_received = updated_session.amount_received    // 整个 Session 的总收款额
    2. fee = max(floor_deposit, total_received × fee_percentage)
    3. actual_fee = min(fee, total_received)
    4. net = total_received - actual_fee
    5. if net > 0:
       5a. BillingService.process_deposit(txn, merchant_id, net)
       5b. UPDATE checkout_sessions SET fee_amount = actual_fee, net_amount = net
    ```
    - **精度保证 (C1 Fix)**: 费率计算使用 `rust_decimal::Decimal` 替代 `f64`，消除浮点精度丢失。
    - **Dust Guard (C2 Fix)**: 仅在 `net > 0` 时才创建 `billing_log` 并设置费用/净额，避免零值污染。
    - **幂等性**: Terminal Guard 确保已完成的 Session 不会被再次入账。每笔交易的 `(network, tx_hash, log_index)` + `is_credited` 标志提供双重保护。
    - **一次性费用**: 手续费在总额上只计算一次 (非每笔累加)，避免多笔小额支付的 floor fee 叠加问题。
    - **覆盖而非累加**: `fee_amount` 和 `net_amount` 为最终值直接覆盖，无需 `COALESCE + add`。

    > [!IMPORTANT]
    > **设计决策 (Lazy Credit)**: 余额入账与 Session 终态绑定。商户余额仅在 Session 到达 `Paid`/`Overpaid` 时一次性到账，手续费按总额计算。这简化了多笔支付的费用核算，避免了 per-payment floor fee 叠加，并通过 Terminal Guard 彻底消除竞态条件下的重复入账风险。
