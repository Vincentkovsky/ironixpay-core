---
title: Next.js / React 接入 USDT 支付 — IronixPay
description: 为 Next.js 或 React 应用接入 USDT 加密货币支付，支持嵌入式收银台和跳转模式，TypeScript 类型完整。
---

# Next.js / React 接入 USDT 支付

为你的 Next.js 或 React 应用接入 USDT 支付，支持嵌入式收银台和跳转模式。

## 为什么选择 IronixPay？

- **嵌入式体验** — iframe 无缝嵌入你的页面，用户无需跳转，品牌感一致
- **TypeScript 完整** — `@ironix-pay/sdk` 提供完整类型定义，IDE 自动补全
- **API Route 友好** — 后端逻辑放在 Next.js API Route，一个项目搞定前后端

## 典型场景

SaaS 订阅付费、数字产品一次性购买、课程售卖、API 额度充值 — 任何 React 生态的Web应用。

## 工作原理

<FlowChart>
  <Step icon="click" title="用户点击「升级 Pro」">在你的 React 应用中发起支付</Step>
  <Step icon="api" title="React 调用 Next.js API Route">POST /api/checkout 附带套餐信息</Step>
  <Step icon="key" title="API Route 创建 Checkout Session">IronixPay 返回 session ID 和 URL</Step>
  <Step icon="wallet" title="SDK 挂载嵌入式收银台">用户在页面内看到支付界面</Step>
  <Step icon="send" title="用户完成 USDT 转账">从任意钱包、任意支持的链</Step>
  <Step icon="scan" title="IronixPay 检测链上支付">通过索引器实时确认</Step>
  <Step icon="webhook" title="Webhook 推送到 API Route">在你的数据库中激活用户订阅</Step>
</FlowChart>

## 快速示例

### 后端：API Route

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

### 前端：嵌入模式

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
      // 展示成功 UI，后端通过 webhook 验证
    })

    return () => el.unmount()
  }, [sessionId])

  return <div ref={ref} />
}
```

> [!TIP]
> 也可以使用跳转模式：直接 `window.location.href = session.url`，零前端 SDK 依赖。详见[前端集成](/guide/integration)。

## 常见问题

### 支持 React / Vue / 其他框架吗？

SDK 是框架无关的纯 JavaScript 包，适用于任何前端框架。上面以 Next.js 为例，Vue、Svelte 等框架的接入方式类似。

### 嵌入模式和跳转模式怎么选？

| | 跳转模式 | 嵌入模式 |
|---|---|---|
| **代码量** | 最少（无需 SDK） | 需要 `@ironix-pay/sdk` |
| **体验** | 跳转到 IronixPay 页面 | 页面内完成支付 |
| **推荐** | 快速接入、MVP | 品牌化体验、SaaS 产品 |

### 怎么验证支付是否真实？

永远通过 [Webhook](/guide/webhooks) 在后端验证，不要仅依赖前端 SDK 事件。Webhook 携带签名，防篡改。

### 怎么测试？

使用 Sandbox 环境（`sk_test_...`密钥 + `environment: 'sandbox'`），在 TRON Nile 测试网免费测试。详见[测试指南](/guide/testing)。

## 开始使用

- [Next.js Starter 模板](https://github.com/Vincentkovsky/ironixpay-core/tree/main/examples/nextjs-starter) — 开箱即用的完整示例
- [深度教程系列](https://dev.to/ironixpay/series/36293) — Dev.to 上的完整集成教程
- [前端集成指南](/guide/integration) — 嵌入模式 & 跳转模式详解
- [Webhooks 指南](/guide/webhooks) — 配置支付通知
- [npm: @ironix-pay/sdk](https://www.npmjs.com/package/@ironix-pay/sdk) — SDK 安装 & API
