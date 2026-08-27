# Checkout Service + Session Expiry + JS SDK

> 📍 [返回架构目录](../README.md)

**核心业务逻辑中心 (The Brain)**。管理 Session 生命周期，判定支付状态，提供嵌入式 SDK，处理 Session 过期。

---

## Checkout Service (收银台服务 - The Brain)

- **职责**: **核心业务逻辑中心**。管理 Session 生命周期，判定支付状态。
- **逻辑**:
    - **Session 创建**: 通过 `AddressManager` 申请地址。
    - **支付判定 (`apply_payment`)**: 接收来自 [PaymentEventProcessor](payment-processing.md) 的调用，执行：
        - 累加 `amount_received`。
        - 判定状态 (Paid/Underpaid/Overpaid)。
        - **滚动延期**: 若 Underpaid，自动延长有效期 24h。
    - **原子性**: 所有的状态更新均在数据库事务中完成。
- **API 错误处理**: 采用 **Stripe 式嵌套错误格式** (`ApiErrorBody`)，所有 API 错误统一返回 `{ error: { type, code, message, param?, doc_url? } }` 结构。`AppError` 枚举变体自动映射到 HTTP 状态码和错误分类 (如 `invalid_request_error`, `api_error`)。`ValidationError` 携带可选 `param` 字段，标明引发错误的请求参数名。
- **集成模式 (Integration Modes)**:
    | 模式 | 适用场景 | 复杂度 | 说明 |
    | :--- | :--- | :--- | :--- |
    | **Redirect 模式** | 标准电商网站 | ⭐ | 跳转到托管支付页面，支付完成后重定向回商户 |
    | **JS SDK 模式** | 自定义 UI 需求 | ⭐⭐⭐ | 在商户页面内嵌入 iframe 或调用 SDK API |

    **Redirect 模式流程**:
    ```
    1. 商户创建 Session (传入 success_url, cancel_url)
    2. 用户跳转至 session.url (托管支付页面)
    3. 支付完成 → 重定向至 success_url
       支付过期 → 重定向至 cancel_url
    4. 商户通过 Webhook 确认最终状态
    ```

---

## Session Expiry Worker (Session 过期服务)

- **职责**: 定时扫描并过期超过 TTL 的 Session，触发 `session.expired` Webhook。
- **架构**: **Per-Session Atomic Transactions (逐 Session 原子事务)**
    ```
    ┌─────────────────────────────────────────────────────────────────┐
    │  SessionExpiryWorker (每 60s 执行)                               │
    └───────────────────────────────────────────────────────────────┬─┘
                                                                    │
    ┌───── For Each Candidate ─────────────────────────────────────┐│
    │  BEGIN TRANSACTION                                            ││
    │    1. mark_session_expired_with_txn() (CAS: status check)    ││
    │    2. queue_event_with_txn() (Webhook Outbox)                ││
    │  COMMIT                                                       ││
    └──────────────────────────────────────────────────────────────┘│
                                                                    │
                        AFTER COMMIT: trigger_delivery()            │
    ```
- **核心设计**:
    - **分离查询与更新**:
        - `get_expiry_candidates()`: 只读查询 (SELECT)，返回候选 Session。
        - `mark_session_expired_with_txn()`: 在调用方事务内执行 CAS 更新。
    - **CAS 防竞态**: `WHERE status IN ('Pending', 'Underpaid')` 确保不会覆盖已支付的 Session。
    - **Bool 返回值**: 更新成功返回 `true`，CAS 失败返回 `false`，避免发送错误 Webhook。
    - **逐 Session 事务**: 一个 Session 失败不影响其他 Session 处理。
- **容错机制**:
    - **Circuit Breaker**: 连续失败 5 次后触发 Fail-Fast，让 K8s 重启 Pod。
    - **错误隔离**: 单个 Session 处理失败仅记录日志，继续处理其他 Session。
- **与 Webhook Service 集成**: 遵循 Transactional Outbox 模式，状态更新与 Webhook 入库在同一事务，提交后才触发投递。

---

## JS SDK (Embedded Checkout Mode)

- **职责**: 为商户提供可嵌入的支付 UI，通过 iFrame 在商户页面内展示支付流程。
- **架构**: **iFrame Bridge Pattern**
    ```
    ┌─────────────────┐  postMessage   ┌─────────────────┐  HTTP API   ┌──────────┐
    │  Merchant Page  │◄──────────────►│  Checkout App   │◄───────────►│ Backend  │
    │  (SDK Host)     │                │  (iFrame)       │             │ API      │
    └─────────────────┘                └─────────────────┘             └──────────┘
            │                                   │
      IronixPay SDK                        Vue Store
    (CheckoutElement)                   (checkout.ts)
    ```
- **包结构**:
    ```
    frontend/packages/sdk/
    ├── src/
    │   ├── index.ts           # 入口导出
    │   ├── IronixPay.ts        # SDK 主类（初始化配置）
    │   ├── CheckoutElement.ts # iFrame 生命周期管理
    │   ├── events.ts          # postMessage 事件常量
    │   └── types.ts           # TypeScript 类型定义
    └── dist/
        ├── ironix-pay.mjs      # ESM 格式 (~7KB)
        └── ironix-pay.umd.js   # UMD 格式 (~4.5KB)
    ```
- **核心安全机制**:
    | 机制 | 说明 |
    | :--- | :--- |
    | **Origin 验证** | SDK 严格校验 `event.origin`，防止恶意页面伪造消息 |
    | **Source 标记** | 所有消息携带 `source: 'ironix-pay-checkout'` 标识，过滤其他 postMessage 流量 |
    | **加载超时** | iFrame 加载 10 秒未响应 READY 事件，触发 `error` 回调 |
    | **Token 保护** | Session ID 本身即为访问令牌，公开 API (`/sessions/:id/view`) 无需额外认证 |

- **postMessage 协议**:
    | 方向 | 事件类型 | 触发时机 | Payload |
    | :--- | :--- | :--- | :--- |
    | Checkout → SDK | `IRONIX_PAY_READY` | iFrame 加载完成 | `{sessionId}` |
    | Checkout → SDK | `IRONIX_PAY_RESIZE` | 内容高度变化 | `{height}` |
    | Checkout → SDK | `IRONIX_PAY_PAYMENT_SUCCESS` | 支付成功 | `{sessionId, status, amountReceived, transactionHash}` |
    | Checkout → SDK | `IRONIX_PAY_PAYMENT_EXPIRED` | Session 过期 | `{sessionId}` |
    | Checkout → SDK | `IRONIX_PAY_PAYMENT_DETECTED` | 检测到支付 (0-conf) | `{sessionId, amountReceived}` |
    | Checkout → SDK | `IRONIX_PAY_ERROR` | 错误发生 | `{code, message}` |
    | SDK → Checkout | `IRONIX_PAY_INIT` | SDK 初始化完成 | `{theme, locale}` |

- **自动高度调整**:
    - Checkout 端使用 `ResizeObserver` 监听内容元素尺寸变化。
    - 实现 **防抖机制** (100ms debounce) + **阈值过滤** (5px threshold) 防止抖动。
    - 高度报告时包含 32px buffer (16px × 2) 确保内容不被裁剪。
    - 终态 (Paid/Expired) 后自动停止观察，减少资源消耗。

- **Embed 模式 UI 适配**:
    - 检测 `?embed=1` URL 参数启用嵌入模式。
    - 隐藏 Header/Footer，仅展示支付核心 UI。
    - 禁用页面内重定向，改由 SDK 事件通知商户处理导航。
    - `body.overflow = 'hidden'` 防止 iFrame 内出现滚动条。

- **商户集成示例**:
    ```javascript
    import { IronixPay } from '@ironix-pay/sdk';

    const ironixPay = new IronixPay({
        publicKey: 'pk_live_xxx',
        environment: 'production'  // or 'sandbox'
    });

    const element = ironixPay.createPaymentElement({
        sessionId: 'cs_xxx',
        theme: 'dark',
        locale: 'zh-CN'
    });

    element.mount('#checkout-container');

    element.on('payment_success', (result) => {
        console.log('Payment completed:', result.sessionId);
        // 仍需等待 Webhook 确认后再发货
    });

    element.on('payment_expired', () => {
        // 提示用户重新下单
    });
    ```

- **注意事项**:
    > [!IMPORTANT]
    > 商户应**始终以 Webhook 为准**进行发货判断。SDK 的 `payment_success` 事件仅用于 UI 更新，不应作为业务发货的唯一依据。
