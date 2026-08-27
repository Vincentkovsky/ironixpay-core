<script setup lang="ts">
import { computed } from 'vue'
import { useData } from 'vitepress'
import { ANALYTICS_PREFERENCES_EVENT } from '../analytics'

const { lang, page, frontmatter } = useData()
const isEn = computed(() => lang.value === 'en-US' || lang.value === 'en')
const prefix = computed(() => (isEn.value ? '/en' : ''))
const DOC_ROUTE_RE = /^(en\/)?(api|guide|use-cases)\//

const shouldRenderFooter = computed(() => {
  if (frontmatter.value.footer === false) return false
  const path = page.value.relativePath || ''
  return !DOC_ROUTE_RE.test(path)
})

const columns = computed(() =>
  isEn.value
    ? [
        {
          title: 'Product',
          links: [
            { text: 'Checkout', href: '/en/checkout' },
            { text: 'Payouts', href: '/en/payouts' },
            { text: 'Pricing', href: '/en/pricing' },
            { text: 'Enterprise', href: '/en/enterprise' },
            { text: 'Live Demo', href: '/en/demo' },
            { text: 'Dashboard', href: 'https://app.ironixpay.com', external: true },
            { text: 'Supported Networks', href: '/en/guide/networks' },
          ],
        },
        {
          title: 'Developers',
          links: [
            { text: 'Quick Start', href: '/en/guide/quickstart' },
            { text: 'API Reference', href: 'https://api.ironixpay.com/docs', external: true },
            { text: 'Webhooks', href: '/en/guide/webhooks' },
            { text: 'SDK Integration', href: '/en/guide/integration' },
            { text: 'Error Codes', href: '/en/guide/errors' },
          ],
        },
        {
          title: 'Resources',
          links: [
            { text: 'WooCommerce', href: '/en/use-cases/woocommerce' },
            { text: 'Telegram Bot', href: '/en/use-cases/telegram-bot' },
            { text: 'Forex Brokers', href: '/en/use-cases/forex' },
            { text: 'E-commerce', href: '/en/use-cases/ecommerce' },
            { text: 'PSP & Marketplace', href: '/en/use-cases/psp-marketplace' },
          ],
        },
        {
          title: 'Legal',
          links: [
            { text: 'Terms of Service', href: '/en/terms' },
            { text: 'Privacy Policy', href: '/en/privacy' },
            { text: 'Trust & Security', href: '/en/trust' },
          ],
        },
      ]
    : [
        {
          title: '产品',
          links: [
            { text: '收款 Checkout', href: '/checkout' },
            { text: '出款 Payouts', href: '/payouts' },
            { text: '定价', href: '/pricing' },
            { text: '企业接入', href: '/enterprise' },
            { text: '在线体验', href: '/demo' },
            { text: '控制台', href: 'https://app.ironixpay.com', external: true },
            { text: '支持的网络', href: '/guide/networks' },
          ],
        },
        {
          title: '开发者',
          links: [
            { text: '快速开始', href: '/guide/quickstart' },
            { text: 'API 参考', href: 'https://api.ironixpay.com/docs', external: true },
            { text: 'Webhooks', href: '/guide/webhooks' },
            { text: 'SDK 集成', href: '/guide/integration' },
            { text: '错误码', href: '/guide/errors' },
          ],
        },
        {
          title: '使用场景',
          links: [
            { text: 'WooCommerce', href: '/use-cases/woocommerce' },
            { text: 'Telegram Bot', href: '/use-cases/telegram-bot' },
            { text: '外汇经纪商', href: '/use-cases/forex' },
            { text: '跨境电商', href: '/use-cases/ecommerce' },
            { text: 'PSP 与聚合平台', href: '/use-cases/psp-marketplace' },
          ],
        },
        {
          title: '法律',
          links: [
            { text: '服务条款', href: '/terms' },
            { text: '隐私政策', href: '/privacy' },
            { text: '安全与合规', href: '/trust' },
          ],
        },
      ],
)

const networks = [
  { name: 'TRON', icon: '/networks/tron.svg' },
  { name: 'Solana', icon: '/networks/solana.svg' },
  { name: 'BSC', icon: '/networks/bsc.svg' },
  { name: 'Ethereum', icon: '/networks/ethereum.svg' },
  { name: 'Polygon', icon: '/networks/polygon.svg' },
  { name: 'Arbitrum', icon: '/networks/arb.svg' },
  { name: 'Optimism', icon: '/networks/op.svg' },
  { name: 'Base', icon: '/networks/base.svg' },
]

const year = new Date().getFullYear()
const copyright = computed(() =>
  isEn.value
    ? `© ${year} IronixPay. All rights reserved.`
    : `© ${year} IronixPay。保留所有权利。`,
)

function openAnalyticsPreferences() {
  if (typeof window !== 'undefined') {
    window.dispatchEvent(new Event(ANALYTICS_PREFERENCES_EVENT))
  }
}
</script>

<template>
  <footer v-if="shouldRenderFooter" class="ix-footer">
    <!-- Top section: brand + columns -->
    <div class="ix-footer__main">
      <div class="ix-footer__inner">
        <!-- Brand column -->
        <div class="ix-footer__brand">
          <img :src="'/logo-white.svg'" alt="IronixPay" class="ix-footer__logo-img" width="140" />
          <p class="ix-footer__tagline">
            {{ isEn ? 'Stablecoin payment infrastructure for merchants.' : '面向商户的稳定币支付基础设施。' }}
          </p>
          <!-- Social icons -->
          <div class="ix-footer__socials">
            <a href="https://x.com/IronixPay" target="_blank" rel="noopener noreferrer" aria-label="X / Twitter" class="ix-footer__social-link">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z"/></svg>
            </a>
            <a href="https://t.me/ironixpay" target="_blank" rel="noopener noreferrer" aria-label="Telegram" class="ix-footer__social-link">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M20.665 3.717l-17.73 6.837c-1.21.486-1.203 1.161-.222 1.462l4.552 1.42 10.532-6.645c.498-.303.953-.14.579.192l-8.533 7.701h-.002l.002.001-.314 4.692c.46 0 .663-.211.921-.46l2.211-2.15 4.599 3.397c.848.467 1.457.227 1.668-.785l3.019-14.228c.309-1.239-.473-1.8-1.282-1.434z"/></svg>
            </a>
            <a href="mailto:support@ironixpay.com" aria-label="Email" class="ix-footer__social-link">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="4" width="20" height="16" rx="2"/><path d="m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7"/></svg>
            </a>
          </div>
        </div>

        <!-- Link columns -->
        <nav class="ix-footer__columns">
          <div v-for="col in columns" :key="col.title" class="ix-footer__col">
            <h4 class="ix-footer__col-title">{{ col.title }}</h4>
            <ul class="ix-footer__links">
              <li v-for="link in col.links" :key="link.text">
                <a
                  :href="link.href"
                  class="ix-footer__link"
                  :target="link.external ? '_blank' : undefined"
                  :rel="link.external ? 'noopener noreferrer' : undefined"
                >
                  {{ link.text }}
                  <span v-if="link.external" class="ix-footer__ext">↗</span>
                </a>
              </li>
            </ul>
          </div>
        </nav>
      </div>
    </div>


    <!-- Jurisdiction disclaimer -->
    <div class="ix-footer__compliance">
      <div class="ix-footer__compliance-inner">
        <svg class="ix-footer__compliance-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
        </svg>
        <p>
          {{ isEn
            ? 'IronixPay only provides compliant payment consulting services to clients based outside Mainland China and the United States.'
            : 'IronixPay 仅向中国大陆及美国以外的客户提供合规的支付咨询服务。'
          }}
        </p>
      </div>
    </div>

    <!-- Bottom bar -->
    <div class="ix-footer__bottom">
      <div class="ix-footer__bottom-inner">
        <span>{{ copyright }}</span>
        <div class="ix-footer__bottom-links">
          <a :href="`${prefix}/terms`">{{ isEn ? 'Terms' : '条款' }}</a>
          <span class="ix-footer__sep">·</span>
          <a :href="`${prefix}/privacy`">{{ isEn ? 'Privacy' : '隐私' }}</a>
          <span class="ix-footer__sep">·</span>
          <button type="button" class="ix-footer__preferences" @click="openAnalyticsPreferences">
            {{ isEn ? 'Cookie preferences' : 'Cookie 设置' }}
          </button>
        </div>
      </div>
    </div>
  </footer>
</template>

<style scoped>
/* ═══ Footer — dark, dense, fintech-grade ═══ */
.ix-footer {
  position: relative;
  z-index: 10;
  background: #0b0f1a;
  color: #94a3b8;
  font-size: 0.875rem;
  line-height: 1.6;
  margin-top: 0;
}

/* ─── Main content area ─── */
.ix-footer__main {
  border-top: 1px solid rgba(255, 255, 255, 0.06);
}

.ix-footer__inner {
  max-width: 1140px;
  margin: 0 auto;
  padding: 56px 32px 40px;
  display: grid;
  grid-template-columns: 280px 1fr;
  gap: 64px;
}

/* ─── Brand ─── */
.ix-footer__brand {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.ix-footer__logo-img {
  height: 28px;
  width: auto;
  opacity: 0.9;
}

.ix-footer__tagline {
  margin: 0;
  font-size: 0.82rem;
  color: #64748b;
  line-height: 1.5;
}

.ix-footer__socials {
  display: flex;
  gap: 12px;
  margin-top: 4px;
}

.ix-footer__social-link {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 34px;
  height: 34px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
  color: #94a3b8;
  transition: all 0.2s ease;
}

.ix-footer__social-link:hover {
  background: rgba(255, 255, 255, 0.1);
  color: #e2e8f0;
}

/* ─── Columns ─── */
.ix-footer__columns {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 24px;
}

.ix-footer__col-title {
  font-family: 'Exo 2', sans-serif;
  font-size: 0.78rem;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: #e2e8f0;
  margin: 0 0 14px;
}

.ix-footer__links {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ix-footer__link {
  color: #64748b;
  text-decoration: none;
  font-size: 0.84rem;
  transition: color 0.15s ease;
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.ix-footer__link:hover {
  color: #e2e8f0;
}

.ix-footer__ext {
  font-size: 0.7rem;
  opacity: 0.5;
}

/* ─── Networks strip ─── */
.ix-footer__networks {
  border-top: 1px solid rgba(255, 255, 255, 0.05);
}

.ix-footer__networks-inner {
  max-width: 1140px;
  margin: 0 auto;
  padding: 20px 32px;
  display: flex;
  align-items: center;
  gap: 24px;
}

.ix-footer__networks-label {
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: #475569;
  white-space: nowrap;
}

.ix-footer__networks-row {
  display: flex;
  gap: 16px;
  align-items: center;
}

.ix-footer__network-icon {
  width: 22px;
  height: 22px;
  opacity: 0.35;
  transition: opacity 0.2s ease;
  filter: grayscale(100%);
}

.ix-footer__network-icon:hover {
  opacity: 0.8;
  filter: grayscale(0%);
}

/* ─── Jurisdiction compliance ─── */
.ix-footer__compliance {
  border-top: 1px solid rgba(255, 255, 255, 0.05);
}

.ix-footer__compliance-inner {
  max-width: 1140px;
  margin: 0 auto;
  padding: 18px 32px;
  display: flex;
  align-items: flex-start;
  gap: 10px;
}

.ix-footer__compliance-icon {
  flex-shrink: 0;
  margin-top: 1px;
  color: #d97706;
  opacity: 0.7;
}

.ix-footer__compliance p {
  margin: 0;
  font-size: 0.78rem;
  line-height: 1.65;
  color: #94a3b8;
  font-weight: 500;
  letter-spacing: 0.005em;
}

/* ─── Bottom bar ─── */
.ix-footer__bottom {
  border-top: 1px solid rgba(255, 255, 255, 0.05);
}

.ix-footer__bottom-inner {
  max-width: 1140px;
  margin: 0 auto;
  padding: 16px 32px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.75rem;
  color: #475569;
}

.ix-footer__bottom-links {
  display: flex;
  align-items: center;
  gap: 6px;
}

.ix-footer__bottom-links a,
.ix-footer__preferences {
  color: #475569;
  text-decoration: none;
  transition: color 0.15s ease;
}

.ix-footer__preferences {
  padding: 0;
  border: 0;
  background: transparent;
  font: inherit;
  letter-spacing: 0;
  cursor: pointer;
}

.ix-footer__bottom-links a:hover,
.ix-footer__preferences:hover {
  color: #94a3b8;
}

.ix-footer__preferences:focus-visible {
  outline: 2px solid #94a3b8;
  outline-offset: 3px;
}

.ix-footer__sep {
  color: #334155;
}

/* ═══ Light mode overrides ═══ */
:root:not(.dark) .ix-footer {
  background: #0f172a;
}

/* ═══ Responsive ═══ */
@media (max-width: 900px) {
  .ix-footer__inner {
    grid-template-columns: 1fr;
    gap: 40px;
    padding: 40px 24px 32px;
  }

  .ix-footer__columns {
    grid-template-columns: repeat(2, 1fr);
    gap: 32px;
  }

  .ix-footer__networks-inner {
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
    padding: 16px 24px;
  }
}

@media (max-width: 540px) {
  .ix-footer__columns {
    grid-template-columns: 1fr;
    gap: 28px;
  }

  .ix-footer__bottom-inner {
    flex-direction: column;
    gap: 8px;
    text-align: center;
  }
}
</style>
