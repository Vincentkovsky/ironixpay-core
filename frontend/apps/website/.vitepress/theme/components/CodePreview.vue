<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { useData } from 'vitepress'

const { lang } = useData()
const isZh = computed(() => lang.value === 'zh-CN')
const prefix = computed(() => (isZh.value ? '' : '/en'))

const t = computed(() =>
  isZh.value
    ? {
        badge: '开发者友好',
        title: '一次 API 调用，开始收款',
        subtitle: '不到 10 行代码即可创建一个完整的 Checkout Session。我们处理地址派生、链上监听、资金归集——你只需关注业务。',
        cta: '查看完整文档',
        tabApi: 'cURL',
        tabSdk: 'JavaScript',
      }
    : {
        badge: 'Developer Friendly',
        title: 'One API call to start accepting payments',
        subtitle: 'Create a complete Checkout Session in under 10 lines. We handle address derivation, on-chain monitoring, and fund sweeping — you focus on your product.',
        cta: 'Read the full docs',
        tabApi: 'cURL',
        tabSdk: 'JavaScript',
      },
)

const activeTab = ref('curl')

const curlCode = `curl -X POST https://api.ironixpay.com/v1/checkout/sessions \\
  -H "Authorization: Bearer sk_test_..." \\
  -H "Content-Type: application/json" \\
  -d '{
    "pricing_amount": "10.50",
    "pricing_currency": "USD",
    "currency": "USDT",
    "network": "TRON",
    "success_url": "https://yoursite.com/success",
    "cancel_url": "https://yoursite.com/cancel"
  }'`

const nodeCode = `const response = await fetch(
  'https://api.ironixpay.com/v1/checkout/sessions',
  {
    method: 'POST',
    headers: {
      'Authorization': \`Bearer \${process.env.IRONIXPAY_SECRET_KEY}\`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      pricing_amount: '10.50',
      pricing_currency: 'USD',
      currency: 'USDT',
      network: 'TRON',
      success_url: 'https://yoursite.com/success',
      cancel_url: 'https://yoursite.com/cancel',
    }),
  }
);

const session = await response.json();
res.redirect(303, session.url);`

const sectionRef = ref<HTMLElement | null>(null)
const isVisible = ref(false)
let observer: IntersectionObserver | null = null

onMounted(() => {
  const mq = window.matchMedia('(prefers-reduced-motion: reduce)')
  if (mq.matches) { isVisible.value = true; return }
  observer = new IntersectionObserver(
    ([e]) => { if (e.isIntersecting) { isVisible.value = true; observer?.disconnect() } },
    { threshold: 0.12 },
  )
  if (sectionRef.value) observer.observe(sectionRef.value)
})

onUnmounted(() => observer?.disconnect())
</script>

<template>
  <section ref="sectionRef" class="ix-code" :class="{ 'ix-code--visible': isVisible }">
    <div class="ix-code__inner">
      <div class="ix-code__text">
        <span class="ix-code__badge">{{ t.badge }}</span>
        <h2 class="ix-code__title">{{ t.title }}</h2>
        <p class="ix-code__subtitle">{{ t.subtitle }}</p>
        <a :href="`${prefix}/guide/quickstart`" class="ix-code__cta">
          {{ t.cta }}
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M3 8h10m0 0L9 4m4 4L9 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
        </a>
      </div>

      <div class="ix-code__terminal">
        <div class="ix-code__terminal-bar">
          <div class="ix-code__dots">
            <span /><span /><span />
          </div>
          <div class="ix-code__tabs">
            <button
              class="ix-code__tab"
              :class="{ 'ix-code__tab--active': activeTab === 'curl' }"
              @click="activeTab = 'curl'"
            >{{ t.tabApi }}</button>
            <button
              class="ix-code__tab"
              :class="{ 'ix-code__tab--active': activeTab === 'node' }"
              @click="activeTab = 'node'"
            >{{ t.tabSdk }}</button>
          </div>
        </div>
        <pre class="ix-code__pre"><code>{{ activeTab === 'curl' ? curlCode : nodeCode }}</code></pre>
      </div>
    </div>
  </section>
</template>

<style scoped>
.ix-code {
  padding: 80px 24px;
}

.ix-code__inner {
  max-width: 1100px;
  margin: 0 auto;
  display: grid;
  grid-template-columns: 1fr 1.2fr;
  gap: 48px;
  align-items: center;
}

/* ─── Text side ─── */
.ix-code__text {
  opacity: 0;
  transform: translateX(-20px);
}

.ix-code--visible .ix-code__text {
  opacity: 1;
  transform: translateX(0);
  transition: opacity 0.6s ease-out, transform 0.6s ease-out;
}

.ix-code__badge {
  display: inline-block;
  font-family: 'Exo 2', sans-serif;
  font-size: 0.75rem;
  font-weight: 600;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: #2563eb;
  margin-bottom: 14px;
}

.ix-code__title {
  font-family: 'Exo 2', sans-serif;
  font-size: 2rem;
  font-weight: 800;
  letter-spacing: -0.03em;
  line-height: 1.2;
  color: #0f172a;
  margin: 0 0 16px;
}

.ix-code__subtitle {
  font-size: 0.95rem;
  line-height: 1.7;
  color: #64748b;
  margin: 0 0 28px;
}

.ix-code__cta {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 0.9rem;
  font-weight: 600;
  color: #2563eb;
  text-decoration: none;
  transition: gap 0.2s ease;
}

.ix-code__cta:hover {
  gap: 10px;
}

/* ─── Terminal ─── */
.ix-code__terminal {
  border-radius: 16px;
  overflow: hidden;
  background: #0f172a;
  border: 1px solid rgba(255, 255, 255, 0.08);
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.15), 0 0 0 1px rgba(255, 255, 255, 0.05);
  opacity: 0;
  transform: translateX(20px);
}

.ix-code--visible .ix-code__terminal {
  opacity: 1;
  transform: translateX(0);
  transition: opacity 0.6s ease-out 0.15s, transform 0.6s ease-out 0.15s;
}

.ix-code__terminal-bar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 14px 18px 0;
  background: #0f172a;
}

.ix-code__dots {
  display: flex;
  gap: 7px;
}

.ix-code__dots span {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.08);
}

.ix-code__dots span:nth-child(1) { background: #ef4444; opacity: 0.7; }
.ix-code__dots span:nth-child(2) { background: #eab308; opacity: 0.7; }
.ix-code__dots span:nth-child(3) { background: #22c55e; opacity: 0.7; }

.ix-code__tabs {
  display: flex;
  gap: 2px;
}

.ix-code__tab {
  padding: 8px 16px;
  border: none;
  border-radius: 8px 8px 0 0;
  background: transparent;
  color: #64748b;
  font-family: 'SF Mono', 'Fira Code', 'JetBrains Mono', Menlo, monospace;
  font-size: 0.78rem;
  font-weight: 500;
  cursor: pointer;
  transition: color 0.2s, background 0.2s;
}

.ix-code__tab--active {
  background: #1e293b;
  color: #e2e8f0;
}

.ix-code__tab:hover:not(.ix-code__tab--active) {
  color: #94a3b8;
}

.ix-code__pre {
  margin: 0;
  padding: 20px 22px 24px;
  background: #1e293b;
  overflow-x: auto;
  scrollbar-width: thin;
  scrollbar-color: rgba(255,255,255,0.1) transparent;
}

.ix-code__pre code {
  font-family: 'SF Mono', 'Fira Code', 'JetBrains Mono', Menlo, monospace;
  font-size: 0.82rem;
  line-height: 1.7;
  color: #e2e8f0;
  white-space: pre;
}

/* ═══ Dark Mode ═══ */
.dark .ix-code__title {
  color: #f1f5f9;
}

.dark .ix-code__subtitle {
  color: #94a3b8;
}

.dark .ix-code__badge {
  color: #60a5fa;
}

.dark .ix-code__cta {
  color: #60a5fa;
}

.dark .ix-code__terminal {
  border-color: rgba(255, 255, 255, 0.06);
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.4), 0 0 0 1px rgba(255, 255, 255, 0.04);
}

/* ═══ Responsive ═══ */
@media (max-width: 900px) {
  .ix-code__inner {
    grid-template-columns: 1fr;
    gap: 32px;
  }

  .ix-code__title {
    font-size: 1.6rem;
  }
}

/* ═══ Reduced Motion ═══ */
@media (prefers-reduced-motion: reduce) {
  .ix-code__text,
  .ix-code__terminal {
    opacity: 1;
    transform: none;
  }
}

/* ═══ VitePress overrides ═══ */
.vp-doc .ix-code h2 {
  border: none;
  margin-top: 0;
  padding-top: 0;
}
</style>
