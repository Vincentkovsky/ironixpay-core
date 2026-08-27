# 端到端业务流程 (End-to-End Flows)

> 📍 [返回架构目录](README.md)

支付的完整生命周期：下单 → 入账 → 归集 → 确认。

---

## 阶段一：下单与监控 (Ingestion)

1. **创建会话**: 商户调用 `POST /v1/checkout/sessions`。
    - **请求 Payload**:
      ```json
      // Crypto-only mode (amount as decimal string)
      {
        "amount": "2",
        "currency": "USDT",
        "network": "Tron",
        "client_reference_id": "order_123",
        "success_url": "https://example.com/success",
        "cancel_url": "https://example.com/cancel"
      }

      // Fiat pricing mode (amount in standard units)
      {
        "pricing_amount": "10.50",
        "pricing_currency": "USD",
        "currency": "USDT",
        "network": "Tron",
        "client_reference_id": "order_123",
        "success_url": "https://example.com/success",
        "cancel_url": "https://example.com/cancel"
      }
      ```
    - **响应 Payload**:
      ```json
      {
        "id": "cs_xxx",
        "url": "https://checkout.example.com/checkout/cs_xxx",
        "success_url": "https://example.com/success",
        "cancel_url": "https://example.com/cancel",
        "pay_address": "TXxx...",
        "amount_expected": "2",
        "status": "Pending",
        "expires_at": "2026-02-05T16:00:00Z",
        "pricing": {
          "currency": "USD",
          "amount": "10.50",
          "exchange_rate": "1.00000000"
        }
      }
      ```
    - **字段说明**:
        | 字段 | 类型 | 必填 | 说明 |
        | :--- | :--- | :--- | :--- |
        | `amount` | string | ✅* | 金额 (人类可读小数字符串, 如 "10.50")。最小值 1 USDT，精度 0.01 USDT。*仅 crypto 模式必填 |
        | `pricing_amount` | string | ✅* | 法币金额 (标准单位, 如 "10.50")。*仅 fiat 模式必填 |
        | `pricing_currency` | string | ✅* | 法币币种代码 (如 "USD", "CNY", "EUR")。*仅 fiat 模式必填 |
        | `currency` | string | ✅ | 结算币种 (`USDT` / `USDC`) |
        | `network` | Network | ✅ | 区块链网络 (如 `Tron`)，必须匹配 API Key 所属环境 |
        | `client_reference_id` | string | ❌ | 商户侧订单号 |
        | `success_url` | string | ✅ | 支付成功后重定向 URL |
        | `cancel_url` | string | ✅ | 支付过期/取消后重定向 URL |
    - **法币定价转换**: `usdt_amount = pricing_amount / exchange_rate`。USD ↔ USDT/USDC 锁定 1:1，其他法币通过 CoinGecko 实时汇率。
    - **逻辑**: `AddressManager` 从 HD 钱包派生新地址（或复用 Idle 地址），状态置为 `Assigned`。
2. **链上监听**: [Indexer](domains/indexer.md) 采用 **Block-Based 扫描模式**：
    - 每 3 秒拉取最新区块的所有 Transfer 事件。
    - 使用内存 `HashSet` 过滤系统地址 (O(1) 匹配)。
    - 监控**所有系统地址**（不仅是 Assigned/Detected），确保任何转入资金都被捕获。

---

## 阶段二：入账与状态判定 (Processing)

当 [Indexer](domains/indexer.md) 检测到一笔 USDT 转入交易并达到确认数后：
1. **Outbox Event**: 写入 `payment_events` 表 (`PaymentConfirmed`)。
2. **[PaymentEventProcessor](domains/payment-processing.md)** 原子执行：
    - 检查 `is_credited` 幂等性标志。
    - AML Gatekeeper 检查。
    - 调用 `CheckoutService::apply_payment_with_txn` 更新 Session 状态。
    - **Lazy Credit 入账**: 若 Session 到达终态 (`Paid`/`Overpaid`)，按 `amount_received` 总额一次性计算费率 → `BillingService.process_deposit(net)` → 设置 `fee_amount`/`net_amount`。
    - 标记 `is_credited = true`，队列化 Webhook。
3. **状态判定**:
    - **少付 (Underpaid)**: 触发滚动延期 (+24h)，地址保持 `Detected`，不触发归集。
    - **足额/多付 (Paid/Overpaid)**: 触发 Webhook + SSE + Sweeper。
4. **Post-Commit Side Effects**: [Webhook](domains/webhook-and-sse.md) 投递、SSE 广播、[Sweeper](domains/sweeper.md) 触发。

---

## 阶段三：定时归集 (Scheduled Sweeping)

[Sweeper Service](domains/sweeper.md) 作为后台轮询服务运行：
1. **筛选目标**: 扫描数据库中状态为 `Detected` 的地址，筛选出满足以下 **归集条件 (Sweep Criteria)** 的记录：
    - ✅ **正常归集**: 关联 Session 状态为 `Paid` 或 `Overpaid` (已完结)。
    - ✅ **残值回收**: 关联 Session 状态为 `Underpaid` **且** 已彻底超时 (TTL Expired)。
    - ✅ **风控强制**: 地址余额超过安全阈值 (如 2000 USDT)。
    - ❌ **排除项**: 排除状态为 `Locked` (AML 风控) 的地址；排除余额低于 Gas 成本 (Dust) 的地址。
2. **资源准备**: 检查目标地址是否有足够 Energy，若不足则调用 `EnergyProvider` 进行租赁或代理。
3. **广播交易**: 构建并广播归集交易（将余额 **全额** 转入 `platform_treasury_address`），将地址状态更新为 `Sweeping`。

## 阶段四：确认与回收 (Finalization)

1. **确认循环**: [Sweeper](domains/sweeper.md) 轮询 `outbound_transactions` 表中 `Signed/BroadcastUnknown/Pending` 的记录。
2. **完成流转**:
    - **成功**: 链上确认后，将地址状态更新为 `Cooling`，清零对应 token 余额，并将该地址上所有成功 Session 的 `settlement_status → Settled`。
    - **链上失败**: 记录 `Reverted`；仅在 `Expired/Replaced/Reverted` 等有链上证据的终态后释放业务状态并允许新尝试。
3. **清理任务**: 独立的 Cleaner 任务定期扫描 `Cooling` 状态的地址：
    - 冷却期满且 USDT/USDC 余额均为 0 时才重置为 `Idle`。
