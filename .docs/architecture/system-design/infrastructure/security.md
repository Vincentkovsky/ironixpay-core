# Security & Encryption + Master Key Management

> 📍 [返回架构目录](../README.md)

敏感凭据加密存储、Master Seed KMS 信封加密、IAM 权限最小化。

---

## 安全与加密策略 (Security & Encryption)

系统对敏感凭据采用以下加密存储方案，确保合规性与安全性：

| 字段 | 存储方式 | 算法 | 说明 |
| :--- | :--- | :--- | :--- |
| `api_keys.key_hash` | **哈希存储 (不可逆)** | SHA-256 | 原始 API Key 仅在创建时返回一次，后续仅存储哈希值用于验证。 |
| `webhook_endpoints.secret_encrypted` | **加密存储 (可逆)** | AES-256-GCM | Webhook Secret 需要在发送回调时解密使用，采用对称加密。 |
| `merchants.totp_secret` | **加密存储 (可逆)** | AES-256-GCM | TOTP 种子需解密后用于验证 OTP 码。 |
| `merchants.xpub_encrypted` | **加密存储 (可逆)** | AES-256-GCM | Account xpub 用于 HD 地址派生，解密后执行 BIP32 子密钥派生。 |
| `merchants.backup_codes` | **哈希存储 (不可逆)** | SHA-256 | 备份码哈希后的 JSON 数组，验证成功后标记为已使用。 |

---

## Master Key 安全 (AWS KMS Envelope Encryption)

- **架构**: 采用 **Seed 级加密 + 按需解密** 模式，BIP39 Seed (64 bytes) 由 AWS KMS 信封加密，存储为 Base64 密文。
    ```
    ┌──────────────────────────────────────────────────────────────────┐
    │  启动阶段                                                        │
    │  KmsEnvelopeProvider::new()                                      │
    │  ├─ Base64 → ciphertext (in-memory)                             │
    │  └─ 测试解密: 验证 IAM 权限 + Encryption Context → Fail-Fast     │
    ├──────────────────────────────────────────────────────────────────┤
    │  每次签名/派生请求                                                │
    │  with_seed(|seed| { ... })                                       │
    │  ├─ KMS Decrypt (per-request, ~50-100ms, TLS session reuse)     │
    │  ├─ 闭包内使用 seed 派生私钥/xpub                                │
    │  └─ Zeroize: seed + derived key 立即清零                         │
    └──────────────────────────────────────────────────────────────────┘
    ```
- **加密目标**: BIP39 Seed (64 bytes)，而非助记词。助记词**永不进入**生产服务器。
- **解密策略**: Per-request decrypt — Seed 在内存中仅存在 ~1ms（闭包执行期间），签名完成后立即 `Zeroize`。
- **Encryption Context**: 所有 KMS Decrypt 调用携带 `{"AppName": "IronixPay"}`，防止密文在其他 AWS 账号/环境被解密。
- **网络韧性**: 3x 指数退避重试 (100ms → 200ms → 400ms)，TLS Session 复用降低延迟。
- **Gas Sponsor Key**: 从 Master Seed HD 派生（`m/44'/195'/0'/0/1`），不再需要独立的加密密钥。
- **Sandbox 降级**: 无 `AWS_KMS_KEY_ID` 时自动回退至 `LocalMnemonicProvider`（明文助记词），适用于开发环境。
- **CLI 工具**: `cargo run --bin encrypt_secrets` 用于离线加密 Seed，输出 Base64 密文供 `.env` 使用。
- **环境变量 (Production)**:
    | 变量 | 说明 |
    | :--- | :--- |
    | `AWS_KMS_KEY_ID` | KMS Key ID 或 Alias (e.g. `alias/ironixpay-master`) |
    | `AWS_ACCESS_KEY_ID` | IAM User (仅 `kms:Decrypt` 权限) |
    | `AWS_SECRET_ACCESS_KEY` | IAM Secret |
    | `AWS_REGION` | KMS Region (e.g. `ap-southeast-1`) |
    | `ENCRYPTED_SEED` | KMS 加密后的 Base64 Seed (Gas Sponsor Key 从此 Seed HD 派生) |

---

## 密钥管理 (Key Management)

- **数据加密密钥 (DEK)**: AES-256-GCM 密钥 (`ENCRYPTION_KEY`)，用于加密 xpub、TOTP Secret 等业务数据。
- **Master Seed 保护**: 采用 **AWS KMS 信封加密** (见上方)。BIP39 Seed 由 KMS CMK 加密，存为 Base64 密文，运行时按需解密。
- **IAM 最小权限**: 后端 IAM User 仅授予 `kms:Decrypt`，加密操作通过离线 CLI 完成 (`kms:Encrypt`)。
- 禁止将 DEK 或 Seed 硬编码于代码或配置文件中。
