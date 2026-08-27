---
title: WooCommerce 接入 USDT/USDC 加密货币支付 — IronixPay
description: 零代码为 WordPress / WooCommerce 商店添加 USDT 和 USDC 稳定币支付，支持 TRON、BSC、ETH、Polygon、Arbitrum、Base、Optimism 等 8 条链，5 分钟完成安装。
---

# WooCommerce 接入 USDT & USDC 支付

零代码为你的 WordPress / WooCommerce 商店添加 USDT 和 USDC 稳定币支付，支持 8 条链，5 分钟完成安装。

## 为什么选择 IronixPay？

- **零代码安装** — WordPress 后台搜索插件、安装、填入 API Key 即可
- **双币种支持** — 同时接受 USDT（Tether）和 USDC（Circle）稳定币
- **8 条链覆盖** — TRON、Solana、BSC、Ethereum、Polygon、Arbitrum、Base、Optimism
- **无需银行账户** — 加密货币直接到你的钱包，跨境收款不受限制
- **完整兼容** — 支持 WooCommerce HPOS（高性能订单存储）和 Blocks Checkout

## 典型场景

跨境电商、数字产品售卖、独立站、Dropshipping — 任何使用 WooCommerce 的在线商店。

## 工作原理

<FlowChart>
  <Step icon="cart" title="客户下单">选择 Pay with Crypto</Step>
  <Step icon="chain" title="客户选择币种">USDT / USDC 和区块链网络</Step>
  <Step icon="api" title="WooCommerce 调用 IronixPay">创建支付会话</Step>
  <Step icon="redirect" title="客户跳转到 IronixPay 收银台" />
  <Step icon="send" title="客户用任意钱包转账稳定币" />
  <Step icon="scan" title="IronixPay 自动检测链上付款" />
  <Step icon="webhook" title="Webhook → WooCommerce 标记订单为「已支付」" />
  <Step icon="check" title="客户跳转回商店「订单成功」页面" />
</FlowChart>

## 安装步骤

### 1. 获取 API 密钥

在 [IronixPay 控制台](https://app.ironixpay.com) 注册账户并创建 API Key。

### 2. 安装插件

WordPress 后台 → 插件 → 添加新插件 → 搜索 "IronixPay" → 安装并启用。

### 3. 配置

WooCommerce → 设置 → 付款 → IronixPay：

| 设置 | 说明 |
|------|------|
| API Key | 你的 `sk_live_...` 或 `sk_test_...` 密钥 |
| 币种 | 选择 USDT、USDC 或两者都支持 |
| 网络 | 选择支持的区块链（7 条可选） |
| Sandbox Mode | 开启后使用测试环境（仅 TRON + USDT） |

填入密钥，选择币种和网络，保存即可。插件会自动注册 Webhook URL。

## 常见问题

### 支持哪些 WooCommerce 版本？

WooCommerce 7.0+ 及 WordPress 5.8+。完整支持 HPOS（高性能订单存储）和 Blocks Checkout。

### 支持哪些币种？

USDT（Tether）和 USDC（Circle），可以单独启用或同时启用。USDC 支持除 TRON 以外的所有网络。

### 客户需要安装什么吗？

不需要。客户在结账时选择 "Pay with Crypto"，选择币种和网络，跳转到 IronixPay 收银台，用任意加密钱包（TronLink、MetaMask 等）转账即可。

### 订单状态如何同步？

插件通过 Webhook 自动同步：支付成功 → 标记为 "Processing"；过期无人支付 → 标记为 "Failed"。完全自动，无需人工干预。

### 怎么测试？

启用 Sandbox Mode，使用 `sk_test_...` 密钥，在 TRON Nile 测试网测试（仅支持 USDT）。详见[测试指南](/guide/testing)。

## 开始使用

- [WordPress.org 插件页](https://wordpress.org/plugins/ironixpay-usdt-gateway/) — 一键安装
- [深度教程系列](https://dev.to/ironixpay/series/36293) — Dev.to 上的完整集成教程
- [快速开始](/guide/quickstart) — 注册账户、获取 API 密钥
- [Webhooks 指南](/guide/webhooks) — 了解支付通知机制
