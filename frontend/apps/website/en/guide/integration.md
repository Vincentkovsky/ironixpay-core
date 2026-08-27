# Frontend Integration

IronixPay offers two integration modes. Choose the one that best fits your checkout flow.

| | Redirect Mode | Embed Mode |
|---|---|---|
| **Effort** | Minimal — no frontend SDK | Requires `@ironix-pay/sdk` |
| **UX** | Customer leaves your site | Customer stays on your page |
| **Best for** | Quick setup, mobile-first | Custom-branded experiences |

> [!TIP]
> Both modes use the same backend API to create a Checkout Session. The only difference is what you do with the `session` object on the frontend.

## Prerequisite: Create a Session (Server-Side)

Regardless of the mode, your **server** creates a Checkout Session first:

```javascript
// Node.js / Express example
app.post('/create-checkout', async (req, res) => {
  const response = await fetch('https://api.ironixpay.com/v1/checkout/sessions', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${process.env.IRONIXPAY_SECRET_KEY}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      pricing_amount: '10.50',       // 10.50 USDT
      pricing_currency: 'USDT',
      currency: 'USDT',
      network: 'TRON',
      success_url: 'https://yoursite.com/success',
      cancel_url: 'https://yoursite.com/cancel',
      client_reference_id: req.body.orderId  // optional
    })
  })

  const session = await response.json()
  res.json(session)
})
```

> [!IMPORTANT]
> Never expose your secret key (`sk_live_...` / `sk_test_...`) in client-side code. Always create sessions from your backend.

---

## Option A: Redirect Mode

The simplest integration — redirect the customer to IronixPay's hosted checkout page.

### How It Works

```
Customer clicks "Pay"
        │
        ▼
Your server creates session
        │
        ▼
Redirect to session.url ──▶ IronixPay Checkout
        │                         │
        │                  Payment complete
        │                         │
        ▼                         ▼
  cancel_url ◀── or ──▶ success_url
```

### Frontend Code

```javascript
// When customer clicks "Pay"
async function handlePayment() {
  // 1. Call your backend to create a session
  const res = await fetch('/create-checkout', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ orderId: 'order_123' })
  })
  const session = await res.json()

  // 2. Redirect to IronixPay checkout
  window.location.href = session.url
}
```

That's it. After payment, the customer is redirected to your `success_url` or `cancel_url`.

> [!NOTE]
> Always verify payment status via [Webhooks](/en/guide/webhooks) — don't rely solely on the redirect. A customer might close the browser before being redirected.

---

## Option B: Embed Mode (SDK)

Embed the checkout UI directly in your page using the `@ironix-pay/sdk`.

### Install

```bash
npm install @ironix-pay/sdk
```

Or via CDN:

```html
<script src="https://unpkg.com/@ironix-pay/sdk@0.1.0/dist/ironix-pay.umd.js"></script>
```

### How It Works

```
Customer clicks "Pay"
        │
        ▼
Your server creates session
        │
        ▼
Frontend receives sessionId
        │
        ▼
SDK mounts iframe in #checkout ──▶ Embedded Checkout
        │                                │
        │                         Payment events
        │                                │
        ▼                                ▼
el.on('payment_success')    el.on('payment_expired')
```

### Frontend Code

```javascript
import { IronixPay } from '@ironix-pay/sdk'

// 1. Initialize SDK
const ironixPay = new IronixPay({
  environment: 'sandbox'              // or 'production'
})

// 2. Call your backend to create a session
const res = await fetch('/create-checkout', { method: 'POST', ... })
const session = await res.json()

// 3. Create and mount the payment element
const element = ironixPay.createPaymentElement({
  sessionId: session.id,
  theme: 'light',                     // 'light' | 'dark' | 'auto'
  locale: 'en'                        // 'en' | 'zh-CN'
})

element.mount('#checkout-container')

// 4. Listen for events
element.on('ready', () => {
  console.log('Checkout loaded')
})

element.on('payment_success', (result) => {
  console.log('Payment received!', result.transactionHash)
  // Show success UI, verify on your server via webhook
})

element.on('payment_expired', ({ sessionId }) => {
  console.log('Session expired:', sessionId)
  // Show retry UI
})

element.on('error', (error) => {
  console.error('Checkout error:', error.code, error.message)
})
```

### HTML

```html
<!-- The SDK will inject an iframe here -->
<div id="checkout-container"></div>
```

### Cleanup

When navigating away or unmounting:

```javascript
element.unmount()
```

### SDK Reference

#### `new IronixPay(options)`

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `environment` | `'production' \| 'sandbox'` | `'production'` | API environment |
| `publicKey` | `string` | — | Public API key (optional, reserved for future use) |
| `checkoutUrl` | `string` | — | Custom checkout URL for self-hosted deployments |

#### `ironixPay.createPaymentElement(options)`

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `sessionId` | `string` | *required* | Session ID from your backend |
| `theme` | `'light' \| 'dark' \| 'auto'` | `'dark'` | UI theme |
| `locale` | `'en' \| 'zh-CN'` | `'en'` | Locale |
| `style.width` | `string` | `'100%'` | Element width |
| `style.height` | `string` | `'480px'` | Initial element height (auto-resizes) |
| `style.borderRadius` | `string` | `'12px'` | Border radius |

#### Events

| Event | Callback | Description |
|-------|----------|-------------|
| `ready` | `() => void` | Checkout iframe loaded and ready |
| `payment_success` | `(result: PaymentResult) => void` | Payment confirmed on-chain |
| `payment_detected` | `(data) => void` | Transaction detected, awaiting confirmation |
| `payment_expired` | `(data) => void` | Session expired |
| `error` | `(error: PaymentError) => void` | Error occurred |
| `resize` | `(data: { height }) => void` | Iframe content height changed |

#### `PaymentResult`

```typescript
{
  sessionId: string
  status: 'Paid' | 'Overpaid'
  amountReceived: number
  transactionHash?: string
}
```

---

## Supported Networks

See [Supported Networks](/en/guide/networks) for the full list of chains, USDT contract addresses, and sandbox availability.

## Next Steps

- [Webhooks](/en/guide/webhooks) — Handle payment notifications
- [Testing](/en/guide/testing) — Sandbox testing guide
- [Errors](/en/guide/errors) — Error handling
- [Live Demo](/en/demo) — Try both modes interactively
