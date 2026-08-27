# 测试与质量保证 (Testing & QA)

> 📍 [返回架构目录](README.md)

---

为确保金融级系统的资金安全与逻辑正确性，本项目采用**测试金字塔 (Test Pyramid)** 策略，覆盖从底层密码学单元到全链路业务流的完整验证。

## 单元测试 (Unit Tests)
*Focus: 核心组件的独立逻辑验证*
- **HD Wallet & Cryptography**:
  - 使用标准 BIP32/BIP44 向量验证私钥派生与地址生成，确保 0 偏差。
  - 验证 AES-256-GCM 加密/解密在各种边界条件下的正确性。
- **State Machine**:
  - 针对 `CheckoutSession` 和 `Address` 的所有状态流转进行穷举测试。
  - 验证 `Paid`, `Underpaid` (含 Rolling Extension), `Overpaid` 等复杂资金状态的判定逻辑。
- **Indexer Logic**:
  - 模拟各种异常链上事件（Fake Token, Dust Transfer, Reorg Block）验证解析器的健壮性。

## 属性测试 (Property-Based Tests)
*Focus: 自动模糊测试 (Fuzzing) 验证不变量*
- **金融不变量 (Financial Invariants)**: 确保任意存取序列下 `Total Swept <= Total Received`，绝不凭空创造资金。
- **地址生成**: 随机生成数万个 path_index，验证派生地址始终符合 Base58Check 格式与 TRON 规范。

## 端到端集成测试 (E2E Integration Tests)
*Focus: 真实环境下的全链路闭环*
- **Happy Path Lifecycle**:
  - `Merchant` -> `Session` -> `Payment (Mocked)` -> `Webhook` -> `Sweep`
  - 验证全流程资金流转、状态变更及数据库最终一致性。
- **Webhook Reliability**:
  - 模拟商户服务端 `HTTP 500` 错误，验证指数退避重试 (Exponential Backoff) 机制。
  - 验证 HMAC-SHA256 签名的正确性与安全性。

## 测试覆盖率 (Test Coverage)
系统核心模块（特别是 `services/sweeper`, `services/checkout`, `services/indexer`）均已实现高覆盖率的自动化测试，确保每一次代码变更不会引入回归缺陷 (Regression)。
