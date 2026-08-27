---
title: Accept USDT Payments in Telegram Bot — IronixPay
description: Add USDT payments to your Telegram Bot in 5 minutes. Support TRON, BSC, ETH, and 5 more chains with automatic webhook confirmation.
---

# Accept USDT Payments in Your Telegram Bot

Add USDT payments to your Telegram Bot in 5 minutes. Support TRON, BSC, ETH, and 5 more chains.

## Why IronixPay?

- **No frontend needed** — One API call generates a payment link, send it to the user
- **Auto-confirmation** — Webhook pushes payment results in real-time, bot delivers automatically
- **8 chains** — Users can pay via TRON (lowest fees), BSC, ETH, and more — one integration covers all

## Common Scenarios

Digital product sales, membership/subscription top-ups, paid group access, game item purchases — any scenario where your Telegram Bot needs to collect payments.

## How It Works

<FlowChart>
  <Step icon="bot" title="User sends /pay command" />
  <Step icon="api" title="Bot backend calls IronixPay API">Creates a Checkout Session</Step>
  <Step icon="redirect" title="Returns session.url">Bot sends payment link</Step>
  <Step icon="click" title="User clicks link, pays on checkout page" />
  <Step icon="scan" title="IronixPay detects on-chain transfer" />
  <Step icon="webhook" title="Webhook pushes to your backend" />
  <Step icon="check" title="Bot sends Payment successful">Delivers product</Step>
</FlowChart>

## Quick Example

Minimal example using [grammY](https://grammy.dev/):

```typescript
import { Bot } from 'grammy'

const bot = new Bot(process.env.BOT_TOKEN!)

bot.command('pay', async (ctx) => {
  // Create an IronixPay checkout session
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

  await ctx.reply(`💰 Click to pay:\n${session.url}`)
})
```

> [!TIP]
> The `client_reference_id` field is returned as-is in the Webhook payload — use it to link Telegram users to orders.

## FAQ


### Does the bot need to handle on-chain logic?

No. On-chain monitoring, address allocation, and payment confirmation are all handled by IronixPay. You just receive Webhook notifications.

### How do I test?

Use the Sandbox environment (`sk_test_...` key) with the TRON Nile testnet for free end-to-end testing. See the [Testing Guide](/en/guide/testing).

## Get Started

- [Telegram Bot Starter Template](https://github.com/Vincentkovsky/ironixpay-core/tree/main/examples/telegram-bot) — Ready-to-use complete example
- [Tutorial Series](https://dev.to/ironixpay/series/36293) — In-depth integration tutorials on Dev.to
- [Quick Start](/en/guide/quickstart) — Create account & get API keys
- [Webhooks Guide](/en/guide/webhooks) — Set up payment notifications
