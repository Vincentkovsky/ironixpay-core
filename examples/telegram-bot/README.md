# IronixPay × Telegram Bot

Accept USDT payments in your Telegram Bot using [IronixPay](https://ironixpay.com).

Users pick a product → click "Pay Now" → complete payment on the IronixPay checkout page → bot sends a confirmation message.

## Quick Start

### 1. Create a Telegram Bot

Message [@BotFather](https://t.me/BotFather) on Telegram → `/newbot` → copy the token.

### 2. Setup

```bash
git clone https://github.com/Vincentkovsky/ironixpay-core.git
cd ironixpay-examples/telegram-bot
cp .env.example .env
npm install
```

Edit `.env`:
```env
BOT_TOKEN=123456:ABC-DEF...        # from @BotFather
IRONIXPAY_SECRET_KEY=sk_test_...   # from https://app.ironixpay.com
IRONIXPAY_API_URL=https://sandbox.ironixpay.com
WEBHOOK_PORT=3000
PUBLIC_URL=https://your-domain.com  # for webhook delivery
```

### 3. Expose webhook URL (local dev)

```bash
# In a separate terminal, use ngrok or similar:
npx ngrok http 3000
# Copy the https URL to PUBLIC_URL in .env
```

Then configure your webhook URL in the IronixPay dashboard:
```
https://your-ngrok-url.ngrok.app/webhooks/ironixpay
```

### 4. Run

```bash
npm run dev
```

Open Telegram → message your bot → `/buy` → select a product → click "Pay Now" 🎉

## Project Structure

```
src/
├── index.ts           # Bot commands + inline button handlers
├── ironixpay.ts       # IronixPay API helper + webhook verification
└── webhook-server.ts  # Express server for payment webhooks
```

## How It Works

```
User ──/buy──▶ Bot shows product list (inline keyboard)
             │
User ──tap──▶ Bot creates IronixPay session
             │
             ├──▶ Shows "Pay Now" button (links to checkout page)
             │
User ──pays──▶ IronixPay verifies on-chain
             │
IronixPay ──webhook──▶ Express server
             │
             └──▶ Bot sends "✅ Payment confirmed!" to user
```

## Key Files

| File | What it does |
|:-----|:-------------|
| `src/index.ts` | Bot commands: `/start`, `/buy`, `/help`. Handles inline button callbacks for product selection and payment creation. |
| `src/ironixpay.ts` | `createCheckoutSession()` — calls IronixPay API. `verifyWebhookSignature()` — HMAC-SHA256 verification. |
| `src/webhook-server.ts` | Express server on port 3000. Receives IronixPay webhooks, verifies signature, sends Telegram message to buyer. |

## Production Notes

- Replace in-memory `pendingOrders` Map with a real database (Redis, PostgreSQL, etc.)
- Switch to production credentials:
  ```env
  IRONIXPAY_SECRET_KEY=sk_live_...
  IRONIXPAY_API_URL=https://api.ironixpay.com
  ```
- Sandbox supports test networks. Hosted production currently supports eight networks.
- Consider using [grammY webhook mode](https://grammy.dev/guide/deployment-types#webhooks) for the bot itself in production.

## Resources

- [IronixPay Docs](https://ironixpay.com/guide/quickstart)
- [grammY Docs](https://grammy.dev)
- [Telegram](https://t.me/ironixpay)

## License

Apache-2.0. See the repository's [LICENSE](../../LICENSE).
