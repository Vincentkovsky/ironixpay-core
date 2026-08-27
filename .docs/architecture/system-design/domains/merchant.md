# Merchant Service + Address Manager + Email Service

> 📍 [返回架构目录](../README.md)

管理商户注册、登录、API Key 生成、Webhook 端点配置；基于 HD 钱包的收款地址派生与地址池管理；事务性邮件发送。

---

## Merchant Service (商户服务)

- **职责**: 管理商户注册、登录、API Key 生成及 **Webhook 端点配置**。
- **逻辑**: 处理 JWT 鉴权，管理商户的收款及扫账配置。`MerchantService` 持有 `environment` 字段以区分当前进程运行环境。
- **安全机制**:
    - **注册滥用防护**: `POST /v1/merchants/register` 在业务注册逻辑之前执行三道门禁：客户端 IP 固定窗口限制（10 分钟 5 次、24 小时 20 次）、Cloudflare Turnstile 服务端 `Siteverify`、邮箱域名拒绝列表。Turnstile token 最长 2048 字节且单次有效；后端要求 `success=true`、`action=register`、`hostname=app.ironixpay.com`，验证失败不会进入数据库或密码哈希流程。IP 限制器为单进程内存状态，当前单实例部署有效；水平扩容前应迁移到 Redis 等共享存储。
    - **Turnstile 密钥边界**: Site Key 是前端公开构建配置；Secret Key 只通过 CircleCI 的 `SHARED_TURNSTILE_SECRET_KEY` 注入后端环境，浏览器、仓库和日志均不得持有。生产启动时 `TURNSTILE_REQUIRED=true`，缺少 Secret 会拒绝启动，避免无声降级。
    - **2FA 强制校验**: 涉及资金安全的操作（如修改 `collection_address`、生成/刷新 API Key/Webhook Secret、修改密码、**提现**）必须经过 **Google Authenticator (TOTP)** 二次验证。
    - **登录 2FA 防爆破**: `POST /api/auth/verify-2fa` 对失败验证同时执行三层固定窗口限制：临时令牌 5 次/5 分钟、用户 10 次/15 分钟、客户端 IP 30 次/15 分钟。验证开始时会预占额度以阻止并发穿透；无效或过期临时令牌仍计入令牌与 IP，成功验证只清除用户和该令牌的失败计数，不清除 IP 聚合计数。所有层命中后统一返回 HTTP 429，不向客户端暴露命中的维度。
    - **多链地址校验**: `update_collection_address_with_2fa` 使用 `Network::validate_collection_address()` 按 chain family 分发校验（TRON: Base58Check / EVM: 0x + 40 hex chars）。
    - **`MerchantResponse.collection_addresses`**: Per-chain per-environment 的 HashMap (`{ "TRON": { "production": addr, "sandbox": addr }, "BSC": {...} }`)，向后兼容旧的 `collection_address` / `collection_address_sandbox` 字段。
    - **提现强制 2FA (M12)**: 提现端点 (`POST /withdrawals`) **强制要求** 2FA。若商户未启用 2FA，API 返回 `401 AuthError` 要求先开启；若已启用则必须携带 `totp_code`。此设计配合已有暴力破解保护（5 次失败/5 分钟锁定）确保出金安全。
    - **备份码 (Backup Codes)**: 系统在 2FA 设置时生成 8 个单次生效的备份码，用于 2FA 设备丢失后的紧急恢复。备份码以 SHA-256 哈希形式存储。
    - **密码管理**:
        - **自主修改**: 支持在线修改密码，若启用 2FA 则强制校验。
        - **邮件重置**: 采用 1 小时限时令牌（stateless JWT），配合 `token_version` 实现单次有效。支持防止用户枚举攻击（账户不存在时返回统一成功响应并引入时序噪音）。
    - **令牌撤销 (Token Revoked)**: 涉及密码变更或安全设置重大更新时，通过自增 `token_version` 实现全端 JWT 令牌即时重置。**Sandbox 环境跳过此检查** — 生产环境签发的 JWT 在沙箱中直接信任签名，避免因跨库 `token_version` 必然失同步而导致认证失败。
    - **邮箱验证**: 注册及敏感行为（如密码找回）需校验邮箱所有权。
- **JIT 影子账户 (Just-In-Time Shadow Account)**:
    - **问题**: 数据库级隔离后，商户在生产环境注册的账户在沙箱数据库中不存在。Dashboard 切换到 Test Mode 时 JWT 验证会因 "Merchant not found" 失败。
    - **方案**: `verify_token()` 在 Sandbox 模式下检测到商户不存在时，自动调用 `ensure_merchant_shadow()` 即时创建影子账户：
        - 使用与生产相同的 `merchant_id`（来自 JWT Claims）。
        - 填充占位 name/email/password_hash（不可登录，仅 JWT 访问）。
        - 自动创建 `merchant_chain_accounts` 行（按已启用的网络和当前环境），`email_verified: true`、`status: Active`。
        - **链账户在首次生成 API Key 时惰性初始化 (Lazy Init)**。
    - **并发安全**: 使用 `INSERT ... ON CONFLICT DO NOTHING`，5 个并发 Dashboard 请求同时触发 JIT 不会产生 500 错误。
- **注册流程优化 (Single-Environment Registration)**:
    - 商户注册时仅创建当前环境的 `merchant_chain_accounts` 行（Production 或 Sandbox），**不再同时创建两个环境的记录**。
    - 另一环境的记录由 JIT 机制在首次访问时自动创建。
- **API Key 管理**:
    - **环境隔离**: API Key 按环境类型区分：
        | 环境 | 可创建的 Key 类型 | Key 前缀 |
        | :--- | :--- | :--- |
        | Sandbox | Test Key | `sk_test_...` |
        | Production | Live Key | `sk_live_...` |
    - **前端强制**: Dashboard 根据当前环境自动限制可用的创建按钮，防止误操作。
    - **Key 生命周期**: 创建时一次性返回完整 Key，后续仅存储 SHA-256 哈希值。
- **Webhook 管理**:
    - **统一端点**: 每个商户配置一个 Webhook URL 和 Signing Secret。
    - **Secret 轮换**: 支持一键轮换 Secret，新 Secret 仅在轮换时显示一次。
    - **投递日志**: 记录所有 Webhook 发送历史，支持手动重发失败的通知。

---

## Address Manager (地址管理器)

- **职责**: 基于 Account xpub 派生收款地址，并管理地址池的分配。支持按 `Network` 选择正确的派生算法（TRON / EVM）。
- **托管模式**: 平台持有 Master Seed (由 AWS KMS 信封加密保护，详见 [security.md](../infrastructure/security.md))，为每个商户派生 Account-level xpub，可自动为归集交易签名。商户提供 `collection_address` 作为提现目标地址（见 [payout.md](payout.md)），归集目标为 `platform_treasury_address`。
- **多链派生路径**:
    | 链族 | 派生路径 | 地址格式 |
    | :--- | :--- | :--- |
    | TRON | `m/44'/195'/{account_index}'/0/{path_index}` | T 开头, Base58Check, 34 字符 |
    | EVM (BSC/ETH) | `m/44'/60'/{account_index}'/0/{path_index}` | 0x 开头, EIP-55 checksum, 42 字符 |

    | 层级 | 含义 |
    | :--- | :--- |
    | `44'` | BIP44 标准 |
    | `{coin_type}'` | 链族 coin_type (195=TRON, 60=EVM) |
    | `{account_index}'` | 商户的 `merchants.account_index` (自增整数, 硬化派生) |
    | `0` | External chain |
    | `{path_index}` | 该商户下的地址索引 (自增) |

- **EVM 地址通用性**: BSC 和 Ethereum 共享 `coin_type=60`，同一 merchant 的同一 xpub 派生出**完全相同**的地址。`merchant_chain_accounts` 表中，BSC 和 ETH 各存一行（不同 Network PK），但 `xpub_encrypted` 内容相同。
- **网络感知派生**:
    - `generate_addresses()` 通过 `derive_address(xpub, index, network)` dispatcher 按 `ChainFamily` 自动选择 `derive_tron_address` 或 `derive_evm_address`。
    - `initialize_merchant_addresses()` 使用 `get_account_xpub_for_coin(account_index, network.coin_type())` 确保为正确的 coin_type 派生 xpub。
- **Account xpub 存储**: 商户的 Account-level xpub 以 **AES-256-GCM 加密** 存储于 `merchant_chain_accounts.xpub_encrypted`。
    - **隔离性**: 每个 Environment + Network 组合都在 `merchant_chain_accounts` 中有独立的配置记录。
    - **派生流程**: 后台任务从 `xpub_encrypted` 解密后，使用 BIP32 非硬化派生生成收款地址。
    - **验证机制**: TRON 使用标准 BIP39 助记词 + TronWeb (JS) 交叉验证；EVM 使用 Ian Coleman BIP39 工具交叉验证。

- **并发策略**: 采用 **预生成池 (Pre-generation Pool)** 模式。
    - **池维护**: 系统通过后台任务预先派生地址，每个商户维护不少于 100 个 `Idle` 地址。
    - **原子抢占**: 下单时使用单条原子 SQL 完成地址分配，避免事务中途崩溃导致地址"丢失"：
      ```sql
      UPDATE addresses
      SET status = 'Assigned', updated_at = NOW()
      WHERE (network, address) = (
          SELECT network, address
          FROM addresses
          WHERE status = 'Idle' AND merchant_id = :merchant_id AND network = :network
          ORDER BY usdt_balance DESC
          LIMIT 1
          FOR UPDATE SKIP LOCKED
      )
      RETURNING network, address;
      ```
    - **批量插入**: 地址生成采用 SeaORM `insert_many()` 实现单次数据库往返，配合 `ON CONFLICT DO NOTHING` 处理重复。
    - **池耗尽降级**: 若 `RETURNING` 无结果（池内所有地址均被锁定或无 `Idle` 地址），返回 `HTTP 503 Service Unavailable`，并触发告警补充地址池。
    - **自动补充 (Auto-Replenishment)**:
        - **触发模式**: **Fire-and-Forget (异步触发)** — 每次 `create_session` 成功后，立即 spawn 后台任务检查并补充地址池。
        - **阈值配置**: 低水位阈值 (threshold) = 20，每次补充批量 (batch_size) = 50。
        - **防抖机制**: 使用 `Arc<DashSet<String>>` 记录正在补充的 `merchant_id:network:environment`，防止高并发下的重复补充（惊群效应）。
        - **Double-Check**: 获取锁后再次查询 `get_idle_count`，确认池确实低于阈值才执行。
        - **空闲计数**: `get_idle_count` 统计指定 network 上处于 `Idle` 状态的地址数量，并排除在其他 network 上已被占用的跨链共享地址（EVM 链共享同一地址集）。
        - **RAII 锁释放**: 使用 `scopeguard` 确保 DashSet 锁在任意退出路径（包括错误）时被正确释放。

- **地址用途**: 所有地址统一用于 Checkout 支付收款，归集目标为平台 `platform_treasury_address`。`address_type` 字段已在数据库级别移除（简化设计，当前仅有 Checkout 一种用途）。

---

## Email Service (邮件服务)

- **职责**: 发送事务性邮件（注册验证、密码重置），为商户注册和安全流程提供邮箱所有权校验。
- **架构**: Trait-based 可插拔后端。
    ```
    EmailSender (trait)
      ├── ResendEmailService  — 生产环境 (Resend API)
      └── DummyEmailService   — 开发/测试 (仅打印日志)
    ```
- **集成方式**: 通过 `MerchantService.with_email_service()` 注入，由 `main.rs` 根据 `RESEND_API_KEY` 环境变量自动选择后端。
- **触发场景**:
    | 场景 | 方法 | 触发方式 |
    | :--- | :--- | :--- |
    | 商户注册 | `send_verification_email` | `tokio::spawn` 异步发送，不阻塞注册响应 |
    | 重发验证 | `resend_verification_email` | 同步等待结果 |
    | 密码重置 | `send_password_reset_email` | 同步等待结果 |
- **Resend API 集成**:
    - **端点**: `POST https://api.resend.com/emails`
    - **认证**: `Authorization: Bearer re_xxx` (通过 `reqwest::Client::default_headers` 注入，标记为 `sensitive`)。
    - **幂等性**: 每次调用生成 UUID `Idempotency-Key` header，防止重试时重复发送（24h 过期）。
    - **重试策略**: 3 次重试 + 指数退避 (500ms → 1s → 2s)。
        - `5xx` 服务端错误 → 重试。
        - `429` Rate Limit → 以 2s 固定退避重试。
        - 其他 `4xx` → Fatal，立即返回错误。
    - **超时**: 10s per request。
- **安全措施**:
    - **Anti-Enumeration**: `resend_verification_email` 和 `send_password_reset_email` 始终返回 `Ok(())`，即使邮箱不存在，防止用户枚举攻击。
    - **Timing Attack Prevention**: 邮箱不存在时加入 50ms 延迟，消除基于响应时间的旁路攻击。
    - **Token 单次使用**: 密码重置 Token 绑定 `token_version`，使用后自增即失效。
- **邮件模板**: 内置 HTML 模板 (`templates.rs`)，包含品牌样式、CTA 按钮和 fallback 链接。
- **环境变量**:
    | 变量 | 必需 | 说明 |
    | :--- | :--- | :--- |
    | `RESEND_API_KEY` | 生产必需 | Resend API Key (缺失则降级为 DummyEmailService) |
    | `EMAIL_FROM` | 可选 | 发件地址 (默认 `onboarding@resend.dev`) |
    | `BASE_URL` | 可选 | 验证/重置链接的 base URL |
- **Webhook**: 当前阶段不接入 Resend Webhook。事务邮件量低，Fire-and-forget + 用户手动重发已足够。未来若引入交易通知邮件（高频），需接入 `email.bounced` / `email.complained` 事件保护域名信誉。
