---
title: Accept USDT & USDC Payments in WooCommerce — IronixPay
description: Accept USDT & USDC stablecoin payments across 8 chains — TRON, BSC, ETH, Polygon, Arbitrum, Optimism, and Base. Low fees, instant settlement, auto-sweeping to your treasury.
---

# Accept USDT & USDC Payments in WooCommerce

Accept USDT and USDC stablecoin payments in your WordPress / WooCommerce store with zero code. Install in 5 minutes, support 8 blockchains.

## Why IronixPay?

- **Zero-code setup** — Search, install, and configure from the WordPress admin panel
- **Multi-currency** — Accept both USDT (Tether) and USDC (Circle) stablecoins
- **8 chains** — TRON, Solana, BSC, Ethereum, Polygon, Arbitrum, Base, Optimism
- **No bank account needed** — Crypto goes directly to your wallet, no border restrictions
- **Full compatibility** — Supports WooCommerce HPOS (High-Performance Order Storage) and Blocks Checkout

## Common Scenarios

Cross-border e-commerce, digital product sales, independent stores, dropshipping — any online store powered by WooCommerce.

## How It Works

<FlowChart>
  <Step icon="cart" title="Customer places order">Selects Pay with Crypto</Step>
  <Step icon="chain" title="Customer picks currency">USDT / USDC and blockchain network</Step>
  <Step icon="api" title="WooCommerce calls IronixPay">Creates a Checkout Session</Step>
  <Step icon="redirect" title="Customer is redirected to IronixPay checkout" />
  <Step icon="send" title="Customer transfers stablecoin from any wallet" />
  <Step icon="scan" title="IronixPay detects on-chain payment" />
  <Step icon="webhook" title="Webhook → WooCommerce marks order as Processing" />
  <Step icon="check" title="Customer redirected to Order Complete page" />
</FlowChart>

## Installation

### 1. Get Your API Key

Register at [IronixPay Dashboard](https://app.ironixpay.com) and create an API Key.

### 2. Install the Plugin

WordPress Admin → Plugins → Add New → Search "IronixPay" → Install & Activate.

### 3. Configure

WooCommerce → Settings → Payments → IronixPay:

| Setting | Description |
|---------|-------------|
| API Key | Your `sk_live_...` or `sk_test_...` key |
| Currencies | USDT, USDC, or both |
| Networks | Choose from 8 supported blockchains |
| Sandbox Mode | Toggle for testing environment (TRON + USDT only) |

Enter your key, select currencies and networks, save, and you're live. The plugin auto-registers the Webhook URL.

## FAQ

### Which WooCommerce versions are supported?

WooCommerce 7.0+ and WordPress 5.8+. Full support for HPOS (High-Performance Order Storage) and Blocks Checkout.

### What currencies can I accept?

USDT (Tether) and USDC (Circle). You can enable one or both. USDC is available on all networks except TRON.

### Does the customer need to install anything?

No. Customers select "Pay with Crypto" at checkout, choose a currency and network, get redirected to the IronixPay checkout page, and pay with any crypto wallet (TronLink, MetaMask, etc.).

### How does order status sync?

The plugin auto-syncs via Webhook: payment confirmed → "Processing"; expired with no payment → "Failed". Fully automatic, no manual intervention.

### How do I test?

Enable Sandbox Mode, use `sk_test_...` key, and test on the TRON Nile testnet (USDT only). See the [Testing Guide](/en/guide/testing).

## Get Started

- [WordPress.org Plugin Page](https://wordpress.org/plugins/ironixpay-usdt-gateway/) — One-click install
- [Tutorial Series](https://dev.to/ironixpay/series/36293) — In-depth integration tutorials on Dev.to
- [Quick Start](/en/guide/quickstart) — Create account & get API keys
- [Webhooks Guide](/en/guide/webhooks) — Understand payment notifications
