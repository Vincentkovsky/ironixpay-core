---
title: Accept USDT Payments in Next.js / React — IronixPay
description: Add USDT payments to your Next.js or React app with embedded checkout or redirect flow. Full TypeScript support, 8 chains.
---

# Accept USDT Payments in Next.js / React

Add USDT payments to your Next.js or React application with an embedded checkout or redirect flow.

## Why IronixPay?

- **Embedded checkout** — iframe integrates seamlessly into your page, consistent brand experience
- **Full TypeScript** — `@ironix-pay/sdk` ships with complete type definitions and IDE autocomplete
- **API Route friendly** — Backend logic lives in Next.js API Routes, single project for frontend + backend

## Common Scenarios

SaaS subscriptions, digital product purchases, course sales, API credit top-ups — any web application in the React ecosystem.

## How It Works

<FlowChart>
  <Step icon="click" title="User clicks 'Upgrade to Pro'">Initiates payment from your React app</Step>
  <Step icon="api" title="React calls Next.js API Route">POST /api/checkout with plan details</Step>
  <Step icon="key" title="API Route creates Checkout Session">IronixPay returns session ID and URL</Step>
  <Step icon="wallet" title="SDK mounts embedded checkout">User sees payment UI within your page</Step>
  <Step icon="send" title="User completes USDT transfer">From any wallet, on any supported chain</Step>
  <Step icon="scan" title="IronixPay detects on-chain payment">Real-time confirmation via indexer</Step>
  <Step icon="webhook" title="Webhook pushes to API Route">Activates subscription in your database</Step>
</FlowChart>

## Quick Example

### Backend: API Route

```typescript
// app/api/checkout/route.ts
import { NextResponse } from 'next/server'

export async function POST(req: Request) {
  const { plan } = await req.json()

  const res = await fetch('https://api.ironixpay.com/v1/checkout/sessions', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${process.env.IRONIXPAY_SECRET_KEY}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      pricing_amount: plan === 'pro' ? '19.99' : '9.99',
      pricing_currency: 'USDT',
      currency: 'USDT',
      network: 'TRON',
      success_url: `${process.env.NEXT_PUBLIC_URL}/dashboard?upgraded=true`,
      cancel_url: `${process.env.NEXT_PUBLIC_URL}/pricing`,
    }),
  })

  return NextResponse.json(await res.json())
}
```

### Frontend: Embed Mode

```tsx
'use client'
import { IronixPay } from '@ironix-pay/sdk'
import { useEffect, useRef } from 'react'

export default function Checkout({ sessionId }: { sessionId: string }) {
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const ironix = new IronixPay({ environment: 'production' })
    const el = ironix.createPaymentElement({
      sessionId,
      theme: 'dark',
    })
    el.mount(ref.current!)

    el.on('payment_success', () => {
      // Show success UI; backend verifies via webhook
    })

    return () => el.unmount()
  }, [sessionId])

  return <div ref={ref} />
}
```

> [!TIP]
> You can also use redirect mode: simply `window.location.href = session.url` — zero SDK dependency. See [Frontend Integration](/en/guide/integration).

## FAQ

### Does it work with React / Vue / other frameworks?

The SDK is a framework-agnostic JavaScript package that works with any frontend framework. The example above uses Next.js, but Vue, Svelte, etc. work similarly.

### Embed mode vs. redirect mode?

| | Redirect Mode | Embed Mode |
|---|---|---|
| **Code** | Minimal (no SDK) | Requires `@ironix-pay/sdk` |
| **Experience** | Redirects to IronixPay page | Payment within your page |
| **Best for** | Quick start, MVP | Branded experience, SaaS |

### How do I verify a payment is real?

Always verify via [Webhook](/en/guide/webhooks) on the backend — don't rely solely on SDK events. Webhooks include a signature for tamper prevention.

### How do I test?

Use the Sandbox environment (`sk_test_...` key + `environment: 'sandbox'`) with the TRON Nile testnet. See the [Testing Guide](/en/guide/testing).

## Get Started

- [Next.js Starter Template](https://github.com/Vincentkovsky/ironixpay-core/tree/main/examples/nextjs-starter) — Ready-to-use complete example
- [Tutorial Series](https://dev.to/ironixpay/series/36293) — In-depth integration tutorials on Dev.to
- [Frontend Integration Guide](/en/guide/integration) — Embed & redirect modes in detail
- [Webhooks Guide](/en/guide/webhooks) — Set up payment notifications
- [npm: @ironix-pay/sdk](https://www.npmjs.com/package/@ironix-pay/sdk) — SDK installation & API
