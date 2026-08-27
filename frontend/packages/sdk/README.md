# @ironix-pay/sdk

[![npm](https://img.shields.io/npm/v/@ironix-pay/sdk)](https://www.npmjs.com/package/@ironix-pay/sdk)
[![npm bundle size](https://img.shields.io/bundlephobia/minzip/@ironix-pay/sdk)](https://bundlephobia.com/package/@ironix-pay/sdk)
[![TypeScript](https://img.shields.io/badge/TypeScript-Ready-blue)](https://www.typescriptlang.org/)

Embed the IronixPay Checkout directly into your page. No redirects — users pay without leaving your site.

```
Your Page                    IronixPay
  │                             │
  │── SDK mounts iframe ──────▶│
  │                             │── Shows QR + address
  │                             │◀── User sends USDT
  │◀── payment_success event ──│
  │── Fulfill order             │
```

## Install

```bash
npm install @ironix-pay/sdk
```

Or via CDN:
```html
<script src="https://unpkg.com/@ironix-pay/sdk@latest/dist/ironix-pay.umd.js"></script>
```

## Quick Start

### 1. Create a session (server-side)

```typescript
// Your backend — Node.js / Express / Next.js API route
const session = await fetch('https://api.ironixpay.com/v1/checkout/sessions', {
  method: 'POST',
  headers: {
    Authorization: `Bearer ${process.env.IRONIXPAY_SECRET_KEY}`,
    'Content-Type': 'application/json',
  },
  body: JSON.stringify({
    amount: '9.99',           // 9.99 USDT (human-readable string)
    currency: 'USDT',
    network: 'TRON',       // or BSC, ETHEREUM, POLYGON, ARBITRUM, OPTIMISM, BASE
    success_url: 'https://my-site.com/success',
    cancel_url: 'https://my-site.com/cancel',
  }),
});

const { id } = await session.json();
// Send `id` to your frontend
```

### 2. Embed checkout (client-side)

```typescript
import { IronixPay } from '@ironix-pay/sdk';

const ironixPay = new IronixPay({ environment: 'production' });

const element = ironixPay.createPaymentElement({
  sessionId: 'cs_abc123',   // from your backend
  theme: 'dark',             // 'light' | 'dark' | 'auto'
  locale: 'en',             // 'en' | 'zh-CN'
});

element.mount('#checkout-container');

element.on('payment_success', (result) => {
  console.log('Paid!', result.amountReceived, result.transactionHash);
  // ⚠️ Verify via webhook before fulfilling!
});

element.on('payment_expired', () => {
  console.log('Session expired');
});

element.on('error', (err) => {
  console.error(err.code, err.message);
});
```

## Framework Examples

### React

```tsx
import { useEffect, useRef } from 'react';
import { IronixPay } from '@ironix-pay/sdk';

export function Checkout({ sessionId }: { sessionId: string }) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const ironixPay = new IronixPay({ environment: 'production' });
    const element = ironixPay.createPaymentElement({ sessionId, theme: 'dark' });

    element.mount(containerRef.current!);
    element.on('payment_success', (result) => {
      window.location.href = `/success?session=${result.sessionId}`;
    });

    return () => element.unmount();
  }, [sessionId]);

  return <div ref={containerRef} />;
}
```

### Vue 3

```vue
<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { IronixPay, type CheckoutElement } from '@ironix-pay/sdk';

const props = defineProps<{ sessionId: string }>();
const container = ref<HTMLDivElement>();
let element: CheckoutElement | null = null;

onMounted(() => {
  const ironixPay = new IronixPay({ environment: 'production' });
  element = ironixPay.createPaymentElement({ sessionId: props.sessionId, theme: 'dark' });
  element.mount(container.value!);

  element.on('payment_success', (result) => {
    window.location.href = `/success?session=${result.sessionId}`;
  });
});

onUnmounted(() => element?.unmount());
</script>

<template>
  <div ref="container" />
</template>
```

### CDN / Vanilla JS

```html
<div id="checkout"></div>
<script src="https://unpkg.com/@ironix-pay/sdk@latest/dist/ironix-pay.umd.js"></script>
<script>
  const ironixPay = new IronixPay.default({ environment: 'production' });
  const element = ironixPay.createPaymentElement({
    sessionId: 'cs_abc123',
    theme: 'dark'
  });
  element.mount('#checkout');

  element.on('payment_success', function(result) {
    alert('Payment confirmed: ' + result.amountReceived + ' USDT');
  });
</script>
```

## API Reference

### `new IronixPay(options?)`

| Option | Type | Default | Description |
|:-------|:-----|:--------|:------------|
| `environment` | `'production' \| 'sandbox'` | `'production'` | API environment |
| `checkoutUrl` | `string` | — | Custom checkout URL (self-hosted) |

### `ironixPay.createPaymentElement(options)`

| Option | Type | Required | Default | Description |
|:-------|:-----|:---------|:--------|:------------|
| `sessionId` | `string` | ✅ | — | Session ID from your backend |
| `theme` | `'light' \| 'dark' \| 'auto'` | — | `'dark'` | UI theme |
| `locale` | `'en' \| 'zh-CN'` | — | `'en'` | Language |
| `style.width` | `string` | — | `'100%'` | Element width |
| `style.height` | `string` | — | `'480px'` | Initial height (auto-resizes) |
| `style.borderRadius` | `string` | — | `'12px'` | Corner radius |

### `element.mount(target)`

Mount to a CSS selector (`'#checkout'`) or DOM element.

### `element.unmount()`

Clean up iframe, listeners, and timers. **Always call in cleanup** (React `useEffect` return, Vue `onUnmounted`).

### `element.on(event, callback)` / `element.off(event, callback)`

| Event | Payload | When |
|:------|:--------|:-----|
| `ready` | — | Checkout iframe loaded |
| `payment_success` | `PaymentResult` | Payment confirmed on-chain |
| `payment_detected` | `{ sessionId, amountReceived }` | Transaction seen (pending confirmation) |
| `payment_expired` | `{ sessionId }` | Session timed out |
| `error` | `PaymentError` | Load timeout or runtime error |
| `resize` | `{ height }` | iframe content height changed |

#### `PaymentResult`

```typescript
{
  sessionId: string;
  status: 'Paid' | 'Overpaid';
  amountReceived: number;       // human-readable (e.g. 10.5)
  transactionHash?: string;
}
```

#### `PaymentError`

```typescript
{
  code: 'LOAD_TIMEOUT' | 'SESSION_NOT_FOUND' | 'NETWORK_ERROR' | 'UNKNOWN';
  message: string;
}
```

## Security

> ⚠️ **Never rely on frontend events for order fulfillment!**
>
> `payment_success` is for UI updates only. Always verify payments via [backend webhooks](https://ironixpay.com/guide/quickstart) before shipping products or granting access.

## Supported Networks

TRON, Solana, BSC, Ethereum, Polygon, Arbitrum, Optimism, and Base.

The network is set when creating the session on your backend. The SDK automatically displays the correct chain.

## Resources

- 📖 [Documentation](https://ironixpay.com/guide/quickstart)
- [Examples](https://github.com/Vincentkovsky/ironixpay-core/tree/main/examples)
- 💬 [Telegram](https://t.me/ironixpay)

## License

Apache-2.0. See the repository's [LICENSE](../../../LICENSE).
