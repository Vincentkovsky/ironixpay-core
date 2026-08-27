# Network Isolation & Deployment (网络隔离与部署)

> 📍 [返回架构目录](../README.md)

确保不同区块链网络（Production/Sandbox）及不同链（TRON/BSC/Ethereum/Polygon/Arbitrum/Base/Optimism）的运行环境完全隔离，防止数据污染或私钥误用。

---

## 架构: 单进程单网络 (Single Network per Process) + 数据库级隔离

- 每个后端进程在启动时必须指定唯一的 `ENVIRONMENT`（Production 或 Sandbox）。
- **强隔离**: 进程启动时自动推导对应的 Network、RPC 节点和 USDT 合约地址。
- **Indexer 过滤**: `Indexer` 服务被注入 `Network` + `Environment`，通过 `chain_name()` 推导为内部标识（如 `TRON_MAINNET`、`BSC_MAINNET`），仅扫描和处理该网络的链上数据。
- **Sweeper 过滤**: 仅处理数据库中匹配当前网络的地址归集任务。

## 多链支持 (`Network` enum)

| Network | ChainFamily | coin_type | 链名示例 |
| :--- | :--- | :--- | :--- |
| `Tron` | `Tron` | 195 | `TRON_MAINNET` / `TRON_NILE` |
| `Bsc` | `Evm` | 60 | `BSC_MAINNET` / `BSC_TESTNET` |
| `Ethereum` | `Evm` | 60 | `ETHEREUM_MAINNET` / `ETHEREUM_SEPOLIA` |
| `Polygon` | `Evm` | 60 | `POLYGON_MAINNET` / `POLYGON_AMOY` |
| `Arbitrum` | `Evm` | 60 | `ARBITRUM_MAINNET` / `ARBITRUM_SEPOLIA` |
| `Base` | `Evm` | 60 | `BASE_MAINNET` / `BASE_SEPOLIA` |
| `Optimism` | `Evm` | 60 | `OPTIMISM_MAINNET` / `OPTIMISM_SEPOLIA` |

- **ChainFamily**: 同 family 的链共享 coin_type 和地址格式。EVM 链（BSC、Ethereum）共用 `coin_type=60`，同一 xpub 可派生完全相同的地址。

## 环境定义 (当前活跃)

- **Production**: `Network::Tron` + `Environment::Production` → 内部标识 `TRON_MAINNET`。
- **Sandbox**: `Network::Tron` + `Environment::Sandbox` → 内部标识 `TRON_NILE`。
- **Local**: 同 Sandbox，允许开发者本地调试。

## 数据库级隔离 (Database-Level Isolation)

同一 PostgreSQL 实例内维护两个独立数据库，共享计算资源但数据完全隔离。

| 数据库 | `DATABASE_URL` | 用途 |
| :--- | :--- | :--- |
| `ironixpay_prod` | `postgres://ironix:<pw>@db:5432/ironixpay_prod` | 生产环境 (主网) |
| `ironixpay_sandbox` | `postgres://ironix:<pw>@db:5432/ironixpay_sandbox` | 沙箱环境 (测试网) |

- **自动初始化**: `scripts/init-db.sh` 挂载为 Docker `entrypoint-initdb.d` — 首次启动 (空 Volume) 时自动创建 `ironixpay_sandbox` 数据库（`ironixpay_prod` 由 `POSTGRES_DB` 环境变量自动创建）。
- **隔离收益**:
    - 商户测试数据不会污染生产表，Sandbox 可随时重置。
    - 两套独立的 Schema Migration，Production 和 Sandbox 互不阻塞。
    - 各自独立的连接池与事务隔离。
