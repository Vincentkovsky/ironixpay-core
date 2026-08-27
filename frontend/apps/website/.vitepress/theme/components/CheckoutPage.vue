<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { useData } from 'vitepress'

const { lang } = useData()
const isZh = computed(() => lang.value === 'zh-CN')
const prefix = computed(() => (isZh.value ? '' : '/en'))

const t = computed(() =>
  isZh.value
    ? {
        heroTag: '收款产品',
        heroTitle: '为你的业务接入加密货币收款',
        heroDesc: '通过简洁的 Checkout 收银台，让全球客户使用 USDT/USDC 完成支付。一次 API 调用，全流程自动化——从创建订单到链上确认、资金归集。',
        ctaPrimary: '开始接入',
        ctaSecondary: '在线体验',
        benefitsTag: '核心优势',
        benefitsTitle: '更高转化率，更低成本',
        benefitsSubtitle: '为商户和客户双方优化的支付体验。',
        benefits: [
          { icon: 'globe', title: '触达全球客户', desc: '支持 8 条公链的 USDT/USDC 支付（TRON 仅 USDT），每笔支付分配独立 HD 派生地址，覆盖全球加密货币用户。' },
          { icon: 'shield', title: '零拒付风险', desc: '加密货币支付为推送式交易，无法逆转，彻底消除信用卡式的拒付欺诈。' },
          { icon: 'zap', title: '准实时结算', desc: '链上确认后资金自动归集至商户余额（通常 1-3 分钟），余额随时可通过 API 或控制台提现。' },
          { icon: 'lock', title: '内置风控', desc: 'OFAC 黑名单 + GoPlus API 双重筛查，每笔入金地址自动审核。命中风险则冻结待审。' },
        ],
        featuresTag: '产品能力',
        featuresTitle: '专为转化率设计的收银台',
        featuresSubtitle: '每一个细节都为减少客户放弃支付而优化。',
        features: [
          { icon: 'flow', title: '引导式支付流程', desc: '清晰的分步引导，支持法币定价自动换算（USD、CNY、EUR 等），系统自动转换为链上金额。' },
          { icon: 'mobile', title: '移动端优先', desc: '通过 @ironix-pay/sdk 嵌入 iframe，在任何屏幕尺寸上都提供流畅的支付体验。' },
          { icon: 'realtime', title: '实时状态更新', desc: '通过 SSE 实时推送链上确认进度，客户全程可见。支付未全额到账时自动延期等待。' },
          { icon: 'error', title: '异常支付处理', desc: '不足额自动延期等待补款，超额视为成功并全额归集。过期迟到付款、重复付款、AML 拦截等异常进入 Resolution Center 处理。' },
          { icon: 'brand', title: 'SDK 主题定制', desc: '通过 SDK 配置 theme 和 locale 参数调整收银台外观。Enterprise 方案支持完全品牌化定制。' },
          { icon: 'webhook', title: 'Webhook 通知', desc: 'session.completed / expired / blocked 等事件实时回调，HMAC-SHA256 签名验证 + 7 级指数退避重试。' },
        ],
        networksTag: '支持的网络',
        networksTitle: '8 条链，统一 API',
        networksSubtitle: '客户自由选择链和币种，你只需对接一套 API。',
        stepsTag: '接入流程',
        stepsTitle: '几分钟即可上线',
        stepsSubtitle: '从注册到接收首笔支付，流程极简。',
        steps: [
          { num: '01', title: '创建商户账户', desc: '注册并完成基本配置，获取 API 密钥。' },
          { num: '02', title: '集成 API', desc: '一次 API 调用创建 Checkout Session，或使用前端 SDK 嵌入收银台。' },
          { num: '03', title: '客户支付', desc: '客户选择链和币种，扫码或转账完成支付。' },
          { num: '04', title: '资金到账', desc: '链上确认后自动归集，余额随时可提现至链上地址。' },
        ],
        faqTag: '常见问题',
        faqTitle: 'FAQ',
        faqs: [
          { q: '支持哪些加密货币？', a: '支持 USDT 和 USDC，覆盖 TRON、Solana、BSC、Ethereum、Polygon、Arbitrum、Optimism、Base 共 8 条链。注意：TRON 仅支持 USDT，暂不支持 USDC。' },
          { q: '手续费是多少？', a: '收款手续费 0.5%（TRON/ETH 最低 1 USDT，其他链最低 0.1 USDT），无月费、无设置费、无隐藏费用。仅在成功收款时扣费。' },
          { q: '资金多久到账？', a: '链上达到所需确认数后（各链不同），资金自动归集至商户余额，整个过程通常 1-3 分钟。余额可随时通过 API 或控制台发起提现。' },
          { q: '需要自己处理加密货币吗？', a: '不需要。所有收款地址通过 HD 钱包派生，资金自动归集至平台 treasury，以 USDT/USDC 余额形式记入你的商户账户。你可随时提现到自己的链上钱包。' },
          { q: '超额或不足额付款怎么办？', a: '不足额时 Session 进入 Underpaid 状态并自动延期 24 小时等待补款。超额（Overpaid）视为支付成功，全额归集并正常触发回调，超出部分记录在交易明细中。过期后的迟到付款、重复付款等异常场景则进入 Resolution Center 处理。' },
          { q: '沙盒环境支持哪些链？', a: '目前沙盒环境仅支持 TRON Nile 测试网。使用 sk_test_ 前缀的 API 密钥即可访问。更多沙盒网络在规划中。' },
        ],
        ctaTitle: '准备好接入加密货币收款了吗？',
        ctaDesc: '注册即可获取 Sandbox 环境（TRON Nile 测试网），零风险测试完整支付流程。',
        ctaBtnPrimary: '免费开始',
        ctaBtnSecondary: '查看文档',
      }
    : {
        heroTag: 'Accept Payments',
        heroTitle: 'Accept crypto payments with a seamless checkout',
        heroDesc: 'Let your customers pay with USDT and USDC through a clean, intuitive checkout. One API call — auto-confirmation, auto-sweeping, near-instant settlement.',
        ctaPrimary: 'Get Started',
        ctaSecondary: 'Live Demo',
        benefitsTag: 'Why IronixPay',
        benefitsTitle: 'More revenue, lower costs',
        benefitsSubtitle: 'A payment experience optimized for both merchants and customers.',
        benefits: [
          { icon: 'globe', title: 'Reach global customers', desc: 'Accept USDT & USDC on 8 chains (TRON supports USDT only). Each payment gets a unique HD-derived address. No borders, no restrictions.' },
          { icon: 'shield', title: 'Zero chargeback risk', desc: 'Crypto payments are push-only. Funds cannot be reversed, protecting your business from costly disputes.' },
          { icon: 'zap', title: 'Near-instant settlement', desc: 'Funds auto-sweep to your merchant balance after on-chain confirmation (typically 1–3 minutes). Withdraw via API or dashboard anytime.' },
          { icon: 'lock', title: 'Built-in compliance', desc: 'OFAC blacklist + GoPlus API dual-layer screening reviews every inbound address. Flagged payments are frozen for review.' },
        ],
        featuresTag: 'Checkout Features',
        featuresTitle: 'Built for conversion',
        featuresSubtitle: 'Every detail is optimized to reduce payment drop-off.',
        features: [
          { icon: 'flow', title: 'Guided payment flow', desc: 'Clear step-by-step guidance with fiat pricing support (USD, CNY, EUR, etc.) — auto-converted to on-chain amounts.' },
          { icon: 'mobile', title: 'Mobile-first design', desc: 'Embed via @ironix-pay/sdk iframe. Seamless payment experience across any screen size or device.' },
          { icon: 'realtime', title: 'Real-time status', desc: 'SSE-powered live on-chain confirmation updates visible to your customer. Underpaid sessions auto-extend.' },
          { icon: 'error', title: 'Exception handling', desc: 'Underpaid sessions auto-extend for top-ups. Overpaid sessions succeed and sweep in full. Late payments, duplicates, and AML flags route to the Resolution Center.' },
          { icon: 'brand', title: 'SDK theming', desc: 'Configure theme and locale via SDK parameters. Enterprise plans support fully branded checkout pages.' },
          { icon: 'webhook', title: 'Webhook notifications', desc: 'session.completed / expired / blocked events with HMAC-SHA256 signature and 7-level exponential backoff retry.' },
        ],
        networksTag: 'Supported Networks',
        networksTitle: '8 chains, one API',
        networksSubtitle: 'Customers choose their preferred chain — you integrate once.',
        stepsTag: 'Integration',
        stepsTitle: 'Go live in minutes',
        stepsSubtitle: 'From sign-up to first payment — minimal friction.',
        steps: [
          { num: '01', title: 'Create account', desc: 'Sign up and configure your merchant settings. Get your API keys.' },
          { num: '02', title: 'Integrate', desc: 'One API call to create a Checkout Session, or embed with our frontend SDK.' },
          { num: '03', title: 'Customer pays', desc: 'Customer selects chain and token, scans QR or sends directly.' },
          { num: '04', title: 'Funds settled', desc: 'Auto-confirmed and swept to your balance. Withdraw to any on-chain address.' },
        ],
        faqTag: 'FAQ',
        faqTitle: 'Frequently asked questions',
        faqs: [
          { q: 'Which cryptocurrencies are supported?', a: 'USDT and USDC across 8 chains: TRON, Solana, BSC, Ethereum, Polygon, Arbitrum, Optimism, and Base. Note: TRON supports USDT only (no USDC).' },
          { q: 'What are the fees?', a: '0.5% per successful payment (min. 1 USDT on TRON/ETH, 0.1 USDT on other chains). No monthly fees, no setup costs, no hidden charges.' },
          { q: 'How fast is settlement?', a: 'After reaching the required on-chain confirmations (varies by chain), funds auto-sweep to your merchant balance — typically within 1–3 minutes. Withdraw anytime via API or dashboard.' },
          { q: 'Do I need to handle crypto myself?', a: 'No. All payment addresses are HD-derived. Funds auto-sweep to the platform treasury and are credited to your merchant ledger balance as USDT/USDC. Withdraw to your own wallet anytime.' },
          { q: 'What about overpayments or underpayments?', a: 'Underpaid sessions enter Underpaid status with a 24-hour rolling extension for the customer to top up. Overpaid sessions are treated as successful — the full amount is swept and the callback fires normally, with the excess recorded in transaction details. Late payments after expiry, duplicate payments, and other edge cases are routed to the Resolution Center.' },
          { q: 'What does Sandbox support?', a: 'Sandbox currently supports TRON Nile testnet only. Use sk_test_ prefixed API keys to access it. More sandbox networks are planned.' },
        ],
        ctaTitle: 'Ready to accept crypto payments?',
        ctaDesc: 'Sign up for free Sandbox access (TRON Nile testnet) and test the full payment flow with zero risk.',
        ctaBtnPrimary: 'Start for free',
        ctaBtnSecondary: 'Read the docs',
      },
)

const networks = [
  { name: 'TRON', icon: '/networks/tron.svg', tokens: 'USDT' },
  { name: 'Solana', icon: '/networks/solana.svg', tokens: 'USDT / USDC' },
  { name: 'BSC', icon: '/networks/bsc.svg', tokens: 'USDT / USDC' },
  { name: 'Ethereum', icon: '/networks/ethereum.svg', tokens: 'USDT / USDC' },
  { name: 'Polygon', icon: '/networks/polygon.svg', tokens: 'USDT / USDC' },
  { name: 'Arbitrum', icon: '/networks/arb.svg', tokens: 'USDT / USDC' },
  { name: 'Optimism', icon: '/networks/op.svg', tokens: 'USDT / USDC' },
  { name: 'Base', icon: '/networks/base.svg', tokens: 'USDT / USDC' },
]

// Scroll animation
const sections = ref<HTMLElement[]>([])
const visibleSections = ref<Set<string>>(new Set())
let observer: IntersectionObserver | null = null

function setSectionRef(el: any, id: string) {
  if (el) {
    ;(el as HTMLElement).dataset.sectionId = id
    sections.value.push(el as HTMLElement)
  }
}

onMounted(() => {
  const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)')
  if (motionQuery.matches) {
    visibleSections.value = new Set(['benefits', 'features', 'networks', 'steps', 'faq', 'cta'])
    return
  }
  observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          const id = (entry.target as HTMLElement).dataset.sectionId
          if (id) visibleSections.value.add(id)
        }
      })
    },
    { threshold: 0.1 },
  )
  sections.value.forEach((el) => observer!.observe(el))
})

onUnmounted(() => observer?.disconnect())
</script>

<template>
  <div class="ix-product-page">
    <!-- Hero -->
    <section class="ix-product-hero">
      <div class="ix-product-hero__bg">
        <div class="ix-product-hero__glow ix-product-hero__glow--1" />
        <div class="ix-product-hero__glow ix-product-hero__glow--2" />
      </div>
      <div class="ix-product-hero__container">
        <span class="ix-product-hero__tag">{{ t.heroTag }}</span>
        <h1 class="ix-product-hero__title">{{ t.heroTitle }}</h1>
        <p class="ix-product-hero__desc">{{ t.heroDesc }}</p>
        <div class="ix-product-hero__actions">
          <a :href="`${prefix}/guide/quickstart`" class="ix-product-btn ix-product-btn--primary">
            {{ t.ctaPrimary }}
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M3 8h10m0 0L9 4m4 4L9 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
          </a>
          <a :href="`${prefix}/demo`" class="ix-product-btn ix-product-btn--ghost">
            {{ t.ctaSecondary }}
          </a>
        </div>
      </div>
    </section>

    <!-- Benefits -->
    <section :ref="(el) => setSectionRef(el, 'benefits')" class="ix-product-section" :class="{ 'ix-product-section--visible': visibleSections.has('benefits') }">
      <div class="ix-product-section__inner">
        <div class="ix-product-section__header">
          <span class="ix-product-section__tag">{{ t.benefitsTag }}</span>
          <h2 class="ix-product-section__title">{{ t.benefitsTitle }}</h2>
          <p class="ix-product-section__subtitle">{{ t.benefitsSubtitle }}</p>
        </div>
        <div class="ix-product-grid ix-product-grid--2">
          <div v-for="(b, i) in t.benefits" :key="i" class="ix-product-card" :style="{ '--delay': `${i * 80}ms` }">
            <div class="ix-product-card__icon">
              <svg v-if="b.icon === 'globe'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M2 12h20"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>
              <svg v-else-if="b.icon === 'shield'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><polyline points="9 12 11 14 15 10"/></svg>
              <svg v-else-if="b.icon === 'zap'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
              <svg v-else-if="b.icon === 'lock'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
            </div>
            <h3 class="ix-product-card__title">{{ b.title }}</h3>
            <p class="ix-product-card__desc">{{ b.desc }}</p>
          </div>
        </div>
      </div>
    </section>

    <!-- Features -->
    <section :ref="(el) => setSectionRef(el, 'features')" class="ix-product-section ix-product-section--alt" :class="{ 'ix-product-section--visible': visibleSections.has('features') }">
      <div class="ix-product-section__inner">
        <div class="ix-product-section__header">
          <span class="ix-product-section__tag">{{ t.featuresTag }}</span>
          <h2 class="ix-product-section__title">{{ t.featuresTitle }}</h2>
          <p class="ix-product-section__subtitle">{{ t.featuresSubtitle }}</p>
        </div>
        <div class="ix-product-grid ix-product-grid--3">
          <div v-for="(f, i) in t.features" :key="i" class="ix-product-card ix-product-card--compact" :style="{ '--delay': `${i * 80}ms` }">
            <div class="ix-product-card__icon ix-product-card__icon--sm">
              <svg v-if="f.icon === 'flow'" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
              <svg v-else-if="f.icon === 'mobile'" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="2" width="14" height="20" rx="2"/><line x1="12" y1="18" x2="12.01" y2="18"/></svg>
              <svg v-else-if="f.icon === 'realtime'" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
              <svg v-else-if="f.icon === 'error'" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
              <svg v-else-if="f.icon === 'brand'" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="13.5" cy="6.5" r="2.5"/><circle cx="6.5" cy="13.5" r="2.5"/><circle cx="17.5" cy="17.5" r="2.5"/><path d="M8.5 8.5L15.5 15.5"/></svg>
              <svg v-else-if="f.icon === 'webhook'" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/></svg>
            </div>
            <h3 class="ix-product-card__title">{{ f.title }}</h3>
            <p class="ix-product-card__desc">{{ f.desc }}</p>
          </div>
        </div>
      </div>
    </section>

    <!-- Networks -->
    <section :ref="(el) => setSectionRef(el, 'networks')" class="ix-product-section" :class="{ 'ix-product-section--visible': visibleSections.has('networks') }">
      <div class="ix-product-section__inner">
        <div class="ix-product-section__header">
          <span class="ix-product-section__tag">{{ t.networksTag }}</span>
          <h2 class="ix-product-section__title">{{ t.networksTitle }}</h2>
          <p class="ix-product-section__subtitle">{{ t.networksSubtitle }}</p>
        </div>
        <div class="ix-product-networks">
          <div v-for="(n, i) in networks" :key="n.name" class="ix-product-network" :style="{ '--delay': `${i * 60}ms` }">
            <img :src="n.icon" :alt="n.name" width="40" height="40" class="ix-product-network__icon" />
            <span class="ix-product-network__name">{{ n.name }}</span>
            <span class="ix-product-network__tokens">{{ n.tokens }}</span>
          </div>
        </div>
      </div>
    </section>

    <!-- Steps -->
    <section :ref="(el) => setSectionRef(el, 'steps')" class="ix-product-section ix-product-section--alt" :class="{ 'ix-product-section--visible': visibleSections.has('steps') }">
      <div class="ix-product-section__inner">
        <div class="ix-product-section__header">
          <span class="ix-product-section__tag">{{ t.stepsTag }}</span>
          <h2 class="ix-product-section__title">{{ t.stepsTitle }}</h2>
          <p class="ix-product-section__subtitle">{{ t.stepsSubtitle }}</p>
        </div>
        <div class="ix-product-steps">
          <div v-for="(s, i) in t.steps" :key="s.num" class="ix-product-step" :style="{ '--delay': `${i * 120}ms` }">
            <div class="ix-product-step__num">{{ s.num }}</div>
            <div v-if="i < t.steps.length - 1" class="ix-product-step__connector">
              <svg width="100%" height="2" preserveAspectRatio="none"><line x1="0" y1="1" x2="100%" y2="1" stroke="currentColor" stroke-width="2" stroke-dasharray="6 4" /></svg>
            </div>
            <h3 class="ix-product-step__title">{{ s.title }}</h3>
            <p class="ix-product-step__desc">{{ s.desc }}</p>
          </div>
        </div>
      </div>
    </section>

    <!-- FAQ -->
    <section :ref="(el) => setSectionRef(el, 'faq')" class="ix-product-section" :class="{ 'ix-product-section--visible': visibleSections.has('faq') }">
      <div class="ix-product-section__inner ix-product-section__inner--narrow">
        <div class="ix-product-section__header">
          <span class="ix-product-section__tag">{{ t.faqTag }}</span>
          <h2 class="ix-product-section__title">{{ t.faqTitle }}</h2>
        </div>
        <div class="ix-product-faq">
          <details v-for="(faq, i) in t.faqs" :key="i" class="ix-product-faq__item">
            <summary class="ix-product-faq__question">{{ faq.q }}</summary>
            <p class="ix-product-faq__answer">{{ faq.a }}</p>
          </details>
        </div>
      </div>
    </section>

    <!-- CTA -->
    <section :ref="(el) => setSectionRef(el, 'cta')" class="ix-product-section" :class="{ 'ix-product-section--visible': visibleSections.has('cta') }">
      <div class="ix-product-section__inner">
        <div class="ix-product-cta">
          <h2 class="ix-product-cta__title">{{ t.ctaTitle }}</h2>
          <p class="ix-product-cta__desc">{{ t.ctaDesc }}</p>
          <div class="ix-product-cta__actions">
            <a href="https://app.ironixpay.com" target="_blank" rel="noopener noreferrer" class="ix-product-btn ix-product-btn--primary">
              {{ t.ctaBtnPrimary }}
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M3 8h10m0 0L9 4m4 4L9 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
            </a>
            <a :href="`${prefix}/guide/checkout`" class="ix-product-btn ix-product-btn--ghost">
              {{ t.ctaBtnSecondary }}
            </a>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
/* ═══ Product Page Shared Styles ═══ */
.ix-product-page {
  max-width: 100%;
  overflow: hidden;
}

/* ─── Hero ─── */
.ix-product-hero {
  position: relative;
  padding: 100px 24px 80px;
  text-align: center;
  overflow: hidden;
}

.ix-product-hero__bg {
  position: absolute;
  inset: 0;
  z-index: 0;
  overflow: hidden;
}

.ix-product-hero__glow {
  position: absolute;
  border-radius: 50%;
  filter: blur(120px);
}

.ix-product-hero__glow--1 {
  width: 500px;
  height: 500px;
  background: radial-gradient(circle, rgba(37, 99, 235, 0.12) 0%, transparent 70%);
  top: -150px;
  left: 50%;
  transform: translateX(-50%);
}

.ix-product-hero__glow--2 {
  width: 400px;
  height: 400px;
  background: radial-gradient(circle, rgba(124, 58, 237, 0.08) 0%, transparent 70%);
  bottom: -100px;
  right: -100px;
}

.ix-product-hero__container {
  position: relative;
  z-index: 1;
  max-width: 720px;
  margin: 0 auto;
}

.ix-product-hero__tag {
  display: inline-block;
  font-family: 'Exo 2', sans-serif;
  font-size: 0.78rem;
  font-weight: 600;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: #3b82f6;
  margin-bottom: 20px;
}

.ix-product-hero__title {
  font-family: 'Exo 2', sans-serif;
  font-size: 3rem;
  font-weight: 800;
  letter-spacing: -0.03em;
  line-height: 1.12;
  color: #0f172a;
  margin: 0 0 20px;
}

.ix-product-hero__desc {
  font-size: 1.1rem;
  line-height: 1.7;
  color: #475569;
  margin: 0 0 32px;
}

.ix-product-hero__actions {
  display: flex;
  gap: 14px;
  justify-content: center;
}

/* ─── Buttons ─── */
.ix-product-btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 14px 28px;
  border-radius: 12px;
  font-size: 0.92rem;
  font-weight: 600;
  text-decoration: none;
  cursor: pointer;
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  border: none;
}

.ix-product-btn--primary {
  background: linear-gradient(135deg, #2563eb, #3b82f6);
  color: #fff;
  box-shadow: 0 4px 20px rgba(37, 99, 235, 0.35), inset 0 1px 0 rgba(255, 255, 255, 0.1);
}

.ix-product-btn--primary:hover {
  transform: translateY(-2px);
  box-shadow: 0 8px 30px rgba(37, 99, 235, 0.45);
}

.ix-product-btn--ghost {
  background: rgba(0, 0, 0, 0.04);
  color: #334155;
  border: 1px solid rgba(0, 0, 0, 0.12);
}

.ix-product-btn--ghost:hover {
  background: rgba(0, 0, 0, 0.08);
  color: #0f172a;
  border-color: rgba(0, 0, 0, 0.2);
  transform: translateY(-1px);
}

/* ─── Sections ─── */
.ix-product-section {
  padding: 80px 24px;
}

.ix-product-section--alt {
  background: #f8fafc;
}

.ix-product-section__inner {
  max-width: 1100px;
  margin: 0 auto;
}

.ix-product-section__inner--narrow {
  max-width: 720px;
}

.ix-product-section__header {
  text-align: center;
  margin-bottom: 48px;
  max-width: 600px;
  margin-left: auto;
  margin-right: auto;
}

.ix-product-section__tag {
  display: inline-block;
  font-family: 'Exo 2', sans-serif;
  font-size: 0.75rem;
  font-weight: 600;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: #3b82f6;
  margin-bottom: 14px;
}

.ix-product-section__title {
  font-family: 'Exo 2', sans-serif;
  font-size: 2.2rem;
  font-weight: 800;
  letter-spacing: -0.03em;
  line-height: 1.15;
  color: #0f172a;
  margin: 0 0 12px;
}

.ix-product-section__subtitle {
  font-size: 1.05rem;
  line-height: 1.6;
  color: #64748b;
  margin: 0;
}

/* ─── Grid ─── */
.ix-product-grid {
  display: grid;
  gap: 20px;
  width: 100%;
}

.ix-product-grid--2 {
  grid-template-columns: repeat(2, 1fr);
}

.ix-product-grid--3 {
  grid-template-columns: repeat(3, 1fr);
}

/* ─── Cards ─── */
.ix-product-card {
  display: flex;
  flex-direction: column;
  padding: 28px 24px;
  border-radius: 20px;
  background: #ffffff;
  border: 1px solid #e5e7eb;
  transition: transform 0.25s ease-out, border-color 0.25s ease-out, box-shadow 0.25s ease-out;
  opacity: 0;
  transform: translateY(20px);
}

.ix-product-section--visible .ix-product-card {
  opacity: 1;
  transform: translateY(0);
  transition: opacity 0.5s ease-out var(--delay), transform 0.5s ease-out var(--delay), border-color 0.25s ease-out, box-shadow 0.25s ease-out;
}

.ix-product-card:hover {
  transform: translateY(-4px);
  border-color: #bfdbfe;
  box-shadow: 0 8px 30px rgba(37, 99, 235, 0.08);
}

.ix-product-card--compact {
  padding: 24px 20px;
}

.ix-product-card__icon {
  width: 44px;
  height: 44px;
  border-radius: 12px;
  background: #eff6ff;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #2563eb;
  margin-bottom: 16px;
  flex-shrink: 0;
}

.ix-product-card__icon--sm {
  width: 38px;
  height: 38px;
  border-radius: 10px;
  margin-bottom: 14px;
}

.ix-product-card__title {
  font-family: 'Exo 2', sans-serif;
  font-size: 1.1rem;
  font-weight: 700;
  color: #111827;
  margin: 0 0 8px;
  line-height: 1.3;
}

.ix-product-card__desc {
  font-size: 0.88rem;
  line-height: 1.6;
  color: #6b7280;
  margin: 0;
}

/* ─── Networks ─── */
.ix-product-networks {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 16px;
}

.ix-product-network {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 24px 16px;
  border-radius: 16px;
  background: #ffffff;
  border: 1px solid #e5e7eb;
  transition: transform 0.25s ease-out, border-color 0.25s ease-out, box-shadow 0.25s ease-out;
  opacity: 0;
  transform: translateY(16px);
}

.ix-product-section--visible .ix-product-network {
  opacity: 1;
  transform: translateY(0);
  transition: opacity 0.5s ease-out var(--delay), transform 0.5s ease-out var(--delay), border-color 0.25s ease-out, box-shadow 0.25s ease-out;
}

.ix-product-network:hover {
  transform: translateY(-3px);
  border-color: #bfdbfe;
  box-shadow: 0 6px 20px rgba(37, 99, 235, 0.06);
}

.ix-product-network__icon {
  width: 40px;
  height: 40px;
}

.ix-product-network__name {
  font-family: 'Exo 2', sans-serif;
  font-size: 0.9rem;
  font-weight: 700;
  color: #111827;
}

.ix-product-network__tokens {
  font-size: 0.75rem;
  color: #64748b;
  font-weight: 500;
}

/* ─── Steps ─── */
.ix-product-steps {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 0;
  width: 100%;
}

.ix-product-step {
  position: relative;
  padding: 0 20px;
  text-align: center;
  opacity: 0;
  transform: translateY(16px);
}

.ix-product-section--visible .ix-product-step {
  opacity: 1;
  transform: translateY(0);
  transition: opacity 0.5s ease-out var(--delay), transform 0.5s ease-out var(--delay);
}

.ix-product-step__num {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 48px;
  height: 48px;
  border-radius: 14px;
  background: linear-gradient(135deg, rgba(37, 99, 235, 0.1), rgba(99, 102, 241, 0.06));
  border: 1px solid rgba(37, 99, 235, 0.2);
  font-family: 'Exo 2', sans-serif;
  font-size: 1rem;
  font-weight: 800;
  color: #2563eb;
  margin-bottom: 18px;
}

.ix-product-step__connector {
  position: absolute;
  top: 24px;
  left: calc(50% + 32px);
  right: calc(-50% + 32px);
  color: rgba(37, 99, 235, 0.2);
  pointer-events: none;
}

.ix-product-step__title {
  font-family: 'Exo 2', sans-serif;
  font-size: 1.05rem;
  font-weight: 700;
  color: #0f172a;
  margin: 0 0 8px;
  padding: 0;
  border: none;
}

.ix-product-step__desc {
  font-size: 0.85rem;
  color: #64748b;
  line-height: 1.6;
  margin: 0;
}

/* ─── FAQ ─── */
.ix-product-faq {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.ix-product-faq__item {
  border: 1px solid #e5e7eb;
  border-radius: 14px;
  overflow: hidden;
  background: #ffffff;
  transition: border-color 0.2s ease;
}

.ix-product-faq__item[open] {
  border-color: #bfdbfe;
}

.ix-product-faq__question {
  padding: 18px 24px;
  font-size: 0.95rem;
  font-weight: 600;
  color: #111827;
  cursor: pointer;
  list-style: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.ix-product-faq__question::-webkit-details-marker {
  display: none;
}

.ix-product-faq__question::after {
  content: '+';
  font-size: 1.2rem;
  font-weight: 300;
  color: #94a3b8;
  transition: transform 0.2s ease;
}

.ix-product-faq__item[open] .ix-product-faq__question::after {
  content: '−';
}

.ix-product-faq__answer {
  padding: 0 24px 18px;
  font-size: 0.9rem;
  line-height: 1.7;
  color: #6b7280;
  margin: 0;
}

/* ─── CTA ─── */
.ix-product-cta {
  text-align: center;
  padding: 56px 40px;
  border-radius: 24px;
  background: linear-gradient(135deg, #eff6ff 0%, #f8fafc 100%);
  border: 1px solid #dbeafe;
}

.ix-product-cta__title {
  font-family: 'Exo 2', sans-serif;
  font-size: 2rem;
  font-weight: 800;
  color: #0f172a;
  margin: 0 0 12px;
  letter-spacing: -0.02em;
}

.ix-product-cta__desc {
  font-size: 1.05rem;
  color: #64748b;
  margin: 0 0 32px;
  line-height: 1.6;
}

.ix-product-cta__actions {
  display: flex;
  gap: 14px;
  justify-content: center;
}

/* ═══ Dark Mode ═══ */
.dark .ix-product-hero__title { color: #f1f5f9; }
.dark .ix-product-hero__desc { color: #94a3b8; }
.dark .ix-product-hero__tag { color: #60a5fa; }

.dark .ix-product-btn--ghost {
  background: rgba(255, 255, 255, 0.06);
  color: #cbd5e1;
  border-color: rgba(255, 255, 255, 0.12);
}
.dark .ix-product-btn--ghost:hover {
  background: rgba(255, 255, 255, 0.1);
  color: #f1f5f9;
  border-color: rgba(255, 255, 255, 0.2);
}

.dark .ix-product-section__tag { color: #60a5fa; }
.dark .ix-product-section__title { color: #f3f4f6; }
.dark .ix-product-section__subtitle { color: #94a3b8; }
.dark .ix-product-section--alt { background: #0f1420; }

.dark .ix-product-card {
  background: #131a2b;
  border-color: rgba(255, 255, 255, 0.06);
}
.dark .ix-product-card:hover {
  border-color: rgba(59, 130, 246, 0.35);
  box-shadow: 0 8px 30px rgba(59, 130, 246, 0.08);
}
.dark .ix-product-card__icon {
  background: rgba(59, 130, 246, 0.1);
  color: #60a5fa;
}
.dark .ix-product-card__title { color: #f3f4f6; }
.dark .ix-product-card__desc { color: #94a3b8; }

.dark .ix-product-network {
  background: #131a2b;
  border-color: rgba(255, 255, 255, 0.06);
}
.dark .ix-product-network:hover {
  border-color: rgba(59, 130, 246, 0.35);
  box-shadow: 0 6px 20px rgba(59, 130, 246, 0.06);
}
.dark .ix-product-network__name { color: #f3f4f6; }

.dark .ix-product-step__num {
  background: linear-gradient(135deg, rgba(59, 130, 246, 0.15), rgba(99, 102, 241, 0.1));
  border-color: rgba(59, 130, 246, 0.25);
  color: #60a5fa;
}
.dark .ix-product-step__connector { color: rgba(59, 130, 246, 0.25); }
.dark .ix-product-step__title { color: #f1f5f9; }
.dark .ix-product-step__desc { color: #94a3b8; }

.dark .ix-product-faq__item {
  background: #131a2b;
  border-color: rgba(255, 255, 255, 0.06);
}
.dark .ix-product-faq__item[open] { border-color: rgba(59, 130, 246, 0.3); }
.dark .ix-product-faq__question { color: #f3f4f6; }
.dark .ix-product-faq__question::after { color: #64748b; }
.dark .ix-product-faq__answer { color: #94a3b8; }

.dark .ix-product-cta {
  background: linear-gradient(135deg, rgba(59, 130, 246, 0.08) 0%, #111827 100%);
  border-color: rgba(255, 255, 255, 0.06);
}
.dark .ix-product-cta__title { color: #f3f4f6; }
.dark .ix-product-cta__desc { color: #94a3b8; }

/* ═══ Responsive ═══ */
@media (max-width: 1023px) {
  .ix-product-grid--3 { grid-template-columns: repeat(2, 1fr); }
  .ix-product-networks { grid-template-columns: repeat(4, 1fr); }
}

@media (max-width: 768px) {
  .ix-product-hero { padding: 80px 20px 60px; }
  .ix-product-hero__title { font-size: 2.2rem; }
  .ix-product-hero__actions { flex-direction: column; align-items: center; }
  .ix-product-grid--2 { grid-template-columns: 1fr; }
  .ix-product-grid--3 { grid-template-columns: 1fr; }
  .ix-product-networks { grid-template-columns: repeat(2, 1fr); }
  .ix-product-steps { grid-template-columns: repeat(2, 1fr); gap: 28px 0; }
  .ix-product-step__connector { display: none; }
  .ix-product-section__title { font-size: 1.75rem; }
  .ix-product-cta { padding: 40px 24px; }
  .ix-product-cta__title { font-size: 1.6rem; }
  .ix-product-cta__actions { flex-direction: column; align-items: center; }
}

@media (max-width: 480px) {
  .ix-product-hero__title { font-size: 1.8rem; }
  .ix-product-networks { grid-template-columns: repeat(2, 1fr); }
  .ix-product-steps { grid-template-columns: 1fr; gap: 24px; }
}

/* ═══ Reduced Motion ═══ */
@media (prefers-reduced-motion: reduce) {
  .ix-product-card,
  .ix-product-network,
  .ix-product-step {
    opacity: 1;
    transform: none;
  }
  .ix-product-card:hover,
  .ix-product-network:hover {
    transform: none;
  }
}

/* ═══ VitePress overrides ═══ */
.vp-doc .ix-product-page h1,
.vp-doc .ix-product-page h2,
.vp-doc .ix-product-page h3 {
  border: none;
  margin-top: 0;
  padding-top: 0;
}
</style>
