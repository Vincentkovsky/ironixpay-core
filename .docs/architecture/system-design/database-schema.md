# 数据库 Schema 设计 (Database Schema)

> 📍 [返回架构目录](README.md)

核心实体及关联关系，体现订单与归集任务的解耦设计。

---

## ER Diagram

```mermaid
erDiagram
    merchants ||--o{ merchant_chain_accounts : "has"


    merchants ||--o{ api_keys : "has"
    merchants ||--o{ checkout_sessions : "creates"
    merchants ||--o{ webhook_endpoints : "configures"

    webhook_endpoints ||--o{ webhook_events : "receives"

    merchants ||--o{ billing_logs : "records"
    merchants ||--o{ payment_exceptions : "receives_exceptions"
    merchants ||--o{ withdrawals : "requests"
    merchants ||--o{ payouts : "creates_payouts"

    addresses ||--o{ checkout_sessions : "allocated_to"
    addresses ||--o{ outbound_transactions : "sweeps_from"
    addresses ||--o{ payment_exceptions : "receives_to"

    checkout_sessions ||--o{ transactions : "records"
    checkout_sessions ||--o{ webhook_events : "triggers"
    checkout_sessions ||--o{ billing_logs : "generates_fee"
    checkout_sessions ||--o{ payment_exceptions : "may_relate"
    checkout_sessions ||--o{ outbound_transactions : "triggers_sweep"
    payment_exceptions ||--o{ outbound_transactions : "resolved_by"
    payouts ||--o{ outbound_transactions : "executed_by"
    withdrawals ||--o{ outbound_transactions : "executed_by"
    outbound_transactions ||--o{ outbound_transactions : "has_auxiliary_tx"

    merchants ||--o{ idempotency_keys : "caches"

    merchants {
        string id PK
        string name "商户名称"
        string email UNIQUE "登录邮箱"
        string password_hash "密码哈希 (bcrypt/argon2)"
        enum status "active|suspended|pending_verification"
        int account_index UNIQUE "派生路径 account 层级 (自增)"
        string totp_secret "TOTP 密钥 (Base32, 加密存储)"
        boolean is_totp_enabled "是否启用 2FA"
        boolean email_verified "邮箱是否验证"
        int token_version "令牌版本，用于 JWT 撤销"
        string backup_codes "哈希后的 2FA 备份码 (JSON 数组)"
        decimal custom_fee_percentage "商户专属费率 (Decimal(5,4), 可空, NULL=使用全局默认)"
        timestamptz created_at
        timestamptz updated_at
    }

    merchant_chain_accounts {
        string merchant_id PK FK
        string environment PK
        string network PK "Network (TRON, BSC)"
        bigint balance "商户 per-chain 余额 (单位: USDT 微单位)"
        string xpub_encrypted "Account xpub (AES-256-GCM 加密)"
        int last_path_index "地址派生指针"
        string collection_address "商户归集目标地址"
        timestamptz created_at
        timestamptz updated_at
    }

    billing_logs {
        string id PK
        string environment "Environment (production/sandbox)"
        string merchant_id FK
        string session_id FK
        string external_ref_id "session_{id} / tx_hash / wd_{id}"
        enum billing_type "PaymentCredit|Withdrawal|Payout|Refund"
        bigint previous_balance "变动前余额 (USDT 微单位)"
        bigint amount_change "金额变动, +:Credit, -:Withdrawal"
        bigint balance_after "变动后余额 - 约束: previous + change = after"
        string description
        timestamptz created_at
    }

    webhook_endpoints {
        string id PK
        string merchant_id FK
        string url
        string description
        string secret_encrypted
        enum status "enabled|disabled"
        timestamptz created_at
    }

    api_keys {
        string id PK
        string merchant_id FK
        string key_prefix
        string key_hash
        boolean is_active
        timestamptz created_at
    }

    addresses {
        string network PK "联合主键: 网络标识 (TRON/BSC/ETHEREUM/...)"
        string address PK "联合主键: 地址"
        string merchant_id FK
        int path_index "HD派生路径索引"
        bigint native_balance "原生代币余额 (SUN/Wei)"
        bigint usdt_balance "USDT 余额 (6 decimals)"
        enum status "Idle|Assigned|Detected|Sweeping|Cooling|Locked|Error"
        text error_reason "失败原因 (仅 Error 状态)"
        int sweep_attempts "归集重试次数"
        timestamptz created_at
        timestamptz updated_at "状态最后变更时间"
    }

    checkout_sessions {
        string id PK
        string merchant_id FK
        string network FK "联合外键 → addresses(network, address)"
        string pay_address FK "联合外键 → addresses(network, address)"
        string client_reference_id "商户侧订单号"
        string currency "结算币种 (USDT / USDC)"
        string currency_contract "合约地址 - 防假币攻击"
        bigint amount_expected "应收金额 (最小单位, USDT=10^6)"
        bigint amount_received "实收金额 (最小单位)"
        bigint fee_amount "累计手续费 (USDT 微单位, Nullable)"
        bigint net_amount "累计净入账 (USDT 微单位, Nullable)"
        string success_url "支付成功重定向 URL (NOT NULL)"
        string cancel_url "支付过期/取消重定向 URL (NOT NULL)"
        enum status "Pending|Underpaid|Paid|Overpaid|Expired|Blocked"
        enum settlement_status "Unsettled|Settled"
        string pricing_currency "法币币种 (USD/CNY/EUR, Nullable - NULL=crypto模式)"
        decimal pricing_amount "法币原始金额 (Decimal(18,8), Nullable)"
        decimal exchange_rate "创建时汇率 1 crypto = N fiat (Decimal(18,8), Nullable)"
        timestamptz expires_at
        timestamptz created_at
        timestamptz updated_at
    }

    transactions {
        string network PK "联合主键: 网络标识 (TRON)"
        string tx_hash PK "联合主键: 交易哈希"
        int log_index PK "联合主键: 事件日志索引 (支持批量转账)"
        string session_id FK
        string merchant_id FK "冗余字段，便于查询"
        string currency_symbol "USDT"
        string currency_contract "合约地址 (唯一真理)"
        string from_address "用户地址 (退款用)"
        string to_address "系统地址 (校验用)"
        bigint amount "金额 (最小单位, USDT=10^6)"
        enum status "Unconfirmed|Confirmed|Reorged"
        boolean is_credited "是否已入账商户余额 (幂等性标志)"
        int confirmations_count "当前确认数"
        bigint block_number "用于计算确认深度"
        timestamptz block_timestamp "链上区块时间"
        timestamptz created_at
        timestamptz updated_at
    }

    outbound_transactions {
        string id PK
        string merchant_id FK "冗余字段，便于查询"
        enum environment "production|sandbox"
        string session_id FK "可选: 关联结账会话"
        string exception_id FK "可选: 关联来源异常 (手动操作)"
        string payout_id FK "可选: 关联 API payout"
        string withdrawal_id FK "可选: 关联商户提现"
        string parent_transaction_id FK "辅助交易关联主转账"
        enum operation_type "auto_sweep|manual_sweep|manual_transfer|payout|withdrawal"
        enum purpose "token_transfer|gas_funding|energy_funding"
        string network "链标识"
        string from_address "发送地址快照"
        string to_address "接收地址快照"
        string tx_hash "本地签名后即可确定"
        string provider_reference "外部 energy provider 订单号"
        bigint amount "业务金额或辅助交易原始金额"
        enum state "Preparing|Signed|BroadcastUnknown|Pending|Confirmed|Reverted|Expired|Replaced|Failed"
        text signed_payload_encrypted "可重播的完整签名交易，AES-GCM 加密"
        bigint nonce "EVM nonce"
        timestamptz expires_at "TRON 交易过期时间"
        bigint last_valid_block_height "Solana 有效区块高度"
        int broadcast_attempts
        timestamptz last_broadcast_at
        timestamptz observed_at
        timestamptz created_at
        timestamptz updated_at
        timestamptz confirmed_at "链上确认时间"
    }

    %% Root rows exactly one of session_id/exception_id/payout_id/withdrawal_id;
    %% child rows only set parent_transaction_id. Partial unique indexes allow
    %% at most one active root per business source and one active child per purpose.

    webhook_events {
        string id PK
        string endpoint_id FK
        string session_id FK
        string merchant_id FK
        string event_type
        jsonb payload
        enum status "pending|success|failed|giving_up"
        int http_status_code
        text response_body "遗留取证字段；应用不再写入、读取或返回"
        int attempt_count
        timestamptz last_attempt_at
        timestamptz next_retry_at
        timestamptz created_at
    }

    idempotency_keys {
        string merchant_id PK "联合主键: 商户ID"
        string idempotency_key PK "联合主键: 幂等键 (UUID)"
        string request_path "请求路径 (e.g., /v1/checkout/sessions)"
        string request_hash "请求体 SHA256 哈希"
        int response_code "缓存的响应状态码"
        jsonb response_body "缓存的响应体"
        timestamptz created_at "24h 后自动清理"
    }

    indexer_state {
        string network PK "网络标识 (TRON)"
        bigint last_processed_block "最后处理的区块号"
        timestamptz updated_at "最后更新时间"
    }

    payment_exceptions {
        string id PK
        string network "网络标识"
        string tx_hash "交易哈希"
        int log_index "事件日志索引"
        enum exception_type "SessionExpired|NoActiveSession|SessionAlreadyCompleted|DustPayment|UnderpaidExpired|RiskBlocked|Unknown"
        string to_address "接收地址 (系统地址)"
        string from_address "发送地址 (用户地址)"
        bigint amount "金额 (最小单位)"
        string currency_symbol "币种符号"
        string merchant_id FK "商户ID (若可确定)"
        string session_id FK "关联Session (若存在)"
        bigint block_number "区块号"
        timestamptz block_timestamp "区块时间"
        enum status "Pending|Processing|Resolved|Failed"
        enum resolution "Accepted|Attached|Transferred|Swept|Ignored"
        string resolution_ref_id "解决关联ID (如退款 TX Hash、目标 Session ID)"
        timestamptz resolved_at "解决时间"
        string resolved_by "解决人"
        text notes "备注"
        timestamptz created_at
        timestamptz updated_at
    }

    withdrawals {
        string id PK "wd_xxx"
        string merchant_id FK
        string environment "production|sandbox"
        string network "TRON|BSC|ETH|Polygon|..."
        bigint amount "请求提现金额 (USDT 微单位)"
        bigint network_fee "网络费 (USDT 微单位)"
        bigint net_amount "实发金额 = amount - network_fee"
        string to_address "商户 collection_address"
        enum status "Pending|Processing|Completed|Failed"
        string tx_hash "链上交易哈希 (nullable)"
        string error_reason "失败原因 (nullable)"
        timestamptz created_at
        timestamptz updated_at "用于安全判定过期的 Processing claim"
        timestamptz completed_at "完成时间 (nullable)"
    }

    payouts {
        string id PK "po_xxx"
        string merchant_id FK
        string environment "production|sandbox"
        string network "TRON|BSC|ETH|Polygon|..."
        string to_address "任意外部地址 (由商户指定)"
        bigint amount "请求金额 (USDT 微单位)"
        bigint fee "平台手续费 (USDT 微单位)"
        bigint net_amount "实发金额 = amount - fee"
        enum status "Pending|Processing|Completed|Failed"
        string tx_hash "链上交易哈希 (nullable)"
        string error_reason "失败原因 (nullable)"
        string idempotency_key "幂等键 (UNIQUE per merchant+env)"
        string description "可选描述"
        jsonb metadata "可选 JSON 元数据 (max 4KB)"
        timestamptz created_at
        timestamptz updated_at
        timestamptz completed_at "完成时间 (nullable)"
    }

    payment_events {
        string id PK
        string network
        string tx_hash
        int log_index
        string session_id FK
        bigint amount
        enum status "Pending|Processed"
        timestamptz created_at
        timestamptz processed_at
    }

    exchange_rates {
        string id PK
        string crypto "加密货币符号 (USDT/USDC)"
        string fiat "法币符号 (USD/CNY/EUR)"
        decimal rate "1 crypto = N fiat (Decimal(18,8))"
        string source "数据来源 (coingecko)"
        timestamptz created_at
    }
```

---

## 数据一致性约束 (Consistency Constraints)

> [!IMPORTANT]
> 1. **`billing_logs` 并发安全与审计闭环**:
>    - 使用 **`SELECT ... FOR UPDATE` 悲观锁** (`get_profile_lock`) 锁定商户资料行，然后执行 read-modify-write：
>      ```rust
>      let profile = get_profile_lock(txn, merchant_id, env).await?;
>      let new_balance = profile.balance + amount;
>      profile.update(txn).await?; // 同一事务内更新
>      ```
>    - 写入 `billing_logs` 时，`previous_balance + amount_change = balance_after` 构成审计闭环。
>    - 两个操作必须处于**同一事务 (ACID)** 中。
> 2. **`transactions` 三字段联合主键**: 采用 `(network, tx_hash, log_index)` 复合主键。`log_index` 用于区分单笔交易中的多个 Transfer 事件（如批量转账场景），确保每条入账记录唯一。
> 3. **金额存储与 API 表示分离**: 所有金额字段在 DB 中以 **最小单位 (i64 microunits)** 存储（USDT/USDC: 10^6, TRX/SUN: 10^6），确保整数计算无精度丢失。费率计算使用 `rust_decimal::Decimal`，结果转回 `i64` 存储。**API 边界统一使用 `from_micro()`/`to_micro()` 转换为人类可读小数字符串**（如 `"10.5"` 表示 10.5 USDT），前端直接渲染，不做任何除法。

---

## 索引策略 (Indexing Strategy)

> [!NOTE]
> ```sql
> -- 地址池查询优化 (Sweeper/Checkout 高频)
> CREATE INDEX idx_addresses_merchant_status ON addresses(merchant_id, status);
>
> -- 商户入账流水查询
> CREATE INDEX idx_transactions_merchant ON transactions(merchant_id, created_at DESC);
>
> -- Webhook 重试队列
> CREATE INDEX idx_webhook_events_retry ON webhook_events(status, next_retry_at) WHERE status IN ('pending', 'failed');
>
> -- 幂等键自动过期清理
> CREATE INDEX idx_idempotency_keys_expire ON idempotency_keys(created_at);
>
> -- 支付异常唯一约束 (防止重复记录)
> CREATE UNIQUE INDEX idx_payment_exceptions_tx_unique ON payment_exceptions(network, tx_hash, log_index);
>
> -- 支付异常商户查询
> CREATE INDEX idx_payment_exceptions_merchant ON payment_exceptions(merchant_id);
>
> -- 支付异常状态筛选 (待处理队列)
> CREATE INDEX idx_payment_exceptions_status ON payment_exceptions(status) WHERE status = 'pending';
>
> -- 支付异常地址查询
> CREATE INDEX idx_payment_exceptions_address ON payment_exceptions(network, to_address);
> ```
