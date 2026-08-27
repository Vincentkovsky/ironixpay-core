# Chain Abstraction Layer + Chains Configuration

> 📍 [返回架构目录](../README.md)

提供链无关的 trait 和类型，使 Indexer、Sweeper、Checkout 等业务模块无需关心具体链实现。

---

## Chain Abstraction Layer (多链抽象层)

- **职责**: 提供链无关的 trait 和类型，使 Indexer、Sweeper、Checkout 等业务模块无需关心具体链实现。
- **代码位置**: `services/chain/` (`mod.rs`, `traits.rs`, `types.rs`), `services/evm/` (EVM 实现)
- **核心设计**:
    - **`ChainClient` trait**: 定义链的读写交互接口（查余额、构建交易、广播、查状态、`validate_payment_tx`）。当前实现：`TronClient` (TRON)、`EvmClient` (所有 EVM 链)、`SolanaClient` (Solana)。
    - **`BlockScanner` trait**: 抽象区块扫描逻辑，使 `TransactionIndexer` 链无关。TRON 使用 TronGrid Event API 逐块扫描；EVM 使用 `eth_getLogs(fromBlock, toBlock)` 批量范围扫描；Solana 使用 `getSignaturesForAddress` + `getTransaction` 基于 slot 的签名扫描。
    - **`ChainSigner` trait**: 纯密码学签名（接收 32 字节 digest → 返回 65 字节签名）。与 ChainClient **职责分离** — client 不持有私钥。Solana 使用 ED25519 签名（64 字节），通过 `sign_transaction_for_coin(coin_type=501)` 调用。
    - **Enum Dispatch (非 trait object)**: `ChainUnsignedTx`, `ChainSignedTx` 等类型使用 enum 而非 `Box<dyn>` 实现零开销分发。每增加新链加一个 variant（需重编译，但编译期安全）。
    - **金额类型**: 使用 `alloy_primitives::U256` 统一处理所有链的金额（TRON 6 decimals, EVM 18 decimals），避免 i64 溢出。Solana SPL Token 使用 6 decimals (USDT/USDC)。
- **EVM Client (`services/evm/`)**:
    - **`mod.rs`**: 通用 EVM JSON-RPC 客户端，实现 `ChainClient` + `BlockScanner` trait。支持 `eth_getLogs` 批量扫描、`eth_getTransactionReceipt` 确认、`eth_sendRawTransaction` 广播。
    - **`gas_funder.rs`**: EVM 链的 Gas 赞助服务，从 HD 派生的 sponsor 地址向子地址转入原生代币 (BNB/ETH/POL) 以覆盖 Gas 费用。
    - **`signing.rs`**: EVM 交易签名（RLP 编码 + secp256k1）。使用 `coin_type=60` 从 Master Seed 派生私钥，`keccak256` 哈希。
- **Solana Client (`services/solana/`)**:
    - **`mod.rs`**: Solana JSON-RPC 客户端，支持多 RPC URL failover。核心方法：`get_spl_balance`、`get_sol_balance`、`broadcast_solana`、`get_signature_statuses`、`build_spl_sweep`、`build_spl_transfer`。
    - **`indexer.rs`**: Solana Indexer 实现。使用 `getSignaturesForAddress` 按 Mint ATA 扫描入账交易，逐笔解析 SPL Token `Transfer`/`TransferChecked` 指令。
    - **`sweep_executor.rs`**: `SolanaSweepExecutor` — 实现 `SweepExecutor` trait。构建双签名交易（子地址 owner + treasury fee payer），包含 `CreateATA(Idempotent)` + `TransferChecked` + `CloseAccount` 指令回收 ATA rent。
    - **Payout**: `SolanaPayoutExecutor` 定义在 `services/payout/executor.rs`（与 TRON/EVM executor 同文件）。使用 `SolanaClient::build_spl_transfer` 构建单签名 SPL 转账（treasury → 商户地址），包含 `CreateAssociatedTokenAccount` (幂等) + `TransferChecked` 指令。
    - **无 Gas Funder**: Solana 交易费由 fee payer (treasury) 直接支付 SOL，无需独立的 Gas 赞助机制。
    - **签名**: ED25519 签名，`coin_type=501`，通过 `TransactionSigner::sign_transaction_for_coin` 调用。Solana 交易使用原始消息字节签名（非 digest hash）。
- **Key Provider 设计**:
    - **`MasterKeyProvider`**: `get_account_xpub_for_coin(account_index, coin_type)` 为 **required method**，`get_account_xpub(index)` 为便捷 wrapper（默认 coin_type=195）。
    - **`TransactionSigner`**: `sign_transaction_for_coin(bytes, account_index, path_index, coin_type)` 为 **required method**，`sign_transaction(...)` 为便捷 wrapper。
    - **实现**: `LocalMnemonicProvider`（Dev）、`KmsEnvelopeProvider`（Prod）、`MockMasterKeyProvider`（Test）。
    - **哈希算法分派**: TRON 使用 SHA-256，EVM 使用 Keccak-256，Solana 使用 ED25519（签名原始消息字节，不做外部哈希）。由调用方根据 `ChainFamily` 选择。
- **`chain_clients` 注册表**: `AppState.chain_clients: HashMap<Network, Arc<dyn ChainClient>>` 在 `main.rs` 启动时根据 `chains.toml` 配置动态注册。TRON 始终注册，EVM 链按 `chains.toml` 中的配置按需注册，Solana 按 `[solana]` 配置段注册。
- **当前状态**: TRON + 全部 EVM 链 + Solana 均已实现。EVM 链通过 `spawn_evm_chain()` 统一启动；Solana 有独立的 Indexer/Sweeper 启动逻辑。共 8 条链全覆盖。

---

## Chains Configuration (`chains.toml`)

- **职责**: 声明式配置每条链的 RPC URL、Gas Sponsor 参数和出金费用。
- **文件路径**: `./chains.toml`（工作目录），可通过 `CHAINS_CONFIG_PATH` 环境变量覆盖。
- **加载逻辑**: `Config::load_chains()` 在 `from_env()` 中调用。若文件不存在，退化为 TRON-only 模式（向后兼容）。
- **示例结构**:
    ```toml
    [chains.TRON]
    rpc_url = "https://api.trongrid.io"

    [chains.BSC]
    rpc_urls = ["https://bsc-dataseed.binance.org", "https://bsc-dataseed1.defibit.io"]
    outbound_fee = 500000            # 0.5 USDT per payout (overrides flat_payout_fee)

    [chains.BSC.gas_sponsor]
    funding_amount_wei = "350000000000000"  # 0.00035 BNB

    [solana]
    rpc_urls = ["https://api.mainnet-beta.solana.com"]
    treasury_address = "<Base58 address>"
    outbound_fee = 500000            # 0.5 USDT per payout
    tokens = { USDT = "Es9vMF...", USDC = "EPjFWd..." }
    ```
    > [!NOTE]
    > Solana 配置在顶层 `[solana]` 段而非 `[chains.Solana]`，因为 Solana 不属于 EVM `ChainsConfig` 体系，有独立的 `SolanaConfig` 结构。

- **启用链推导**: `main.rs` 根据 `chains.toml` 中声明的链自动确定 `enabled_networks`。EVM 链调用 `spawn_evm_chain()` 启动独立的 Indexer + Sweeper；Solana 有独立的 `spawn_solana_indexer` + `spawn_solana_sweeper` 流程。
