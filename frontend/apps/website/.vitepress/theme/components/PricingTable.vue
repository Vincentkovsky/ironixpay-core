<script setup lang="ts">
import { computed, ref } from 'vue'
import { useData } from 'vitepress'

const { lang } = useData()
const isZh = computed(() => lang.value === 'zh-CN')

// FAQ accordion state
const openFaq = ref<number | null>(null)
function toggleFaq(i: number) {
  openFaq.value = openFaq.value === i ? null : i
}

const t = computed(() =>
  isZh.value
    ? {
        title: '简单透明的定价',
        subtitle: '没有月费，没有隐藏成本。只在成功收款时收费。',
        // Standard card
        stdName: 'Standard',
        stdDesc: '适用于所有商户。注册即用，无审核。',
        stdRate: '0.5%',
        stdRateLabel: '每笔成功支付',
        stdMinFee: 'TRON/ETH 最低 1 USDT，其他链最低 0.1 USDT',
        stdCta: '开始使用',
        stdCtaLink: 'https://app.ironixpay.com',
        stdFeatures: [
          'USDT / USDC 多链收款 — 8 条链',
          'HD 钱包 — 每笔支付独立地址',
          '准实时链上确认',
          'Webhook 通知（签名 + 自动重试）',
          '异常支付自动捕获',
          '制裁名单 + 链上风控',
          '沙盒测试环境',
          'API 文档 & SDK',
          '商户控制台',
        ],
        // Enterprise card
        entName: 'Enterprise',
        entDesc: '专为高频交易与大额付款商户打造的灵活定制方案。',
        entRate: 'Custom',
        entRateLabel: '定制化费率',
        entCta: '联系我们',
        entCtaLink: '/enterprise',
        entFeatures: [
          '所有 Standard 功能',
          '专属优惠费率',
          '优先技术支持',
          '专属对接人',
          '定制 Webhook 配置',
          'Branded Checkout 页面',
        ],
        // Fee table
        feeTitle: '出金手续费',
        feeSubtitle: '从出金金额中扣除，无额外费用。',
        feeHeaders: ['网络', '手续费'],
        feeRows: [
          ['TRON', '$1.50'],
          ['BSC / Ethereum / Polygon / Arbitrum / Optimism / Base / Solana', '$0.50'],
        ],
        // Comparison
        compTitle: '与传统方案对比',
        compHeaders: ['', 'IronixPay', '传统支付网关', '自建收款'],
        compRows: [
          ['手续费', '0.5%', '2.9% + $0.30', '0%（开发成本高）'],
          ['结算速度', '近实时', '2–7 工作日', '即时'],
          ['接入工作量', '一次 API 调用', '审核 + 签约', '数月开发'],
          ['账户冻结风险', '无', '有', '无'],
          ['合规筛查', '内置', '部分', '需自建'],
        ],
        // FAQ
        faqTitle: '常见问题',
        faqs: [
          {
            q: '有隐藏费用吗？',
            a: '没有。无月费、无设置费、无最低消费。仅在成功收款时收取 0.5% 手续费（TRON/ETH 最低 1 USDT，其他链最低 0.1 USDT）。出金手续费按网络固定收取，详见上方费率表。',
          },
          {
            q: '支持哪些链和代币？',
            a: '目前支持 TRON、Solana、BSC、Ethereum、Polygon、Arbitrum、Optimism、Base 共 8 条链上的 USDT 和 USDC。沙盒环境暂仅支持 TRON Nile 测试网。',
          },
          {
            q: '资金什么时候到账？',
            a: '链上确认后资金自动归集至商户余额（通常 1-3 分钟）。商户余额可随时通过 API 或控制台发起出金。',
          },
          {
            q: '资金如何流转？',
            a: '每笔支付使用独立地址，链上确认后自动归集至商户余额。商户可以通过 API 或控制台发起出金。',
          },
          {
            q: '接入需要多久？',
            a: '一次 API 调用即可创建收银台。注册到首次收款通常不到 30 分钟，支持 JavaScript SDK 和 WooCommerce 插件。',
          },
        ],
      }
    : {
        title: 'Simple, Transparent Pricing',
        subtitle: 'No monthly fees. No hidden costs. You only pay when you earn.',
        stdName: 'Standard',
        stdDesc: 'For all merchants. Sign up and start collecting — no review needed.',
        stdRate: '0.5%',
        stdRateLabel: 'per successful payment',
        stdMinFee: 'Min. 1 USDT on TRON/ETH, 0.1 USDT on other chains',
        stdCta: 'Get Started',
        stdCtaLink: 'https://app.ironixpay.com',
        stdFeatures: [
          'USDT / USDC multi-chain collection — 8 chains',
          'HD Wallet — unique address per payment',
          'Near-instant on-chain confirmation',
          'Webhook notifications (signed + auto-retry)',
          'Exception auto-capture & Resolution Center',
          'Sanctions list + on-chain risk screening',
          'Sandbox testing environment',
          'API docs & SDK',
          'Merchant dashboard',
        ],
        entName: 'Enterprise',
        entDesc: 'Bespoke pricing solutions designed for merchants with high transaction volumes or enterprise needs.',
        entRate: 'Custom',
        entRateLabel: 'volume-based pricing',
        entCta: 'Contact Us',
        entCtaLink: '/en/enterprise',
        entFeatures: [
          'Everything in Standard',
          'Custom volume-based rates',
          'Priority technical support',
          'Dedicated account manager',
          'Custom webhook configuration',
          'Branded checkout page',
        ],
        feeTitle: 'Withdrawal Fees',
        feeSubtitle: 'Deducted from the payout amount. No additional charges.',
        feeHeaders: ['Network', 'Fee'],
        feeRows: [
          ['TRON', '$1.50'],
          ['BSC / Ethereum / Polygon / Arbitrum / Optimism / Base / Solana', '$0.50'],
        ],
        compTitle: 'How We Compare',
        compHeaders: ['', 'IronixPay', 'Traditional Gateways', 'Self-hosted'],
        compRows: [
          ['Fees', '0.5%', '2.9% + $0.30', '0% (high dev cost)'],
          ['Settlement', 'Near-instant', '2–7 business days', 'Instant'],
          ['Integration', 'One API call', 'Review + contract', 'Months of dev'],
          ['Account freeze risk', 'None', 'Yes', 'None'],
          ['Compliance screening', 'Built-in', 'Partial', 'Build yourself'],
        ],
        faqTitle: 'Frequently Asked Questions',
        faqs: [
          {
            q: 'Are there any hidden fees?',
            a: 'None. No monthly fees, no setup costs, no minimum volume. We charge 0.5% per successful payment (min. 1 USDT on TRON/ETH, 0.1 USDT on other chains) and a flat withdrawal fee per network, listed in the table above.',
          },
          {
            q: 'Which chains and tokens are supported?',
            a: 'We support USDT and USDC on 8 chains: TRON, Solana, BSC, Ethereum, Polygon, Arbitrum, Optimism, and Base. Sandbox currently supports TRON Nile testnet only.',
          },
          {
            q: 'How fast is settlement?',
            a: 'After on-chain confirmation, funds are automatically swept to your merchant balance (typically 1–3 minutes). You can withdraw anytime via the API or dashboard.',
          },
          {
            q: 'How do funds move?',
            a: 'Each payment uses a unique address. After on-chain confirmation, funds are automatically credited to your merchant balance and can be withdrawn via the API or dashboard.',
          },
          {
            q: 'How long does integration take?',
            a: 'One API call creates a checkout. From sign-up to first payment is typically under 30 minutes. We provide a JavaScript SDK and WooCommerce plugin out of the box.',
          },
        ],
      },
)
</script>

<template>
  <div class="ix-pricing">
    <!-- Header -->
    <div class="ix-pricing__header">
      <h1 class="ix-pricing__title">{{ t.title }}</h1>
      <p class="ix-pricing__subtitle">{{ t.subtitle }}</p>
    </div>

    <!-- Dual Cards -->
    <div class="ix-pricing__cards">
      <!-- Standard Card -->
      <div class="ix-pricing__card ix-pricing__card--std">
        <div class="ix-pricing__card-top">
          <span class="ix-pricing__plan-label">{{ t.stdName }}</span>
          <p class="ix-pricing__plan-desc">{{ t.stdDesc }}</p>
          <div class="ix-pricing__rate-block">
            <span class="ix-pricing__rate-num">{{ t.stdRate }}</span>
            <span class="ix-pricing__rate-sub">{{ t.stdRateLabel }}</span>
          </div>
          <span class="ix-pricing__min-fee">{{ t.stdMinFee }}</span>
        </div>
        <div class="ix-pricing__card-body">
          <ul class="ix-pricing__features">
            <li v-for="f in t.stdFeatures" :key="f">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M3.5 8.5l3 3 6-6" stroke="#22c55e" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>
              {{ f }}
            </li>
          </ul>
        </div>
        <div class="ix-pricing__card-cta">
          <a
            :href="t.stdCtaLink"
            target="_blank"
            class="ix-pricing__btn ix-pricing__btn--primary"
            data-analytics-event="cta_click"
            data-analytics-name="create_account"
            data-analytics-location="pricing_standard"
          >
            {{ t.stdCta }}
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M3 8h10m0 0L9 4m4 4L9 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
          </a>
        </div>
      </div>

      <!-- Enterprise Card -->
      <div class="ix-pricing__card ix-pricing__card--ent">
        <div class="ix-pricing__card-top">
          <span class="ix-pricing__plan-label">{{ t.entName }}</span>
          <p class="ix-pricing__plan-desc">{{ t.entDesc }}</p>
          <div class="ix-pricing__rate-block">
            <span class="ix-pricing__rate-num ix-pricing__rate-num--ent">{{ t.entRate }}</span>
            <span class="ix-pricing__rate-sub">{{ t.entRateLabel }}</span>
          </div>
        </div>
        <div class="ix-pricing__card-body">
          <ul class="ix-pricing__features">
            <li v-for="f in t.entFeatures" :key="f">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M3.5 8.5l3 3 6-6" stroke="#22c55e" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>
              {{ f }}
            </li>
          </ul>
        </div>
        <div class="ix-pricing__card-cta">
          <a
            :href="t.entCtaLink"
            class="ix-pricing__btn ix-pricing__btn--outline"
            data-analytics-event="cta_click"
            data-analytics-name="contact_sales"
            data-analytics-location="pricing_enterprise"
          >
            {{ t.entCta }}
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M4 10L10 4m0 0H5m5 0v5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
          </a>
        </div>
      </div>
    </div>

    <!-- Withdrawal Fee Table -->
    <div class="ix-pricing__fee-section">
      <h2 class="ix-pricing__section-heading">{{ t.feeTitle }}</h2>
      <p class="ix-pricing__section-sub">{{ t.feeSubtitle }}</p>
      <div class="ix-pricing__fee-table-wrap">
        <table class="ix-pricing__fee-table">
          <thead>
            <tr>
              <th v-for="h in t.feeHeaders" :key="h">{{ h }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(row, i) in t.feeRows" :key="i">
              <td>{{ row[0] }}</td>
              <td class="ix-pricing__fee-amount">{{ row[1] }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Comparison Table -->
    <div class="ix-pricing__comparison">
      <h2 class="ix-pricing__section-heading">{{ t.compTitle }}</h2>
      <div class="ix-pricing__comp-wrap">
        <table class="ix-pricing__comp-table">
          <thead>
            <tr>
              <th v-for="h in t.compHeaders" :key="h">{{ h }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(row, i) in t.compRows" :key="i">
              <td v-for="(cell, j) in row" :key="j" :class="{ 'ix-pricing__highlight': j === 1 }">
                {{ cell }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- FAQ -->
    <div class="ix-pricing__faq">
      <h2 class="ix-pricing__section-heading">{{ t.faqTitle }}</h2>
      <div class="ix-pricing__faq-list">
        <div
          v-for="(faq, i) in t.faqs"
          :key="i"
          class="ix-pricing__faq-item"
          :class="{ 'ix-pricing__faq-item--open': openFaq === i }"
        >
          <button class="ix-pricing__faq-q" @click="toggleFaq(i)">
            <span>{{ faq.q }}</span>
            <svg class="ix-pricing__faq-chevron" width="18" height="18" viewBox="0 0 18 18" fill="none">
              <path d="M4.5 6.75L9 11.25L13.5 6.75" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </button>
          <div class="ix-pricing__faq-a">
            <p>{{ faq.a }}</p>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* ═══ Pricing — professional, dense, trustworthy ═══ */
.ix-pricing {
  max-width: 960px;
  margin: 0 auto;
  padding: 48px 24px 80px;
}

/* ─── Header ─── */
.ix-pricing__header {
  text-align: center;
  margin-bottom: 48px;
}

.ix-pricing__title {
  font-family: 'Exo 2', sans-serif;
  font-size: 2.4rem;
  font-weight: 800;
  letter-spacing: -0.03em;
  color: #0f172a;
  margin: 0 0 12px;
}

.ix-pricing__subtitle {
  font-size: 1.05rem;
  color: #64748b;
  margin: 0;
}

/* ─── Dual Cards ─── */
.ix-pricing__cards {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 24px;
  margin-bottom: 64px;
}

.ix-pricing__card {
  border-radius: 16px;
  border: 1px solid #e2e8f0;
  background: #fff;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  transition: box-shadow 0.3s ease;
}

.ix-pricing__card:hover {
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.06);
}

.ix-pricing__card--std {
  border-color: #2563eb;
  box-shadow: 0 0 0 1px rgba(37, 99, 235, 0.15), 0 4px 24px rgba(37, 99, 235, 0.08);
}

.ix-pricing__card-top {
  padding: 28px 28px 24px;
  border-bottom: 1px solid #f1f5f9;
}

.ix-pricing__plan-label {
  display: inline-block;
  font-size: 0.72rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.1em;
  color: #64748b;
  margin-bottom: 8px;
}

.ix-pricing__plan-desc {
  font-size: 0.88rem;
  color: #475569;
  margin: 0 0 20px;
  line-height: 1.5;
}

.ix-pricing__rate-block {
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.ix-pricing__rate-num {
  font-family: 'Exo 2', sans-serif;
  font-size: 3rem;
  font-weight: 800;
  color: #0f172a;
  line-height: 1;
}

.ix-pricing__rate-num--ent {
  font-size: 2.2rem;
}

.ix-pricing__rate-sub {
  font-size: 0.82rem;
  color: #64748b;
}

.ix-pricing__min-fee {
  display: inline-block;
  margin-top: 8px;
  font-size: 0.78rem;
  color: #94a3b8;
}

/* ─── Features list ─── */
.ix-pricing__card-body {
  padding: 24px 28px;
  flex: 1;
}

.ix-pricing__features {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.ix-pricing__features li {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  font-size: 0.88rem;
  color: #334155;
  line-height: 1.5;
}

.ix-pricing__features li svg {
  flex-shrink: 0;
  margin-top: 2px;
}

/* ─── CTA ─── */
.ix-pricing__card-cta {
  padding: 20px 28px 28px;
}

.ix-pricing__btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  width: 100%;
  padding: 14px 24px;
  border-radius: 10px;
  font-size: 0.92rem;
  font-weight: 600;
  text-decoration: none;
  cursor: pointer;
  transition: all 0.2s ease;
}

.ix-pricing__btn--primary {
  background: #0f172a;
  color: #fff;
  border: none;
}

.ix-pricing__btn--primary:hover {
  background: #1e293b;
  transform: translateY(-1px);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
}

.ix-pricing__btn--outline {
  background: transparent;
  color: #0f172a;
  border: 1.5px solid #cbd5e1;
}

.ix-pricing__btn--outline:hover {
  border-color: #64748b;
  background: #f8fafc;
}

/* ─── Section headings ─── */
.ix-pricing__section-heading {
  font-family: 'Exo 2', sans-serif;
  font-size: 1.4rem;
  font-weight: 700;
  color: #0f172a;
  text-align: center;
  margin: 0 0 8px;
  padding: 0;
  border: none;
}

.ix-pricing__section-sub {
  text-align: center;
  font-size: 0.88rem;
  color: #64748b;
  margin: 0 0 24px;
}

/* ─── Fee Table ─── */
.ix-pricing__fee-section {
  margin-bottom: 64px;
}

.ix-pricing__fee-table-wrap {
  max-width: 420px;
  margin: 0 auto;
  border-radius: 12px;
  overflow: hidden;
  border: 1px solid #e2e8f0;
}

.ix-pricing__fee-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.88rem;
}

.ix-pricing__fee-table th {
  padding: 12px 20px;
  text-align: left;
  font-size: 0.72rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: #64748b;
  background: #f8fafc;
  border-bottom: 1px solid #e2e8f0;
}

.ix-pricing__fee-table td {
  padding: 11px 20px;
  color: #334155;
  border-bottom: 1px solid #f1f5f9;
}

.ix-pricing__fee-table tr:last-child td {
  border-bottom: none;
}

.ix-pricing__fee-amount {
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

/* ─── Comparison Table ─── */
.ix-pricing__comparison {
  margin-bottom: 64px;
}

.ix-pricing__comp-wrap {
  border-radius: 12px;
  overflow: hidden;
  border: 1px solid #e2e8f0;
  background: #fff;
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.03);
}

.ix-pricing__comp-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.85rem;
}

.ix-pricing__comp-table th {
  padding: 14px 18px;
  text-align: left;
  font-size: 0.72rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: #64748b;
  background: #f8fafc;
  border-bottom: 1px solid #e2e8f0;
}

.ix-pricing__comp-table td {
  padding: 14px 18px;
  color: #334155;
  border-bottom: 1px solid #f1f5f9;
}

.ix-pricing__comp-table tr:last-child td {
  border-bottom: none;
}

.ix-pricing__comp-table .ix-pricing__highlight {
  color: #2563eb !important;
  font-weight: 600;
}

/* ─── FAQ Accordion ─── */
.ix-pricing__faq {
  margin-bottom: 32px;
}

.ix-pricing__faq-list {
  max-width: 700px;
  margin: 16px auto 0;
  display: flex;
  flex-direction: column;
}

.ix-pricing__faq-item {
  border-bottom: 1px solid #e2e8f0;
}

.ix-pricing__faq-q {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 18px 0;
  background: none;
  border: none;
  cursor: pointer;
  font-size: 0.95rem;
  font-weight: 600;
  color: #0f172a;
  text-align: left;
  line-height: 1.4;
  gap: 12px;
}

.ix-pricing__faq-q:hover {
  color: #2563eb;
}

.ix-pricing__faq-chevron {
  flex-shrink: 0;
  transition: transform 0.2s ease;
  color: #94a3b8;
}

.ix-pricing__faq-item--open .ix-pricing__faq-chevron {
  transform: rotate(180deg);
}

.ix-pricing__faq-a {
  max-height: 0;
  overflow: hidden;
  transition: max-height 0.3s ease, padding 0.3s ease;
  padding: 0;
}

.ix-pricing__faq-item--open .ix-pricing__faq-a {
  max-height: 400px;
  padding: 0 0 18px;
}

.ix-pricing__faq-a p {
  margin: 0;
  font-size: 0.88rem;
  color: #475569;
  line-height: 1.7;
}

/* ═══ Dark mode ═══ */
.dark .ix-pricing__title { color: #f1f5f9; }
.dark .ix-pricing__subtitle { color: #94a3b8; }

.dark .ix-pricing__card {
  background: #111827;
  border-color: rgba(255, 255, 255, 0.08);
}
.dark .ix-pricing__card--std {
  border-color: #3b82f6;
  box-shadow: 0 0 0 1px rgba(59, 130, 246, 0.2), 0 4px 24px rgba(59, 130, 246, 0.06);
}
.dark .ix-pricing__card-top {
  border-bottom-color: rgba(255, 255, 255, 0.06);
}
.dark .ix-pricing__plan-label { color: #94a3b8; }
.dark .ix-pricing__plan-desc { color: #94a3b8; }
.dark .ix-pricing__rate-num { color: #f1f5f9; }
.dark .ix-pricing__rate-sub { color: #64748b; }
.dark .ix-pricing__min-fee { color: #475569; }
.dark .ix-pricing__features li { color: #d1d5db; }
.dark .ix-pricing__btn--primary {
  background: #e2e8f0;
  color: #0f172a;
}
.dark .ix-pricing__btn--primary:hover {
  background: #f1f5f9;
}
.dark .ix-pricing__btn--outline {
  color: #e2e8f0;
  border-color: #334155;
}
.dark .ix-pricing__btn--outline:hover {
  border-color: #64748b;
  background: rgba(255, 255, 255, 0.04);
}

.dark .ix-pricing__section-heading { color: #f1f5f9; }
.dark .ix-pricing__section-sub { color: #94a3b8; }

.dark .ix-pricing__fee-table-wrap {
  border-color: #1e293b;
}
.dark .ix-pricing__fee-table th {
  background: #111827;
  color: #94a3b8;
  border-bottom-color: #1e293b;
}
.dark .ix-pricing__fee-table td {
  color: #d1d5db;
  border-bottom-color: rgba(255, 255, 255, 0.05);
}

.dark .ix-pricing__comp-wrap {
  border-color: #1e293b;
  background: #0f172a;
  box-shadow: none;
}
.dark .ix-pricing__comp-table th {
  color: #94a3b8;
  background: #0b0f1a;
  border-bottom-color: #1e293b;
}
.dark .ix-pricing__comp-table td {
  color: #94a3b8;
  border-bottom-color: rgba(255, 255, 255, 0.05);
}
.dark .ix-pricing__comp-table .ix-pricing__highlight {
  color: #60a5fa !important;
}

.dark .ix-pricing__faq-item {
  border-bottom-color: rgba(255, 255, 255, 0.06);
}
.dark .ix-pricing__faq-q {
  color: #f1f5f9;
}
.dark .ix-pricing__faq-q:hover {
  color: #60a5fa;
}
.dark .ix-pricing__faq-a p {
  color: #94a3b8;
}

/* ═══ Responsive ═══ */
@media (max-width: 720px) {
  .ix-pricing__cards {
    grid-template-columns: 1fr;
  }

  .ix-pricing__title {
    font-size: 1.8rem;
  }

  .ix-pricing__rate-num {
    font-size: 2.4rem;
  }

  .ix-pricing__comp-table {
    font-size: 0.78rem;
  }

  .ix-pricing__comp-table th,
  .ix-pricing__comp-table td {
    padding: 10px 12px;
  }
}

/* ═══ Override VitePress defaults ═══ */
:deep(.vp-doc) .ix-pricing h1,
:deep(.vp-doc) .ix-pricing h2,
:deep(.vp-doc) .ix-pricing h3 {
  border: none;
  margin-top: 0;
  padding-top: 0;
}

:deep(.vp-doc) .ix-pricing a {
  color: inherit;
  text-decoration: none;
  font-weight: inherit;
}

:deep(.vp-doc) .ix-pricing ul {
  list-style: none;
  padding: 0;
}
</style>
