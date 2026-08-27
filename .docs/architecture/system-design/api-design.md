# API 路由架构与扩展性设计 (API Design)

> 📍 [返回架构目录](README.md)

3-Tier 路由架构、前瞻性字段设计、幂等性机制。

---

## API 路由架构 (API Route Architecture)

系统将 API 端点按认证方式划分为六个区域：

| 路由前缀 | 认证方式 | 用途 |
| :--- | :--- | :--- |
| `/health`, `/ready` | 无 | 健康探针 (Liveness / Readiness) |
| `/v1/checkout/*` | 混合 (API Key + 公开) | Checkout API：创建/查询 Session、SSE 事件、FAPD notify-payment、公开查看 |
| `/v1/payouts/*` | API Key | Payout API：创建/查询出金（商户后端程序化调用）|
| `/api/auth/*` | 无 (公开) | 商户注册、登录、密码重置等认证端点 |
| `/api/internal/*` | JWT (Dashboard) | 商户控制台：Profile、Webhook、Resolution、Billing、提现、Payout 查看、Config (费率) |
| `/api/admin/*` | ADMIN_TOKEN (静态) | 平台管理：商户管理、费率配置、健康状态查看 |

- **`/v1/checkout`** 内部使用组合路由：公开端点（`/sessions/:id/view`、`/sessions/:id/events`、`/sessions/:id/notify-payment`）无需认证，Session CRUD 端点通过 `unified_auth` 中间件保护。
- **`/api/internal`** 在路由组层面统一应用 `jwt_auth` 中间件，子模块（merchants, webhooks, resolution, billing, config）无需重复声明。
- **`/api/admin`** 通过 `admin_auth` 中间件保护，使用静态 `ADMIN_TOKEN` 认证。

---

## API 扩展性与兼容性设计 (API Extensibility)

为了在保持 MVP 聚焦（仅支持 Tron）的同时，避免未来扩展多链时给商户带来集成痛苦，系统遵循以下 API 设计准则：

### 前瞻性字段 (Future-Proofing)
在所有涉及网络和币种的接口中，显式包含 `network` 和 `currency` 字段：
- **请求层面**: 让商户在下单时显式传递 `"network": "Tron"`。
- **校验逻辑**: 后端采用 **Strict Enum Deserialization**，自动拒绝非法网络值。
- **价值**: 当未来系统重构支持 BSC 或 Solana 时，商户侧的 API 调用结构无需任何变动，仅需修改参数值。

### 统一验签头 (Standardized Security)
所有 Webhook 通知统一使用 `X-Signature` 头，而非链相关的名称（如 `X-Tron-Signature`）。这确保了商户的中间件逻辑在未来多链环境下无需为了不同的 Header 名称而编写冗余的适配代码。

### 端点隔离 (Endpoint Segmentation)
通过独立的 `webhook_endpoints` 管理，商户可以为不同的业务模块或不同环境（Dev/Prod）配置完全独立的通知地址，确保了生产环境的稳定性与开发调试的灵活性。

---

## 幂等性设计 (Idempotency)

为防止网络重试导致重复创建订单，所有 **写操作 API**（如 `POST /v1/checkout/sessions`）支持 `Idempotency-Key` 请求头：

| Header | 说明 |
| :--- | :--- |
| `Idempotency-Key` | 商户生成的唯一标识符（推荐 UUID v4），有效期 **24 小时** |
| `Idempotency-Replayed` | 响应头，值为 `"true"` 表示该响应来自缓存 |

### 服务端处理流程 (Insert-First + CAS Zombie Takeover)

采用 **Insert-First** 策略 — 先尝试写入占位记录获取锁，失败后再查询现有记录状态。对超时 Zombie 记录使用 **CAS (Compare-And-Swap)** 防止并发双重接管。

```
┌─────────────────────────────────────────────────────────────────┐
│  Loop (最多 3 次重试)                                             │
├─────────────────────────────────────────────────────────────────┤
│  1. INSERT 占位记录 (response_code=0, response_body='{}')        │
│     ├─ 成功 → 获取锁，执行业务逻辑                                │
│     └─ UniqueViolation → 进入步骤 2                              │
├─────────────────────────────────────────────────────────────────┤
│  2. SELECT 现有记录                                               │
│     ├─ 未找到 (极端边缘: 记录在两步之间被过期清理)                   │
│     │   └─ continue (回到步骤 1 重试)                             │
│     │                                                            │
│     ├─ response_code == 0 (Processing 状态)                      │
│     │   ├─ created_at < 60s → HTTP 202 Accepted                 │
│     │   └─ created_at ≥ 60s (Zombie 超时) → 步骤 3              │
│     │                                                            │
│     ├─ request_hash 不匹配 → HTTP 409 Conflict                  │
│     └─ request_hash 匹配 → 返回缓存响应 + Idempotency-Replayed  │
├─────────────────────────────────────────────────────────────────┤
│  3. CAS Zombie 接管 (Compare-And-Swap)                           │
│     UPDATE idempotency_keys                                      │
│       SET created_at = NOW()                                     │
│       WHERE merchant_id = ? AND idempotency_key = ?              │
│         AND response_code = 0                                    │
│         AND created_at = <读到的旧值>   ← CAS 条件               │
│     ├─ rows_affected > 0 → 接管成功，执行业务逻辑                  │
│     └─ rows_affected = 0 → 被其他线程抢先，continue 重试          │
├─────────────────────────────────────────────────────────────────┤
│  4. 业务逻辑完成后 UPDATE 缓存响应                                 │
│     response_code = <实际 HTTP 状态码>                            │
│     response_body = <实际响应 JSON>                               │
└─────────────────────────────────────────────────────────────────┘
```

> [!IMPORTANT]
> **并发安全**: Zombie 接管使用 `UPDATE ... WHERE created_at = <旧值>` 作为乐观锁。当两个请求同时检测到同一 Zombie 时，仅有一个能成功更新（`rows_affected > 0`），另一个回到 Loop 重新读取状态。这确保了绝对的幂等性 — 同一笔业务逻辑只会被执行一次。

### 错误响应格式
幂等性错误使用统一的 Stripe 式嵌套格式：
```json
{
  "error": {
    "type": "idempotency_error",
    "code": "idempotency_conflict",
    "message": "Idempotency key was used with a different request body",
    "doc_url": "https://docs.ironixpay.io/errors#idempotency_conflict"
  }
}
```

### 存储方案
使用 PostgreSQL `idempotency_keys` 表（见 [database-schema.md](database-schema.md)），复合主键 `(merchant_id, idempotency_key)`，无需引入 Redis。

### 数据清理
每日凌晨通过 Cron Job 执行：
```sql
DELETE FROM idempotency_keys WHERE created_at < NOW() - INTERVAL '24 hours';
```

> [!TIP]
> 商户端应使用订单号或请求 UUID 作为 `Idempotency-Key`，确保重试安全。若未传递该 Header，请求将正常处理但不享受幂等保护。
