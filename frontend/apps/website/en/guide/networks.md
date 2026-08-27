# Supported Networks

IronixPay supports stablecoin payments (USDT & USDC) across 8 blockchain networks in production, with TRON Nile testnet available for sandbox development.

## Production (`sk_live_`)

| Network | USDT | USDC | Explorer |
|---------|------|------|----------|
| **TRON** | [`TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t`](https://tronscan.org/#/token20/TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t) | — | [Tronscan](https://tronscan.org) |
| **BSC** | [`0x55d398326f99059fF775485246999027B3197955`](https://bscscan.com/token/0x55d398326f99059fF775485246999027B3197955) | [`0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d`](https://bscscan.com/token/0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d) | [BscScan](https://bscscan.com) |
| **Ethereum** | [`0xdAC17F958D2ee523a2206206994597C13D831ec7`](https://etherscan.io/token/0xdAC17F958D2ee523a2206206994597C13D831ec7) | [`0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`](https://etherscan.io/token/0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48) | [Etherscan](https://etherscan.io) |
| **Polygon** | [`0xc2132D05D31c914a87C6611C10748AEb04B58e8F`](https://polygonscan.com/token/0xc2132D05D31c914a87C6611C10748AEb04B58e8F) | [`0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359`](https://polygonscan.com/token/0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359) | [PolygonScan](https://polygonscan.com) |
| **Arbitrum** | [`0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9`](https://arbiscan.io/token/0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9) | [`0xaf88d065e77c8cC2239327C5EDb3A432268e5831`](https://arbiscan.io/token/0xaf88d065e77c8cC2239327C5EDb3A432268e5831) | [Arbiscan](https://arbiscan.io) |
| **Optimism** | [`0x94b008aA00579c1307B0EF2c499aD98a8ce58e58`](https://optimistic.etherscan.io/token/0x94b008aA00579c1307B0EF2c499aD98a8ce58e58) | [`0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85`](https://optimistic.etherscan.io/token/0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85) | [OP Etherscan](https://optimistic.etherscan.io) |
| **Base** | [`0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2`](https://basescan.org/token/0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2) | [`0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913`](https://basescan.org/token/0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913) | [BaseScan](https://basescan.org) |
| **Solana** | [`Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB`](https://solscan.io/token/Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB) | [`EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v`](https://solscan.io/token/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v) | [Solscan](https://solscan.io) |

::: tip USDC on TRON
USDC is **not available** on TRON. If you need USDC, use Solana, BSC, or any EVM chain.
:::

## Sandbox (`sk_test_`)

| Network | Tokens | Contract | Explorer |
|---------|--------|----------|----------|
| **TRON Nile** | USDT (TRC-20 testnet) | [`TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf`](https://nile.tronscan.org/#/token20/TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf) | [Nile Tronscan](https://nile.tronscan.org) |

::: tip Getting Test Tokens
Get free TRX and test USDT from the [Nile Faucet](https://nileex.io/join/getJoinPage). You'll need a small amount of TRX for gas fees before sending USDT.
:::

::: warning Sandbox Limitation
Sandbox currently only supports **TRON** (Nile testnet). Passing other networks with a `sk_test_` key will return an error. More sandbox networks are planned.
:::

## How `network` Works

Pass the network value (e.g. `"TRON"`, `"BSC"`) when [creating a session](/en/guide/checkout). The API automatically selects mainnet or testnet based on your API key:

- `sk_live_` + `network: "TRON"` → TRON Mainnet
- `sk_test_` + `network: "TRON"` → TRON Nile (testnet)
- `sk_live_` + `network: "BSC"` → BSC Mainnet

```bash
curl -X POST https://api.ironixpay.com/v1/checkout/sessions \
  -H "Authorization: Bearer $IRONIXPAY_SECRET_KEY" \
  -H "Content-Type: application/json" \
  -d '{"pricing_amount": "10.50", "pricing_currency": "USDT", "currency": "USDT", "network": "BSC", ...}'
```
