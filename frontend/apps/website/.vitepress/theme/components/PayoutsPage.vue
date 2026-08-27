<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { useData } from 'vitepress'

const { lang } = useData()
const isZh = computed(() => lang.value === 'zh-CN')
const prefix = computed(() => (isZh.value ? '' : '/en'))

const t = computed(() =>
  isZh.value
    ? {
        heroTag: '出款产品',
        heroTitle: '通过 API 实现全球加密货币出款',
        heroDesc: '将 USDT/USDC 从商户余额发送至任意链上地址。指定链和币种，一次 API 调用完成出款，Idempotency-Key 保证不重复扣款。',
        ctaPrimary: '开始接入',
        ctaSecondary: '查看文档',
        benefitsTag: '为什么用加密货币出款',
        benefitsTitle: '更快、更便宜、无国界',
        benefitsSubtitle: '告别传统跨境汇款的高成本和长周期。',
        benefits: [
          { icon: 'cost', title: '大幅降低成本', desc: '避免银行电汇手续费和高额汇率差价。链上出款费用远低于传统渠道。' },
          { icon: 'globe', title: '无国界出款', desc: '无需当地银行账户或国际清算。覆盖全球 — 只要收款方有链上地址。' },
          { icon: 'stable', title: '稳定币避免波动', desc: '使用 USDT/USDC 出款，锁定价值，收款方收到的就是你发送的金额。' },
          { icon: 'scale', title: '可编程自动化', desc: '通过 API 集成到后台系统实现自动出款，适用于代理佣金、返利结算、合作伙伴付款等场景。支持自动提现规则。' },
        ],
        workflowsTag: '出款方式',
        workflowsTitle: '灵活的出款工作流',
        workflowsSubtitle: '根据你的业务场景选择合适的出款方式。',
        workflows: [
          { icon: 'single', title: 'API Payout', desc: '通过 Payout API 将余额发送至任意链上地址。适合向外部用户、供应商或承包商付款。', bullets: ['Idempotency-Key 防重复', '目标地址 AML 自动审查', 'payout.completed / failed Webhook'] },
          { icon: 'batch', title: '控制台提现', desc: '通过商户控制台将余额提现至自己的 collection address。需要 2FA 验证。', bullets: ['JWT + 强制 2FA 认证', '提现至商户预设地址', '风控规则可配置审批流'] },
          { icon: 'auto', title: '自动提现', desc: '配置全局余额阈值，系统每 5 分钟自动检查所有链和币种，将超出部分提现至商户地址。', bullets: ['单一阈值自动应用于所有链和币种', '自动跳过风控审批', '无需人工干预'] },
        ],
        networksTag: '支持的网络',
        networksTitle: '跨链出款，统一接口',
        networksSubtitle: '一套 API 覆盖所有链。收款方可在任何支持的链上收款。',
        complianceTag: '合规与安全',
        complianceTitle: '每笔出款都受保护',
        complianceSubtitle: '内置合规工具，确保出款安全可控。',
        complianceItems: [
          { icon: 'aml', title: '出金地址审查', desc: 'API Payout 出款目标地址自动通过 OFAC 黑名单 + GoPlus API 双重筛查（提现至商户自有地址不适用）。' },
          { icon: 'log', title: '完整审计日志', desc: '每笔出款包含 TXID、时间戳、手续费、链信息等完整记录。' },
          { icon: 'role', title: '权限管理', desc: '基于角色的访问控制，定义谁可以发起、审批出款。' },
          { icon: 'retry', title: '幂等重试', desc: '相同幂等键保证不重复扣款，网络抖动无忧。' },
        ],
        faqTag: '常见问题',
        faqTitle: 'FAQ',
        faqs: [
          { q: '支持哪些币种和链？', a: '支持 USDT 和 USDC，覆盖 TRON、Solana、BSC、Ethereum、Polygon、Arbitrum、Optimism、Base 共 8 条链。注意：TRON 仅支持 USDT。' },
          { q: '出款手续费是多少？', a: '按网络收取固定费用（含 gas）：TRON $1.50，其他链均为 $0.50。从出款金额中扣除。' },
          { q: '有金额限制吗？', a: 'API Payout 有限额：单笔上限 10,000 USDT/USDC，每个商户每日（UTC）累计上限 50,000 USDT/USDC。控制台提现和自动提现无金额限制。' },
          { q: '出款多久到账？', a: '交易广播后需等待链上确认。确认时间因链而异，广播后可通过 Webhook（payout.completed / payout.failed）或 API 轮询追踪状态。' },
          { q: '收款方需要 IronixPay 账户吗？', a: '不需要。收款方只需一个兼容的链上钱包地址。资金直接发送到链上。' },
          { q: '出款失败怎么办？', a: '如果链上广播或执行失败，扣款金额会自动退回商户余额，并通过 payout.failed Webhook 通知。' },
        ],
        ctaTitle: '准备好通过 API 出款了吗？',
        ctaDesc: '注册即可获取 Sandbox 环境（TRON Nile 测试网），零风险测试出款流程。',
        ctaBtnPrimary: '免费开始',
        ctaBtnSecondary: '查看文档',
      }
    : {
        heroTag: 'Crypto Payouts',
        heroTitle: 'Send crypto payouts globally via API',
        heroDesc: 'Send USDT/USDC from your merchant balance to any on-chain address. Specify chain and token, one API call — with Idempotency-Key to prevent duplicate debits.',
        ctaPrimary: 'Get Started',
        ctaSecondary: 'Read the Docs',
        benefitsTag: 'Why crypto payouts',
        benefitsTitle: 'Faster, cheaper, borderless',
        benefitsSubtitle: 'Leave behind the high costs and slow speeds of traditional cross-border payments.',
        benefits: [
          { icon: 'cost', title: 'Lower costs dramatically', desc: 'Skip banking fees and FX margins. On-chain payouts cost a fraction of traditional wire transfers.' },
          { icon: 'globe', title: 'Send without borders', desc: 'No local bank accounts or international clearing needed. Reach anyone with an on-chain address.' },
          { icon: 'stable', title: 'Stablecoin precision', desc: 'Pay in USDT/USDC — recipients receive exactly what you send. No volatility risk.' },
          { icon: 'scale', title: 'Programmable automation', desc: 'Integrate via API for automated disbursements — affiliate commissions, rebates, partner settlements. Auto-withdrawal rules supported.' },
        ],
        workflowsTag: 'Payout Workflows',
        workflowsTitle: 'Flexible payout methods',
        workflowsSubtitle: 'Choose the workflow that matches your operations.',
        workflows: [
          { icon: 'single', title: 'API Payout', desc: 'Send from your balance to any on-chain address via the Payout API. Ideal for paying external users, suppliers, or contractors.', bullets: ['Idempotency-Key prevents duplicates', 'Target address AML screening', 'payout.completed / failed webhooks'] },
          { icon: 'batch', title: 'Dashboard Withdrawal', desc: 'Withdraw balance to your own collection address via the merchant dashboard. Requires 2FA verification.', bullets: ['JWT + mandatory 2FA auth', 'Withdraw to preset merchant address', 'Configurable risk approval rules'] },
          { icon: 'auto', title: 'Auto-Withdrawal', desc: 'Set a global balance threshold — the system checks every 5 minutes across all chains and tokens, auto-withdrawing excess to your address.', bullets: ['Single threshold applied to all chains and tokens', 'Bypasses risk approval', 'Fully hands-off operation'] },
        ],
        networksTag: 'Supported Networks',
        networksTitle: 'Cross-chain payouts, unified API',
        networksSubtitle: 'One API for all chains. Recipients can receive on any supported network.',
        complianceTag: 'Compliance & Security',
        complianceTitle: 'Every payout is protected',
        complianceSubtitle: 'Built-in compliance tools to keep your payouts safe and auditable.',
        complianceItems: [
          { icon: 'aml', title: 'Address screening', desc: 'API Payout target addresses are automatically screened against OFAC blacklist + GoPlus API (not applied to withdrawals to your own address).' },
          { icon: 'log', title: 'Full audit trail', desc: 'Each payout includes TXID, timestamp, fees, chain info, and complete lifecycle records.' },
          { icon: 'role', title: 'Role-based access', desc: 'Define who can initiate, approve, and manage payouts with granular permissions.' },
          { icon: 'retry', title: 'Idempotent retries', desc: 'Same idempotency key guarantees no duplicate debits. Network jitter? No problem.' },
        ],
        faqTag: 'FAQ',
        faqTitle: 'Frequently asked questions',
        faqs: [
          { q: 'Which tokens and chains are supported?', a: 'USDT and USDC across 8 chains: TRON, Solana, BSC, Ethereum, Polygon, Arbitrum, Optimism, and Base. Note: TRON supports USDT only.' },
          { q: 'What are the payout fees?', a: 'Fixed per-network fees (gas included): TRON $1.50, all other chains $0.50 each. Deducted from the payout amount.' },
          { q: 'Are there amount limits?', a: 'API Payouts have limits: single max 10,000 USDT/USDC, daily aggregate max 50,000 USDT/USDC (UTC). Dashboard withdrawals and auto-withdrawals have no amount limits.' },
          { q: 'How fast are payouts?', a: 'After broadcast, payouts await on-chain confirmation. Confirmation time varies by chain. Track status via payout.completed / payout.failed webhooks or API polling.' },
          { q: 'Do recipients need an IronixPay account?', a: 'No. Recipients only need a compatible on-chain wallet address. Funds are sent directly on-chain.' },
          { q: 'What happens if a payout fails?', a: 'If the on-chain broadcast or execution fails, the debited amount is automatically returned to your merchant balance with a payout.failed webhook notification.' },
        ],
        ctaTitle: 'Ready to send crypto payouts via API?',
        ctaDesc: 'Sign up for free Sandbox access (TRON Nile testnet) and test the payout flow with zero risk.',
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
const sectionEls = ref<HTMLElement[]>([])
const visibleSections = ref<Set<string>>(new Set())
let observer: IntersectionObserver | null = null

function setSectionRef(el: any, id: string) {
  if (el) {
    ;(el as HTMLElement).dataset.sectionId = id
    sectionEls.value.push(el as HTMLElement)
  }
}

onMounted(() => {
  const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)')
  if (motionQuery.matches) {
    visibleSections.value = new Set(['benefits', 'workflows', 'networks', 'compliance', 'faq', 'cta'])
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
  sectionEls.value.forEach((el) => observer!.observe(el))
})

onUnmounted(() => observer?.disconnect())
</script>

<template>
  <div class="ix-product-page">
    <!-- Hero -->
    <section class="ix-product-hero">
      <div class="ix-product-hero__bg">
        <div class="ix-product-hero__glow ix-product-hero__glow--1" />
        <div class="ix-product-hero__glow ix-product-hero__glow--payout" />
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
          <a :href="`${prefix}/guide/payouts`" class="ix-product-btn ix-product-btn--ghost">
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
              <svg v-if="b.icon === 'cost'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="1" x2="12" y2="23"/><path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"/></svg>
              <svg v-else-if="b.icon === 'globe'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M2 12h20"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>
              <svg v-else-if="b.icon === 'stable'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><path d="M12 8v4l2 2"/></svg>
              <svg v-else-if="b.icon === 'scale'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="7" width="20" height="14" rx="2"/><path d="M16 7V4a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v3"/><line x1="12" y1="12" x2="12" y2="16"/><line x1="10" y1="14" x2="14" y2="14"/></svg>
            </div>
            <h3 class="ix-product-card__title">{{ b.title }}</h3>
            <p class="ix-product-card__desc">{{ b.desc }}</p>
          </div>
        </div>
      </div>
    </section>

    <!-- Workflows -->
    <section :ref="(el) => setSectionRef(el, 'workflows')" class="ix-product-section ix-product-section--alt" :class="{ 'ix-product-section--visible': visibleSections.has('workflows') }">
      <div class="ix-product-section__inner">
        <div class="ix-product-section__header">
          <span class="ix-product-section__tag">{{ t.workflowsTag }}</span>
          <h2 class="ix-product-section__title">{{ t.workflowsTitle }}</h2>
          <p class="ix-product-section__subtitle">{{ t.workflowsSubtitle }}</p>
        </div>
        <div class="ix-product-grid ix-product-grid--3">
          <div v-for="(w, i) in t.workflows" :key="i" class="ix-product-card ix-product-card--workflow" :style="{ '--delay': `${i * 100}ms` }">
            <div class="ix-product-card__icon">
              <svg v-if="w.icon === 'single'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><line x1="7" y1="17" x2="17" y2="7"/><polyline points="7 7 17 7 17 17"/></svg>
              <svg v-else-if="w.icon === 'batch'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="3" y1="9" x2="21" y2="9"/><line x1="3" y1="15" x2="21" y2="15"/><line x1="9" y1="3" x2="9" y2="21"/></svg>
              <svg v-else-if="w.icon === 'auto'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 3 21 3 21 8"/><line x1="4" y1="20" x2="21" y2="3"/><polyline points="21 16 21 21 16 21"/><line x1="15" y1="15" x2="21" y2="21"/><line x1="4" y1="4" x2="9" y2="9"/></svg>
            </div>
            <h3 class="ix-product-card__title">{{ w.title }}</h3>
            <p class="ix-product-card__desc">{{ w.desc }}</p>
            <ul class="ix-product-card__bullets">
              <li v-for="bullet in w.bullets" :key="bullet">{{ bullet }}</li>
            </ul>
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

    <!-- Compliance -->
    <section :ref="(el) => setSectionRef(el, 'compliance')" class="ix-product-section ix-product-section--alt" :class="{ 'ix-product-section--visible': visibleSections.has('compliance') }">
      <div class="ix-product-section__inner">
        <div class="ix-product-section__header">
          <span class="ix-product-section__tag">{{ t.complianceTag }}</span>
          <h2 class="ix-product-section__title">{{ t.complianceTitle }}</h2>
          <p class="ix-product-section__subtitle">{{ t.complianceSubtitle }}</p>
        </div>
        <div class="ix-product-grid ix-product-grid--2">
          <div v-for="(c, i) in t.complianceItems" :key="i" class="ix-product-card" :style="{ '--delay': `${i * 80}ms` }">
            <div class="ix-product-card__icon">
              <svg v-if="c.icon === 'aml'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><polyline points="9 12 11 14 15 10"/></svg>
              <svg v-else-if="c.icon === 'log'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/></svg>
              <svg v-else-if="c.icon === 'role'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>
              <svg v-else-if="c.icon === 'retry'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>
            </div>
            <h3 class="ix-product-card__title">{{ c.title }}</h3>
            <p class="ix-product-card__desc">{{ c.desc }}</p>
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
            <a :href="`${prefix}/guide/payouts`" class="ix-product-btn ix-product-btn--ghost">
              {{ t.ctaBtnSecondary }}
            </a>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
/* Reuse product page styles from CheckoutPage — only add payout-specific overrides */
.ix-product-page { max-width: 100%; overflow: hidden; }

/* ─── Hero ─── */
.ix-product-hero { position: relative; padding: 100px 24px 80px; text-align: center; overflow: hidden; }
.ix-product-hero__bg { position: absolute; inset: 0; z-index: 0; overflow: hidden; }
.ix-product-hero__glow { position: absolute; border-radius: 50%; filter: blur(120px); }
.ix-product-hero__glow--1 { width: 500px; height: 500px; background: radial-gradient(circle, rgba(37, 99, 235, 0.12) 0%, transparent 70%); top: -150px; left: 50%; transform: translateX(-50%); }
.ix-product-hero__glow--payout { width: 400px; height: 400px; background: radial-gradient(circle, rgba(16, 185, 129, 0.1) 0%, transparent 70%); bottom: -100px; left: -100px; }
.ix-product-hero__container { position: relative; z-index: 1; max-width: 720px; margin: 0 auto; }
.ix-product-hero__tag { display: inline-block; font-family: 'Exo 2', sans-serif; font-size: 0.78rem; font-weight: 600; letter-spacing: 0.14em; text-transform: uppercase; color: #3b82f6; margin-bottom: 20px; }
.ix-product-hero__title { font-family: 'Exo 2', sans-serif; font-size: 3rem; font-weight: 800; letter-spacing: -0.03em; line-height: 1.12; color: #0f172a; margin: 0 0 20px; }
.ix-product-hero__desc { font-size: 1.1rem; line-height: 1.7; color: #475569; margin: 0 0 32px; }
.ix-product-hero__actions { display: flex; gap: 14px; justify-content: center; }

/* ─── Buttons ─── */
.ix-product-btn { display: inline-flex; align-items: center; gap: 8px; padding: 14px 28px; border-radius: 12px; font-size: 0.92rem; font-weight: 600; text-decoration: none; cursor: pointer; transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1); border: none; }
.ix-product-btn--primary { background: linear-gradient(135deg, #2563eb, #3b82f6); color: #fff; box-shadow: 0 4px 20px rgba(37, 99, 235, 0.35), inset 0 1px 0 rgba(255, 255, 255, 0.1); }
.ix-product-btn--primary:hover { transform: translateY(-2px); box-shadow: 0 8px 30px rgba(37, 99, 235, 0.45); }
.ix-product-btn--ghost { background: rgba(0, 0, 0, 0.04); color: #334155; border: 1px solid rgba(0, 0, 0, 0.12); }
.ix-product-btn--ghost:hover { background: rgba(0, 0, 0, 0.08); color: #0f172a; border-color: rgba(0, 0, 0, 0.2); transform: translateY(-1px); }

/* ─── Sections ─── */
.ix-product-section { padding: 80px 24px; }
.ix-product-section--alt { background: #f8fafc; }
.ix-product-section__inner { max-width: 1100px; margin: 0 auto; }
.ix-product-section__inner--narrow { max-width: 720px; }
.ix-product-section__header { text-align: center; margin-bottom: 48px; max-width: 600px; margin-left: auto; margin-right: auto; }
.ix-product-section__tag { display: inline-block; font-family: 'Exo 2', sans-serif; font-size: 0.75rem; font-weight: 600; letter-spacing: 0.14em; text-transform: uppercase; color: #3b82f6; margin-bottom: 14px; }
.ix-product-section__title { font-family: 'Exo 2', sans-serif; font-size: 2.2rem; font-weight: 800; letter-spacing: -0.03em; line-height: 1.15; color: #0f172a; margin: 0 0 12px; }
.ix-product-section__subtitle { font-size: 1.05rem; line-height: 1.6; color: #64748b; margin: 0; }

/* ─── Grid ─── */
.ix-product-grid { display: grid; gap: 20px; width: 100%; }
.ix-product-grid--2 { grid-template-columns: repeat(2, 1fr); }
.ix-product-grid--3 { grid-template-columns: repeat(3, 1fr); }

/* ─── Cards ─── */
.ix-product-card { display: flex; flex-direction: column; padding: 28px 24px; border-radius: 20px; background: #ffffff; border: 1px solid #e5e7eb; transition: transform 0.25s ease-out, border-color 0.25s ease-out, box-shadow 0.25s ease-out; opacity: 0; transform: translateY(20px); }
.ix-product-section--visible .ix-product-card { opacity: 1; transform: translateY(0); transition: opacity 0.5s ease-out var(--delay), transform 0.5s ease-out var(--delay), border-color 0.25s ease-out, box-shadow 0.25s ease-out; }
.ix-product-card:hover { transform: translateY(-4px); border-color: #bfdbfe; box-shadow: 0 8px 30px rgba(37, 99, 235, 0.08); }
.ix-product-card__icon { width: 44px; height: 44px; border-radius: 12px; background: #eff6ff; display: flex; align-items: center; justify-content: center; color: #2563eb; margin-bottom: 16px; flex-shrink: 0; }
.ix-product-card__title { font-family: 'Exo 2', sans-serif; font-size: 1.1rem; font-weight: 700; color: #111827; margin: 0 0 8px; line-height: 1.3; }
.ix-product-card__desc { font-size: 0.88rem; line-height: 1.6; color: #6b7280; margin: 0; }

.ix-product-card__bullets { list-style: none; padding: 0; margin: 14px 0 0; display: flex; flex-direction: column; gap: 6px; }
.ix-product-card__bullets li { font-size: 0.82rem; color: #6b7280; padding-left: 18px; position: relative; line-height: 1.5; }
.ix-product-card__bullets li::before { content: '✓'; position: absolute; left: 0; color: #22c55e; font-weight: 700; font-size: 0.75rem; }

/* ─── Networks ─── */
.ix-product-networks { display: grid; grid-template-columns: repeat(4, 1fr); gap: 16px; }
.ix-product-network { display: flex; flex-direction: column; align-items: center; gap: 10px; padding: 24px 16px; border-radius: 16px; background: #ffffff; border: 1px solid #e5e7eb; transition: transform 0.25s ease-out, border-color 0.25s ease-out, box-shadow 0.25s ease-out; opacity: 0; transform: translateY(16px); }
.ix-product-section--visible .ix-product-network { opacity: 1; transform: translateY(0); transition: opacity 0.5s ease-out var(--delay), transform 0.5s ease-out var(--delay), border-color 0.25s ease-out, box-shadow 0.25s ease-out; }
.ix-product-network:hover { transform: translateY(-3px); border-color: #bfdbfe; box-shadow: 0 6px 20px rgba(37, 99, 235, 0.06); }
.ix-product-network__icon { width: 40px; height: 40px; }
.ix-product-network__name { font-family: 'Exo 2', sans-serif; font-size: 0.9rem; font-weight: 700; color: #111827; }
.ix-product-network__fee { font-size: 0.85rem; color: #2563eb; font-weight: 700; font-family: 'Exo 2', sans-serif; }
.ix-product-network__tokens { font-size: 0.72rem; color: #64748b; font-weight: 500; }

/* ─── FAQ ─── */
.ix-product-faq { display: flex; flex-direction: column; gap: 12px; }
.ix-product-faq__item { border: 1px solid #e5e7eb; border-radius: 14px; overflow: hidden; background: #ffffff; transition: border-color 0.2s ease; }
.ix-product-faq__item[open] { border-color: #bfdbfe; }
.ix-product-faq__question { padding: 18px 24px; font-size: 0.95rem; font-weight: 600; color: #111827; cursor: pointer; list-style: none; display: flex; align-items: center; justify-content: space-between; }
.ix-product-faq__question::-webkit-details-marker { display: none; }
.ix-product-faq__question::after { content: '+'; font-size: 1.2rem; font-weight: 300; color: #94a3b8; transition: transform 0.2s ease; }
.ix-product-faq__item[open] .ix-product-faq__question::after { content: '−'; }
.ix-product-faq__answer { padding: 0 24px 18px; font-size: 0.9rem; line-height: 1.7; color: #6b7280; margin: 0; }

/* ─── CTA ─── */
.ix-product-cta { text-align: center; padding: 56px 40px; border-radius: 24px; background: linear-gradient(135deg, #eff6ff 0%, #f8fafc 100%); border: 1px solid #dbeafe; }
.ix-product-cta__title { font-family: 'Exo 2', sans-serif; font-size: 2rem; font-weight: 800; color: #0f172a; margin: 0 0 12px; letter-spacing: -0.02em; }
.ix-product-cta__desc { font-size: 1.05rem; color: #64748b; margin: 0 0 32px; line-height: 1.6; }
.ix-product-cta__actions { display: flex; gap: 14px; justify-content: center; }

/* ═══ Dark Mode ═══ */
.dark .ix-product-hero__title { color: #f1f5f9; }
.dark .ix-product-hero__desc { color: #94a3b8; }
.dark .ix-product-hero__tag { color: #60a5fa; }
.dark .ix-product-btn--ghost { background: rgba(255, 255, 255, 0.06); color: #cbd5e1; border-color: rgba(255, 255, 255, 0.12); }
.dark .ix-product-btn--ghost:hover { background: rgba(255, 255, 255, 0.1); color: #f1f5f9; border-color: rgba(255, 255, 255, 0.2); }
.dark .ix-product-section__tag { color: #60a5fa; }
.dark .ix-product-section__title { color: #f3f4f6; }
.dark .ix-product-section__subtitle { color: #94a3b8; }
.dark .ix-product-section--alt { background: #0f1420; }
.dark .ix-product-card { background: #131a2b; border-color: rgba(255, 255, 255, 0.06); }
.dark .ix-product-card:hover { border-color: rgba(59, 130, 246, 0.35); box-shadow: 0 8px 30px rgba(59, 130, 246, 0.08); }
.dark .ix-product-card__icon { background: rgba(59, 130, 246, 0.1); color: #60a5fa; }
.dark .ix-product-card__title { color: #f3f4f6; }
.dark .ix-product-card__desc { color: #94a3b8; }
.dark .ix-product-card__bullets li { color: #94a3b8; }
.dark .ix-product-card__bullets li::before { color: #34d399; }
.dark .ix-product-network { background: #131a2b; border-color: rgba(255, 255, 255, 0.06); }
.dark .ix-product-network:hover { border-color: rgba(59, 130, 246, 0.35); box-shadow: 0 6px 20px rgba(59, 130, 246, 0.06); }
.dark .ix-product-network__name { color: #f3f4f6; }
.dark .ix-product-network__fee { color: #60a5fa; }
.dark .ix-product-faq__item { background: #131a2b; border-color: rgba(255, 255, 255, 0.06); }
.dark .ix-product-faq__item[open] { border-color: rgba(59, 130, 246, 0.3); }
.dark .ix-product-faq__question { color: #f3f4f6; }
.dark .ix-product-faq__question::after { color: #64748b; }
.dark .ix-product-faq__answer { color: #94a3b8; }
.dark .ix-product-cta { background: linear-gradient(135deg, rgba(59, 130, 246, 0.08) 0%, #111827 100%); border-color: rgba(255, 255, 255, 0.06); }
.dark .ix-product-cta__title { color: #f3f4f6; }
.dark .ix-product-cta__desc { color: #94a3b8; }

/* ═══ Responsive ═══ */
@media (max-width: 1023px) { .ix-product-grid--3 { grid-template-columns: repeat(2, 1fr); } .ix-product-networks { grid-template-columns: repeat(4, 1fr); } }
@media (max-width: 768px) { .ix-product-hero { padding: 80px 20px 60px; } .ix-product-hero__title { font-size: 2.2rem; } .ix-product-hero__actions { flex-direction: column; align-items: center; } .ix-product-grid--2 { grid-template-columns: 1fr; } .ix-product-grid--3 { grid-template-columns: 1fr; } .ix-product-networks { grid-template-columns: repeat(2, 1fr); } .ix-product-section__title { font-size: 1.75rem; } .ix-product-cta { padding: 40px 24px; } .ix-product-cta__title { font-size: 1.6rem; } .ix-product-cta__actions { flex-direction: column; align-items: center; } }
@media (max-width: 480px) { .ix-product-hero__title { font-size: 1.8rem; } .ix-product-networks { grid-template-columns: repeat(2, 1fr); } }

/* ═══ Reduced Motion ═══ */
@media (prefers-reduced-motion: reduce) { .ix-product-card, .ix-product-network { opacity: 1; transform: none; } .ix-product-card:hover, .ix-product-network:hover { transform: none; } }

/* ═══ VitePress overrides ═══ */
.vp-doc .ix-product-page h1, .vp-doc .ix-product-page h2, .vp-doc .ix-product-page h3 { border: none; margin-top: 0; padding-top: 0; }
</style>
