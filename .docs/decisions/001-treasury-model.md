# ADR: Treasury Model & Payout Architecture

> 决策时间：2026-02-24
> 状态：**Accepted**

## 背景

在 chains.toml 重构过程中，我们系统性地评估了三个架构问题：

1. Platform Treasury 还是 Per-Merchant Treasury？
2. Treasury 是否需要 Gnosis Safe 多签？
3. 开放商户出金 API 后架构是否需要调整？

## 决策

### 1. 保持 Platform Treasury（资金池模型）

**采用 Stripe Connect 式资金池**：所有商户资金归集到平台统一 treasury，商户余额通过数据库 ledger 记账。

```
入金: Customer → Collection Address → [Sweep] → Platform Treasury
出金: Merchant API → Platform Treasury → [Payout] → 目标地址
```

**否决 Per-Merchant Treasury 的理由：**

- **手续费扣除困难** — Sweep 全额到 merchant treasury 后，平台手续费需额外一笔 tx 扣回，gas 翻倍
- **流动性碎片化** — 资金分散在 N 个地址，无法跨商户调度流动性
- **虚假的安全感** — 平台持有所有 HD 派生私钥，隔离是经济层非密码学层
- **合规审计复杂** — 资金流分散到 N 条路径，AML 追踪成本倍增

### 2. Gnosis Safe：现阶段不集成

**核心矛盾：Safe 的多签审批流与自动化出金的即时性需求冲突。**

| 场景 | 需要多签 | Safe 适合 |
|------|---------|----------|
| 运营大额冷→热转账 | ✅ | ✅ |
| 商户 API 用户提现 | ❌ 要求即时 | ❌ |
| 商户结算 | ❌ | ⚠️ 过度设计 |

**未来路径：冷热分离架构（Phase 2 Roadmap）**

```
Cold Treasury (Safe/硬件钱包)
    │  手动补给（运营多签审批）
    ▼
Hot Payout Wallet (EOA, 限额)  ← API 出金从这里走
    │  自动签名，秒级
    ▼
目标地址
```

Hot wallet 限额 1-2 天出金量，被攻破损失可控。Safe 用在 Cold→Hot 补给环节。

### 3. 商户出金 API 不改变 Treasury 模型

开放 payout API 后仍使用 Platform Treasury。必须配套的风控措施：

- 商户级 ledger 余额校验（出金 ≤ 余额）
- 频率 + 单笔 + 日累计限额
- Hot wallet 低余额自动告警

## 当前 chains.toml 架构的兼容性

| 未来升级 | 是否需要代码改动 |
|---------|----------------|
| Treasury 换成 Safe 地址（sweep 端） | ❌ 改 TOML 即可 |
| 冷热分离（Hot Wallet 做 payout） | ❌ TOML `treasury_address` 填 hot wallet |
| Payout 从 Safe 出金（多签流程） | ✅ 需集成 Safe SDK，Phase 2 |

## 关于 HD 派生的澄清

**当前阶段 treasury 必须是 HD 派生的 EOA。** 原因：payout 操作从 treasury 签名发送 USDT，TransactionSigner 从 master seed 派生私钥。如果 treasury 不是 HD 派生的地址，就签不了 payout 交易。

| 地址 | 是否 HD 派生 | 原因 |
|------|------------|------|
| Treasury (TRON) | ✅ HD 派生 | Payout 签名需要对应私钥 |
| Treasury (BSC) | ✅ HD 派生 | 同上 |
| Gas Sponsor (TRON) | ✅ HD 派生 | m/44'/195'/0'/0/1，已从独立私钥统一 |
| Gas Sponsor (BSC) | ✅ HD 派生 | m/44'/60'/0'/0/1 |
| Collection Addresses | ✅ HD 派生 | 按商户 per-merchant 派生 |

### chains.toml 中地址的语义

- **TRON**: `treasury_address` 和 `gas_sponsor.address` 是**安全断言** — 启动时校验 HD 派生结果是否与声明一致，防止 seed 配错导致资金打入黑洞。
- **BSC**: 不在 TOML 中声明地址 — 纯 HD 派生。TRON 的断言已经验证了 seed 正确性，BSC 无需重复断言。

### 未来冷热分离后

```
Phase 0 (当前):  Sweep → Treasury(EOA) → Payout (同一地址收发)
Phase 2 (未来):  Sweep → Cold Safe     ← 不需要 HD，纯配置
                 Payout ← Hot Wallet(EOA) ← 独立地址，限额管理
                 补给: Cold Safe → Hot Wallet (运营多签审批)
```

冷热分离后 treasury 才能脱离 HD 派生、使用 Safe 合约地址。

---

## HD 派生路径规范

### 路径结构

```
m/44'/{coin_type}'/{account_index}'/0/{path_index}
```

| coin_type | 链 |
|-----------|-----|
| 195 | TRON |
| 60 | BSC / EVM |

### 地址分配

```
account_index = 0 (平台保留):
  path_index = 0  → Treasury (收 sweep + 发 payout)
  path_index = 1  → Gas Sponsor (付 gas/energy)
  path_index = 2+ → 预留 (未来 hot wallet 等)

account_index = 1,2,3... (商户):
  path_index = 0..N → 收款地址池 (checkout collection)
```

### 数据库约束

> [!IMPORTANT]
> 必须确认 Production DB 中无 `account_index = 0` 的商户，并添加约束：
> ```sql
> SELECT id, account_index FROM merchants WHERE account_index = 0;
> ALTER TABLE merchants ADD CONSTRAINT account_index_min CHECK (account_index >= 1);
> ```

---

## Action Items

| 优先级 | 任务 | 状态 |
|--------|------|------|
| P0 | chains.toml 重构 | ✅ 完成 |
| P0 | 查 production DB account_index=0 + 加约束 | ⬜ 待部署时执行 |
| P1 | TRON gas sponsor HD 派生统一（消除独立私钥） | ✅ 完成 |
| P1 | BSC 地址纯 HD 派生（消除 TOML 配置依赖） | ✅ 完成 |
| P2 | 冷热分离（Safe + Hot Wallet） | ⬜ 业务量上来后 |
