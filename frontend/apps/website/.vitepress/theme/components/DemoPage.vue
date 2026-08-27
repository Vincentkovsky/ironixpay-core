<template>
  <div class="demo-page">
    <!-- Hero -->
    <section class="demo-hero">
      <h1 class="demo-hero__title">Try IronixPay <span class="gradient-text">Live</span></h1>
      <p class="demo-hero__subtitle">
        {{ isZh ? '在沙盒环境中体验两种集成模式，无需注册' : 'Experience both integration modes in sandbox. No sign-up required.' }}
      </p>
    </section>

    <!-- Two Panels -->
    <section class="demo-panels">
      <!-- Redirect Mode -->
      <div class="demo-card">
        <div class="demo-card__header">
          <div class="demo-card__icon">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
          </div>
          <div>
            <h3 class="demo-card__title">{{ isZh ? '跳转模式' : 'Redirect Mode' }}</h3>
            <p class="demo-card__desc">{{ isZh ? '将客户跳转到我们托管的收银台页面' : 'Redirect customers to our hosted checkout page' }}</p>
          </div>
        </div>

        <!-- Order Summary -->
        <div class="demo-order">
          <div class="demo-order__item">
            <span class="demo-order__emoji">☕</span>
            <span class="demo-order__name">{{ isZh ? '开发者咖啡' : 'Developer Coffee' }}</span>
            <span class="demo-order__price">5 USDT</span>
          </div>
        </div>

        <button class="demo-btn demo-btn--primary" :disabled="redirectLoading" @click="handleRedirect">
          <svg v-if="redirectLoading" class="demo-btn__spinner" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
          <template v-else>
            {{ isZh ? '支付 5 USDT' : 'Pay 5 USDT' }}
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="m12 5 7 7-7 7"/></svg>
          </template>
        </button>

        <!-- Code Snippet -->
        <div class="demo-code">
          <div class="demo-code__header">
            <span class="demo-code__lang">JavaScript</span>
          </div>
          <pre class="demo-code__body"><code><span class="code-comment">// {{ isZh ? '创建收银台会话（后端调用）' : 'Create a checkout session (server-side)' }}</span>
<span class="code-keyword">const</span> res = <span class="code-keyword">await</span> <span class="code-fn">fetch</span>(<span class="code-string">'https://api.ironixpay.com/v1/checkout/sessions'</span>, {
  method: <span class="code-string">'POST'</span>,
  headers: {
    <span class="code-string">'Authorization'</span>: <span class="code-string">'Bearer sk_live_...'</span>,
    <span class="code-string">'Content-Type'</span>: <span class="code-string">'application/json'</span>
  },
  body: <span class="code-fn">JSON.stringify</span>({
    pricing_amount: <span class="code-string">'5'</span>,
    pricing_currency: <span class="code-string">'USDT'</span>,
    currency: <span class="code-string">'USDT'</span>,
    network: <span class="code-string">'TRON'</span>,
    success_url: <span class="code-string">'https://yoursite.com/success'</span>,
    cancel_url: <span class="code-string">'https://yoursite.com/cancel'</span>
  })
})
<span class="code-keyword">const</span> session = <span class="code-keyword">await</span> res.<span class="code-fn">json</span>()

<span class="code-comment">// {{ isZh ? '跳转到收银台' : 'Redirect to checkout' }}</span>
window.location = session.url</code></pre>
        </div>
      </div>

      <!-- Embed Mode -->
      <div class="demo-card">
        <div class="demo-card__header">
          <div class="demo-card__icon">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2"/><path d="M3 9h18"/><path d="M9 21V9"/></svg>
          </div>
          <div>
            <h3 class="demo-card__title">{{ isZh ? '嵌入模式' : 'Embed Mode' }}</h3>
            <p class="demo-card__desc">{{ isZh ? '将支付界面直接嵌入你的页面' : 'Embed the payment UI directly in your page' }}</p>
          </div>
        </div>

        <!-- Live Checkout Preview -->
        <div class="demo-preview">
          <!-- Live iframe (shown after creating session) -->
          <div v-if="embedSessionId" class="demo-preview__widget demo-preview__widget--live">
            <iframe
              :src="`${CHECKOUT_URL}/checkout/${embedSessionId}?embed=1&theme=${isDark ? 'dark' : 'light'}`"
              class="demo-embed-iframe"
              frameborder="0"
              allow="clipboard-write"
            ></iframe>
          </div>
          <!-- Static preview (shown by default) -->
          <div v-else class="demo-preview__widget">
            <!-- Blue top bar -->
            <div class="preview-topbar"></div>
            <div class="preview-content">
              <!-- Timer pill -->
              <div class="preview-timer">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
                <span>14:59</span>
              </div>
              <!-- QR Code placeholder -->
              <div class="preview-qr">
                <svg width="80" height="80" viewBox="0 0 80 80">
                  <rect width="80" height="80" rx="4" fill="#f8fafc" stroke="#e2e8f0"/>
                  <rect x="8" y="8" width="24" height="24" rx="2" fill="#1e293b"/>
                  <rect x="12" y="12" width="16" height="16" rx="1" fill="#f8fafc"/>
                  <rect x="15" y="15" width="10" height="10" fill="#1e293b"/>
                  <rect x="48" y="8" width="24" height="24" rx="2" fill="#1e293b"/>
                  <rect x="52" y="12" width="16" height="16" rx="1" fill="#f8fafc"/>
                  <rect x="55" y="15" width="10" height="10" fill="#1e293b"/>
                  <rect x="8" y="48" width="24" height="24" rx="2" fill="#1e293b"/>
                  <rect x="12" y="52" width="16" height="16" rx="1" fill="#f8fafc"/>
                  <rect x="15" y="55" width="10" height="10" fill="#1e293b"/>
                  <rect x="36" y="36" width="8" height="8" fill="#1e293b"/>
                  <rect x="48" y="36" width="4" height="8" fill="#1e293b"/>
                  <rect x="56" y="40" width="8" height="4" fill="#1e293b"/>
                  <rect x="48" y="52" width="12" height="4" fill="#1e293b"/>
                  <rect x="64" y="48" width="8" height="8" fill="#1e293b"/>
                  <rect x="64" y="60" width="8" height="8" fill="#1e293b"/>
                </svg>
              </div>
              <!-- Address field -->
              <div class="preview-field">
                <span class="preview-field__label">{{ isZh ? '钱包地址' : 'WALLET ADDRESS' }}</span>
                <div class="preview-field__value">
                  <span class="preview-field__mono">TXrk...7vFq</span>
                  <span class="preview-field__copy">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect width="14" height="14" x="8" y="8" rx="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>
                  </span>
                </div>
              </div>
              <!-- Amount field -->
              <div class="preview-field">
                <span class="preview-field__label">{{ isZh ? '发送金额' : 'AMOUNT TO SEND' }}</span>
                <div class="preview-field__value">
                  <span class="preview-field__mono">5.000000 <span class="preview-field__unit">USDT</span></span>
                  <span class="preview-field__copy">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect width="14" height="14" x="8" y="8" rx="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>
                  </span>
                </div>
              </div>
              <!-- Footer -->
              <div class="preview-footer">
                <div class="preview-footer__brand">
                  <span class="preview-footer__logo">IX</span>
                  Powered by <span class="preview-footer__name">IronixPay</span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <button class="demo-btn demo-btn--primary" :disabled="embedLoading" @click="handleEmbed">
          <svg v-if="embedLoading" class="demo-btn__spinner" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
          <template v-else>
            {{ embedSessionId ? (isZh ? '重新创建' : 'Reset') : (isZh ? '试试嵌入模式' : 'Try Embed Mode') }}
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="m12 5 7 7-7 7"/></svg>
          </template>
        </button>

        <!-- Code Snippet -->
        <div class="demo-code">
          <div class="demo-code__header">
            <span class="demo-code__lang">JavaScript</span>
          </div>
          <pre class="demo-code__body"><code><span class="code-keyword">import</span> { IronixPay } <span class="code-keyword">from</span> <span class="code-string">'@ironix-pay/sdk'</span>

<span class="code-keyword">const</span> ip = <span class="code-keyword">new</span> <span class="code-fn">IronixPay</span>({ environment: <span class="code-string">'sandbox'</span> })
<span class="code-keyword">const</span> el = ip.<span class="code-fn">createPaymentElement</span>({
  sessionId: <span class="code-string">'cs_...'</span>,
  theme: <span class="code-string">'light'</span>,
  locale: <span class="code-string">'en'</span>
})
el.<span class="code-fn">mount</span>(<span class="code-string">'#checkout'</span>)
el.<span class="code-fn">on</span>(<span class="code-string">'payment_success'</span>, (result) => {
  console.<span class="code-fn">log</span>(<span class="code-string">'Paid!'</span>, result.transactionHash)
})</code></pre>
        </div>
      </div>
    </section>

    <!-- CTA Banner -->
    <section class="demo-cta">
      <h2 class="demo-cta__title">{{ isZh ? '准备好接受 USDT 了吗？' : 'Ready to accept USDT?' }}</h2>
      <p class="demo-cta__desc">{{ isZh ? '5 分钟完成集成，开始收款' : 'Integrate in 5 minutes and start accepting payments' }}</p>
      <div class="demo-cta__actions">
        <a
          :href="isZh ? '/guide/quickstart' : '/en/guide/quickstart'"
          class="demo-btn demo-btn--outline"
          data-analytics-event="cta_click"
          data-analytics-name="quickstart"
          data-analytics-location="demo_cta"
        >
          {{ isZh ? '阅读文档' : 'Read the Docs' }}
        </a>
        <a
          href="https://app.ironixpay.com"
          target="_blank"
          class="demo-btn demo-btn--primary demo-btn--cta"
          data-analytics-event="cta_click"
          data-analytics-name="create_account"
          data-analytics-location="demo_cta"
        >
          {{ isZh ? '免费创建账户' : 'Create Free Account' }}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="m12 5 7 7-7 7"/></svg>
        </a>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useData } from 'vitepress'

const { lang, isDark } = useData()
const isZh = computed(() => lang.value === 'zh-CN')

const redirectLoading = ref(false)
const embedLoading = ref(false)
const embedSessionId = ref('')

const SANDBOX_API = import.meta.env.VITE_API_BASE_URL || 'https://sandbox.ironixpay.com'
const DEMO_API_KEY = import.meta.env.VITE_DEMO_API_KEY || ''
const CHECKOUT_URL = import.meta.env.VITE_CHECKOUT_URL || 'https://pay-sandbox.ironixpay.com'

async function createSession() {
  if (!DEMO_API_KEY) {
    throw new Error('VITE_DEMO_API_KEY is required to enable the website demo')
  }

  const res = await fetch(`${SANDBOX_API}/v1/checkout/sessions`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${DEMO_API_KEY}`,
      'X-Environment': 'sandbox',
    },
    body: JSON.stringify({
      pricing_amount: '5',
      pricing_currency: 'USDT',
      currency: 'USDT',
      network: 'TRON',
      client_reference_id: `website_demo_${Date.now()}`,
      success_url: `${window.location.origin}${isZh.value ? '' : '/en'}/demo?result=success`,
      cancel_url: `${window.location.origin}${isZh.value ? '' : '/en'}/demo?result=cancelled`,
    }),
  })
  if (!res.ok) throw new Error('Failed to create session')
  return await res.json()
}

async function handleRedirect() {
  redirectLoading.value = true
  try {
    const session = await createSession()
    window.location.href = session.url
  } catch (e) {
    console.error('Demo session creation failed:', e)
    alert(isZh.value ? '创建会话失败，请稍后再试' : 'Failed to create session. Please try again later.')
  } finally {
    redirectLoading.value = false
  }
}

async function handleEmbed() {
  embedLoading.value = true
  try {
    const session = await createSession()
    embedSessionId.value = session.id
  } catch (e) {
    console.error('Demo embed creation failed:', e)
    alert(isZh.value ? '创建会话失败，请稍后再试' : 'Failed to create session. Please try again later.')
  } finally {
    embedLoading.value = false
  }
}
</script>
