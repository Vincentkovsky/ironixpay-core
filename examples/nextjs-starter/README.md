# IronixPay × Next.js Starter

Accept USDT payments on 7 blockchains with [IronixPay](https://ironixpay.com) — in 5 minutes.

![Next.js](https://img.shields.io/badge/Next.js-15-black) ![TypeScript](https://img.shields.io/badge/TypeScript-5.6-blue) ![License](https://img.shields.io/badge/License-MIT-green)

## Quick Start

```bash
# 1. Clone
git clone https://github.com/Vincentkovsky/ironixpay-nextjs-starter.git
cd ironixpay-nextjs-starter

# 2. Configure
cp .env.example .env.local
# Edit .env.local → add your IronixPay API key (get one at https://ironixpay.com)

# 3. Run
npm install
npm run dev
# Open http://localhost:3000
```

## What's Inside

| File | Purpose |
|:---|:---|
| `src/lib/ironixpay.ts` | IronixPay API helper with TypeScript types |
| `src/app/api/checkout/route.ts` | Server-side checkout session creation |
| `src/app/api/webhooks/ironixpay/route.ts` | Webhook handler with HMAC-SHA256 verification |
| `src/app/page.tsx` | Demo store with product selection + network picker |
| `src/app/success/page.tsx` | Payment success page |
| `src/app/cancel/page.tsx` | Payment cancelled/expired page |

## How It Works

```
User clicks "Pay" → Your server creates a Checkout Session → User is redirected to IronixPay's hosted payment page → User pays with USDT → Webhook confirms payment → Success page shown
```

1. **Client** sends product + network to your API route (`/api/checkout`)
2. **Server** calls `POST /v1/checkout/sessions` with your secret key
3. **Client** redirects to `session.url` (IronixPay hosted checkout)
4. **IronixPay** shows QR code / address, monitors blockchain for payment
5. **Webhook** fires `checkout.session.completed` to your `/api/webhooks/ironixpay`
6. **User** is redirected to `/success`

## Supported Networks

| Network | Gas Fee | Speed |
|:---|:---|:---|
| TRON | ~$1 | ~3 sec |
| BNB Chain (BSC) | ~$0.10 | ~3 sec |
| Polygon | ~$0.01 | ~2 sec |
| Arbitrum | ~$0.10 | ~1 sec |
| Base | ~$0.05 | ~2 sec |
| Optimism | ~$0.05 | ~2 sec |
| Ethereum | ~$5 | ~15 sec |

## Environment Variables

| Variable | Required | Description |
|:---|:---|:---|
| `IRONIXPAY_SECRET_KEY` | ✅ | API key (`sk_test_...` for sandbox, `sk_live_...` for production) |
| `IRONIXPAY_API_URL` | No | API base URL (default: `https://sandbox.ironixpay.com`) |
| `IRONIXPAY_WEBHOOK_SECRET` | No | Webhook HMAC secret for signature verification |
| `NEXT_PUBLIC_APP_URL` | No | Your app URL for redirects (default: `http://localhost:3000`) |

## Going to Production

1. Get a production API key from [ironixpay.com](https://ironixpay.com)
2. Update `.env.local`:
   ```
   IRONIXPAY_SECRET_KEY=<your-production-api-key>
   IRONIXPAY_API_URL=https://api.ironixpay.com
   ```
3. Set up webhook URL in the IronixPay dashboard: `https://yourdomain.com/api/webhooks/ironixpay`

## Links

- [IronixPay Docs](https://ironixpay.com/guide/quickstart)
- [API Reference](https://ironixpay.com/guide/checkout)
- [JavaScript SDK](https://www.npmjs.com/package/@ironix-pay/sdk)

## License

Apache-2.0. See the repository's [LICENSE](../../LICENSE).
