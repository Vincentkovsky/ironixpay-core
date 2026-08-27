# 支持的网络

IronixPay 生产环境支持 8 条区块链网络的稳定币（USDT & USDC）收付款，沙盒环境支持 TRON Nile 测试网。

## 生产环境（`sk_live_`）

| 网络 | USDT | USDC | 浏览器 |
|------|------|------|--------|
| **TRON** | [`TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t`](https://tronscan.org/#/token20/TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t) | — | [Tronscan](https://tronscan.org) |
| **BSC** | [`0x55d398326f99059fF775485246999027B3197955`](https://bscscan.com/token/0x55d398326f99059fF775485246999027B3197955) | [`0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d`](https://bscscan.com/token/0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d) | [BscScan](https://bscscan.com) |
| **Ethereum** | [`0xdAC17F958D2ee523a2206206994597C13D831ec7`](https://etherscan.io/token/0xdAC17F958D2ee523a2206206994597C13D831ec7) | [`0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`](https://etherscan.io/token/0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48) | [Etherscan](https://etherscan.io) |
| **Polygon** | [`0xc2132D05D31c914a87C6611C10748AEb04B58e8F`](https://polygonscan.com/token/0xc2132D05D31c914a87C6611C10748AEb04B58e8F) | [`0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359`](https://polygonscan.com/token/0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359) | [PolygonScan](https://polygonscan.com) |
| **Arbitrum** | [`0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9`](https://arbiscan.io/token/0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9) | [`0xaf88d065e77c8cC2239327C5EDb3A432268e5831`](https://arbiscan.io/token/0xaf88d065e77c8cC2239327C5EDb3A432268e5831) | [Arbiscan](https://arbiscan.io) |
| **Optimism** | [`0x94b008aA00579c1307B0EF2c499aD98a8ce58e58`](https://optimistic.etherscan.io/token/0x94b008aA00579c1307B0EF2c499aD98a8ce58e58) | [`0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85`](https://optimistic.etherscan.io/token/0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85) | [OP Etherscan](https://optimistic.etherscan.io) |
| **Base** | [`0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2`](https://basescan.org/token/0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2) | [`0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913`](https://basescan.org/token/0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913) | [BaseScan](https://basescan.org) |
| **Solana** | [`Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB`](https://solscan.io/token/Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB) | [`EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v`](https://solscan.io/token/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v) | [Solscan](https://solscan.io) |

::: tip USDC 与 TRON
TRON 网络 **不支持 USDC**。如需使用 USDC，请选择 Solana、BSC 或其他 EVM 链。
:::

## 沙盒环境（`sk_test_`）

| 网络 | 代币 | 合约 | 浏览器 |
|------|------|------|--------|
| **TRON Nile** | USDT (TRC-20 测试网) | [`TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf`](https://nile.tronscan.org/#/token20/TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf) | [Nile Tronscan](https://nile.tronscan.org) |

::: tip 获取测试代币
从 [Nile Faucet](https://nileex.io/join/getJoinPage) 免费领取 TRX 和测试 USDT。发送 USDT 前需要少量 TRX 作为 gas。
:::

::: warning 沙盒限制
沙盒目前仅支持 **TRON**（Nile 测试网）。使用 `sk_test_` 密钥传入其他网络会返回错误。更多沙盒网络正在规划中。
:::

## `network` 参数机制

[创建会话](/guide/checkout)时传入 network 值（如 `"TRON"`、`"BSC"`），API 根据 API Key 自动选择主网或测试网：

- `sk_live_` + `network: "TRON"` → TRON 主网
- `sk_test_` + `network: "TRON"` → TRON Nile 测试网
- `sk_live_` + `network: "BSC"` → BSC 主网

```bash
curl -X POST https://api.ironixpay.com/v1/checkout/sessions \
  -H "Authorization: Bearer $IRONIXPAY_SECRET_KEY" \
  -H "Content-Type: application/json" \
  -d '{"pricing_amount": "10.50", "pricing_currency": "USDT", "currency": "USDT", "network": "BSC", ...}'
```
