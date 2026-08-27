---
title: Telegram Bot 接入 USDT 支付 — IronixPay
description: 5 分钟让你的 Telegram Bot 接受 USDT 加密货币支付，支持 TRON、BSC、ETH 等 8 条链，Webhook 自动确认。
---

# Telegram Bot 接入 USDT 支付

让你的 Telegram Bot 在 5 分钟内接受 USDT 支付，支持 TRON、BSC、ETH 等 8 条链。

## 为什么选择 IronixPay？

- **无需前端** — 后端一个 API 调用生成支付链接，发送给用户即可
- **自动确认** — Webhook 实时推送支付结果，Bot 自动发货/激活
- **8 链覆盖** — 用户可用 TRON（低手续费）、BSC、ETH 等链支付，一套接口全搞定

## 典型场景

数字产品售卖、会员/订阅充值、付费群入群、游戏道具购买 — 任何 Telegram Bot 需要收款的场景。

## 工作原理

<FlowChart>
  <Step icon="bot" title="用户发送 /pay 命令" />
  <Step icon="api" title="Bot 后端调用 IronixPay API">创建 Checkout Session</Step>
  <Step icon="redirect" title="返回 session.url">Bot 发送支付链接</Step>
  <Step icon="click" title="用户点链接，在收银台支付" />
  <Step icon="scan" title="IronixPay 检测到链上转账" />
  <Step icon="webhook" title="Webhook 推送到你的后端" />
  <Step icon="check" title="Bot 发送「支付成功」消息">发货</Step>
</FlowChart>

## 快速示例

使用 [grammY](https://grammy.dev/) 框架的最小示例：

```typescript
import { Bot } from 'grammy'

const bot = new Bot(process.env.BOT_TOKEN!)

bot.command('pay', async (ctx) => {
  // 调用 IronixPay 创建支付会话
  const res = await fetch('https://api.ironixpay.com/v1/checkout/sessions', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${process.env.IRONIXPAY_SECRET_KEY}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      pricing_amount: '5',        // 5 USDT
      pricing_currency: 'USDT',
      currency: 'USDT',
      network: 'TRON',
      client_reference_id: `tg_${ctx.from?.id}`,
      success_url: 'https://t.me/your_bot',
      cancel_url: 'https://t.me/your_bot',
    }),
  })
  const session = await res.json()

  await ctx.reply(`💰 请点击以下链接完成支付：\n${session.url}`)
})
```

> [!TIP]
> `client_reference_id` 字段会在 Webhook 中原样返回，用来关联 Telegram 用户和订单。

## 常见问题

### 支持哪些链？

TRON、Solana、BSC、ETH、Polygon、Arbitrum、Optimism、Base — 共 8 条链。推荐默认使用 TRON（手续费最低，约 1 USDT）。

### Bot 需要处理链上逻辑吗？

不需要。链上监听、地址分配、到账确认全部由 IronixPay 处理，你只需接收 Webhook 通知。

### 怎么测试？

使用 Sandbox 环境（`sk_test_...` 密钥），在 TRON Nile 测试网免费测试完整流程。详见[测试指南](/guide/testing)。

## 开始使用

- [Telegram Bot Starter 模板](https://github.com/Vincentkovsky/ironixpay-core/tree/main/examples/telegram-bot) — 开箱即用的完整示例
- [深度教程系列](https://dev.to/ironixpay/series/36293) — Dev.to 上的完整集成教程
- [快速开始](/guide/quickstart) — 注册账户、获取 API 密钥
- [Webhooks 指南](/guide/webhooks) — 配置支付通知
