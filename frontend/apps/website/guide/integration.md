# 前端集成

IronixPay 提供两种集成模式，选择最适合你结账流程的方式。

| | 跳转模式 | 嵌入模式 |
|---|---|---|
| **工作量** | 最少 — 无需前端 SDK | 需要 `@ironix-pay/sdk` |
| **体验** | 跳转到 IronixPay 页面 | 在你的页面内完成支付 |
| **适用** | 快速接入、移动端优先 | 品牌化定制体验 |

> [!TIP]
> 两种模式使用相同的后端 API 创建 Checkout Session，区别仅在前端的处理方式。

## 前置步骤：创建会话（后端）

无论哪种模式，都需要你的**后端**先创建 Checkout Session：

```javascript
// Node.js / Express 示例
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
      client_reference_id: req.body.orderId  // 可选
    })
  })

  const session = await response.json()
  res.json(session)
})
```

> [!IMPORTANT]
> 切勿在前端暴露密钥（`sk_live_...` / `sk_test_...`）。始终从后端创建会话。

---

## 方案 A：跳转模式

最简单的接入方式 — 将客户跳转到 IronixPay 在线收银页面。

### 流程

```
客户点击"支付"
      │
      ▼
你的后端创建会话
      │
      ▼
跳转到 session.url ──▶ IronixPay 收银台
      │                       │
      │                  支付完成
      │                       │
      ▼                       ▼
 cancel_url ◀── 或 ──▶ success_url
```

### 前端代码

```javascript
// 客户点击"支付"时
async function handlePayment() {
  // 1. 调用你的后端创建会话
  const res = await fetch('/create-checkout', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ orderId: 'order_123' })
  })
  const session = await res.json()

  // 2. 跳转到 IronixPay 收银台
  window.location.href = session.url
}
```

搞定。支付后客户会被跳转回 `success_url` 或 `cancel_url`。

> [!NOTE]
> 始终通过 [Webhooks](/guide/webhooks) 验证支付状态 — 不要仅依赖跳转。客户可能在跳转前关闭浏览器。

---

## 方案 B：嵌入模式（SDK）

使用 `@ironix-pay/sdk` 将收银台 UI 嵌入到你的页面中。

### 安装

```bash
npm install @ironix-pay/sdk
```

或通过 CDN：

```html
<script src="https://unpkg.com/@ironix-pay/sdk@0.1.0/dist/ironix-pay.umd.js"></script>
```

### 流程

```
客户点击"支付"
      │
      ▼
你的后端创建会话
      │
      ▼
前端获得 sessionId
      │
      ▼
SDK 在 #checkout 挂载 iframe ──▶ 嵌入式收银台
      │                               │
      │                          支付事件
      │                               │
      ▼                               ▼
el.on('payment_success')   el.on('payment_expired')
```

### 前端代码

```javascript
import { IronixPay } from '@ironix-pay/sdk'

// 1. 初始化 SDK
const ironixPay = new IronixPay({
  environment: 'sandbox'              // 或 'production'
})

// 2. 调用后端创建会话
const res = await fetch('/create-checkout', { method: 'POST', ... })
const session = await res.json()

// 3. 创建并挂载支付元素
const element = ironixPay.createPaymentElement({
  sessionId: session.id,
  theme: 'light',                     // 'light' | 'dark' | 'auto'
  locale: 'zh-CN'                     // 'en' | 'zh-CN'
})

element.mount('#checkout-container')

// 4. 监听事件
element.on('ready', () => {
  console.log('收银台已加载')
})

element.on('payment_success', (result) => {
  console.log('支付成功！', result.transactionHash)
  // 展示成功 UI，通过 webhook 在后端验证
})

element.on('payment_expired', ({ sessionId }) => {
  console.log('会话已过期:', sessionId)
  // 展示重试 UI
})

element.on('error', (error) => {
  console.error('收银台错误:', error.code, error.message)
})
```

### HTML

```html
<!-- SDK 会在这里注入 iframe -->
<div id="checkout-container"></div>
```

### 清理

页面跳转或组件卸载时：

```javascript
element.unmount()
```

### SDK 参考

#### `new IronixPay(options)`

| 选项 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `environment` | `'production' \| 'sandbox'` | `'production'` | API 环境 |
| `publicKey` | `string` | — | 公开 API 密钥（可选，预留） |
| `checkoutUrl` | `string` | — | 自部署的 checkout URL |

#### `ironixPay.createPaymentElement(options)`

| 选项 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `sessionId` | `string` | *必填* | 后端获取的会话 ID |
| `theme` | `'light' \| 'dark' \| 'auto'` | `'dark'` | UI 主题 |
| `locale` | `'en' \| 'zh-CN'` | `'en'` | 语言 |
| `style.width` | `string` | `'100%'` | 元素宽度 |
| `style.height` | `string` | `'480px'` | 初始高度（自动调整） |
| `style.borderRadius` | `string` | `'12px'` | 圆角半径 |

#### 事件

| 事件 | 回调 | 说明 |
|------|------|------|
| `ready` | `() => void` | 收银台 iframe 加载完毕 |
| `payment_success` | `(result: PaymentResult) => void` | 链上支付确认 |
| `payment_detected` | `(data) => void` | 检测到交易，等待确认 |
| `payment_expired` | `(data) => void` | 会话已过期 |
| `error` | `(error: PaymentError) => void` | 发生错误 |
| `resize` | `(data: { height }) => void` | iframe 内容高度变化 |

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

## 支持的网络

查看[支持的网络](/guide/networks)了解完整的链列表、USDT 合约地址和沙盒可用性。

## 下一步

- [Webhooks](/guide/webhooks) — 处理支付通知
- [测试](/guide/testing) — 沙盒测试指南
- [错误处理](/guide/errors) — 错误处理
- [在线体验](/demo) — 交互式体验两种模式
