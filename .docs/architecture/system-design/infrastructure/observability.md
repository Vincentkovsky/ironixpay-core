# Observability: Sentry + Alerting Service

> 📍 [返回架构目录](../README.md)

Sentry 生产级错误追踪与性能监控 + Slack 业务告警。

---

## Sentry (错误追踪与性能监控)

系统集成 [Sentry](https://sentry.io) 实现生产级错误追踪与性能监控。

**Crate 依赖**:
| Crate | 用途 |
| :--- | :--- |
| `sentry` | 核心 SDK (error capture, breadcrumbs) |
| `sentry-tower` | Axum/Tower HTTP 中间件 (自动创建 transaction) |
| `sentry-tracing` | 与 `tracing` 集成 (将 `error!` / `warn!` 自动上报为 Sentry Event) |

**初始化流程** (位于 `main.rs`，必须在 tracing subscriber 之前):
```rust
let _sentry_guard = sentry::init(sentry::ClientOptions {
    dsn: std::env::var("SENTRY_DSN").ok().filter(|s| !s.is_empty())
         .map(|s| s.parse().expect("Invalid SENTRY_DSN")),
    environment: Some(config.environment.to_string().into()),
    release: sentry::release_name!(),
    traces_sample_rate: if is_deployed { 0.1 } else { 1.0 },
    attach_stacktrace: true,
    ..Default::default()
});
```

**集成方式**:
- **Tracing 层**: `sentry::integrations::tracing::layer()` 注册为 `tracing_subscriber` 的一个 Layer，所有 `error!` 级别日志自动捕获为 Sentry Event。
- **HTTP 中间件**: `SentryHttpLayer` + `NewSentryLayer` 作为 Axum 全局 middleware，自动为每个 HTTP 请求创建 Performance Transaction（含路由、状态码、耗时）。

**环境控制**:
| 环境变量 | 说明 |
| :--- | :--- |
| `SENTRY_DSN` | Sentry 项目 DSN；为空或未设置时 SDK 进入 noop 模式，零性能开销 |
| `environment` | 通过 `config.environment` 自动设置（`production` / `sandbox`） |

> [!TIP]
> 本地开发无需配置 `SENTRY_DSN`，SDK 自动降级为 noop。`traces_sample_rate` 在部署环境设为 `0.1`（10% 采样），本地设为 `1.0`（全量）。

---

## Alerting Service (业务告警 — Slack)

系统通过 `AlertingService` 实现关键业务事件的 Slack 实时告警，作为 Sentry 错误追踪的补充。Sentry 捕获代码异常（panic、unhandled error），AlertingService 捕获**业务层面的危险信号**（如资金异常、服务降级）。

**架构**:
- **Fire-and-Forget**: `send_alert` 使用 `tokio::spawn` 异步发送，不阻塞主业务逻辑。
- **Dedup**: 内置 `DashMap<String, Instant>` 去重机制，同一告警 key 在冷却期（默认 5 分钟）内不重复发送。
- **Fallback**: 若 `ALERT_WEBHOOK_URL` 未配置，告警仅写入 `tracing` 日志，零副作用。

**告警级别**:

| 级别 | 语义 | Slack 颜色 |
| :--- | :--- | :--- |
| `Critical` | 需要立即人工介入 | 🔴 红色 |
| `Warning` | 需要关注但不紧急 | 🟡 黄色 |
| `Info` | 信息性记录 | 🔵 蓝色 |

**告警目录**:

| 告警 | 级别 | 触发服务 | 触发条件 |
| :--- | :--- | :--- | :--- |
| `indexer_ghost_transaction` | Critical | IndexerService | 检测到链上交易但找不到匹配的系统地址（可能链重组） |
| `sweeper_broadcast_failed` | Warning | SweeperService | 归集交易广播失败（RPC 超时、nonce 冲突等） |
| `sweeper_rollback_failed` | Critical | SweeperService | 广播失败后回滚地址状态也失败，地址可能卡死 |
| `sweeper_stuck_funds_missing` | Critical | SweeperService | 链上确认后余额不减，资金可能未真正转出 |
| `payment_event_dead_letter` | Critical | PaymentEventProcessor | 支付事件处理多次失败，进入死信队列 |
| `aml_payment_blocked` | Info | PaymentEventProcessor | AML 检查拦截了一笔高风险付款 |
| `webhook_delivery_exhausted` | Warning | WebhookService | Webhook 重试耗尽，商户可能未收到付款通知 |
| `address_pool_exhausted` | Critical | CheckoutService | 可用地址池耗尽，新订单无法创建 |

**环境变量**:

| 变量 | 说明 |
| :--- | :--- |
| `ALERT_WEBHOOK_URL` | Slack Incoming Webhook URL；未设置时告警仅写日志 |

**服务注入**: `AlertingService` 在 `main.rs` 中作为 `Arc<AlertingService>` 创建，注入到 `SweeperService`、`PaymentEventProcessor`、`WebhookService`，并通过 `AppState` 传递给 API Routes（Checkout 地址耗尽告警）。
