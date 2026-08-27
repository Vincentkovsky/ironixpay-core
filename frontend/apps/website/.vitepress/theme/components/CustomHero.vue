<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { useData } from 'vitepress'

const { lang } = useData()
const isZh = computed(() => lang.value === 'zh-CN')

// --- Payment Terminal Animation ---
const steps = [
  { id: 'amount', duration: 1800 },
  { id: 'address', duration: 2200 },
  { id: 'confirming', duration: 2000 },
  { id: 'paid', duration: 2500 },
] as const

type StepId = (typeof steps)[number]['id']
const currentStep = ref<StepId>('amount')
const showTerminal = ref(false)
let timer: ReturnType<typeof setTimeout> | null = null
let stepIndex = 0

function runStep() {
  const step = steps[stepIndex]
  currentStep.value = step.id
  timer = setTimeout(() => {
    stepIndex = (stepIndex + 1) % steps.length
    runStep()
  }, step.duration)
}

onMounted(() => {
  setTimeout(() => {
    showTerminal.value = true
    runStep()
  }, 600)
})

onUnmounted(() => {
  if (timer) clearTimeout(timer)
})

// i18n strings
const t = computed(() =>
  isZh.value
    ? {
        tagline: '稳定币支付网关',
        headline: '为你的业务接入\nUSDT / USDC 收款',
        desc: '一次 API 调用创建收银台。链上支付自动确认，资金准实时结算至商户余额。',
        ctaPrimary: '快速开始',
        ctaSecondary: 'API 文档',
        termTitle: '支付终端',
        amount: '金额',
        address: '收款地址',
        status: '状态',
        generating: '生成地址中...',
        confirming: '链上确认中...',
        paid: '✓ 已到账',
        network: '网络',
        trustItems: [
          { value: '8 链', label: '多链支持' },
          { value: '0.5%', label: '交易手续费' },
          { value: '自动归集', label: '资金准实时清算' },
          { value: '内置 AML', label: 'OFAC + GoPlus 筛查' },
        ],
      }
    : {
        tagline: 'Stablecoin Payment Gateway',
        headline: 'Accept USDT / USDC\nfor Your Business',
        desc: 'One API call to create a checkout. On-chain payments auto-confirmed, funds settled to your balance in minutes.',
        ctaPrimary: 'Get Started',
        ctaSecondary: 'API Docs',
        termTitle: 'Payment Terminal',
        amount: 'Amount',
        address: 'Receive Address',
        status: 'Status',
        generating: 'Generating address...',
        confirming: 'Confirming on-chain...',
        paid: '✓ Payment Received',
        network: 'Network',
        trustItems: [
          { value: '8 Chains', label: 'Multi-chain support' },
          { value: '0.5%', label: 'Transaction fee' },
          { value: 'Auto-Sweep', label: 'Near-instant settlement' },
          { value: 'Built-in AML', label: 'OFAC + GoPlus screening' },
        ],
      },
)

const ctaPrimaryLink = computed(() => (isZh.value ? '/guide/quickstart' : '/en/guide/quickstart'))
const ctaSecondaryLink = 'https://api.ironixpay.com/docs'
</script>

<template>
  <section class="ix-hero">
    <!-- Ambient background effects -->
    <div class="ix-hero__bg">
      <div class="ix-hero__glow ix-hero__glow--1" />
    </div>

    <div class="ix-hero__container">
      <!-- Left: Slogan -->
      <div class="ix-hero__left">
        <h1 class="ix-hero__title">
          <span class="ix-hero__brand">IronixPay</span>
          <span class="ix-hero__headline" v-html="t.headline.replace(/\n/g, '<br />')" />
        </h1>

        <p class="ix-hero__desc">{{ t.desc }}</p>

        <div class="ix-hero__actions">
          <a
            :href="ctaPrimaryLink"
            class="ix-hero__btn ix-hero__btn--primary"
            data-analytics-event="cta_click"
            data-analytics-name="quickstart"
            data-analytics-location="hero"
          >
            {{ t.ctaPrimary }}
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <path d="M3 8h10m0 0L9 4m4 4L9 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </a>
          <a
            :href="ctaSecondaryLink"
            target="_blank"
            class="ix-hero__btn ix-hero__btn--ghost"
            data-analytics-event="cta_click"
            data-analytics-name="api_docs"
            data-analytics-location="hero"
          >
            {{ t.ctaSecondary }}
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <path d="M4 10L10 4m0 0H5m5 0v5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </a>
        </div>

        <!-- Trust Indicators -->
        <div class="ix-hero__trust">
          <div v-for="(item, i) in t.trustItems" :key="i" class="ix-hero__trust-item">
            <span class="ix-hero__trust-value">{{ item.value }}</span>
            <span class="ix-hero__trust-label">{{ item.label }}</span>
          </div>
        </div>
      </div>

      <!-- Right: Payment Terminal Demo -->
      <div class="ix-hero__right">
        <Transition name="terminal-enter">
          <div v-if="showTerminal" class="ix-terminal">
            <!-- Terminal header -->
            <div class="ix-terminal__header">
              <div class="ix-terminal__dots">
                <span /><span /><span />
              </div>
              <span class="ix-terminal__title">{{ t.termTitle }}</span>
              <div class="ix-terminal__status">
                <span class="ix-terminal__live-dot" />
                LIVE
              </div>
            </div>

            <!-- Terminal body -->
            <div class="ix-terminal__body">
              <!-- Amount row -->
              <div class="ix-terminal__row" :class="{ 'ix-terminal__row--active': currentStep === 'amount' }">
                <span class="ix-terminal__label">{{ t.amount }}</span>
                <div class="ix-terminal__value ix-terminal__value--amount">
                  <span class="ix-terminal__currency">$</span>
                  <span class="ix-terminal__number">
                    <Transition name="digit-roll" mode="out-in">
                      <span key="amount">29.99</span>
                    </Transition>
                  </span>
                  <span class="ix-terminal__unit">USDT</span>
                </div>
              </div>

              <!-- Network row -->
              <div class="ix-terminal__row ix-terminal__row--network">
                <span class="ix-terminal__label">{{ t.network }}</span>
                <div class="ix-terminal__value">
                  <span class="ix-terminal__network-badge">
                    <img src="/networks/tron.svg" alt="TRON" width="16" height="16" />
                    TRON
                  </span>
                </div>
              </div>

              <!-- Address row -->
              <div class="ix-terminal__row" :class="{ 'ix-terminal__row--active': currentStep === 'address' }">
                <span class="ix-terminal__label">{{ t.address }}</span>
                <div class="ix-terminal__value">
                  <Transition name="fade-slide" mode="out-in">
                    <span v-if="currentStep === 'amount'" key="generating" class="ix-terminal__generating">
                      <span class="ix-terminal__spinner" />
                      {{ t.generating }}
                    </span>
                    <span v-else key="address" class="ix-terminal__address">
                      TLa2f6...x9Qp
                    </span>
                  </Transition>
                </div>
              </div>

              <!-- Divider -->
              <div class="ix-terminal__divider" />

              <!-- Status row -->
              <div class="ix-terminal__row ix-terminal__row--status" :class="{ 'ix-terminal__row--active': currentStep === 'confirming' || currentStep === 'paid' }">
                <span class="ix-terminal__label">{{ t.status }}</span>
                <div class="ix-terminal__value">
                  <Transition name="fade-slide" mode="out-in">
                    <span v-if="currentStep === 'amount' || currentStep === 'address'" key="waiting" class="ix-terminal__status-badge ix-terminal__status-badge--waiting">
                      ⏳ Waiting
                    </span>
                    <span v-else-if="currentStep === 'confirming'" key="confirming" class="ix-terminal__status-badge ix-terminal__status-badge--confirming">
                      <span class="ix-terminal__spinner" />
                      {{ t.confirming }}
                    </span>
                    <span v-else key="paid" class="ix-terminal__status-badge ix-terminal__status-badge--paid">
                      {{ t.paid }}
                    </span>
                  </Transition>
                </div>
              </div>
            </div>

            <!-- Terminal footer -->
            <div class="ix-terminal__footer">
              <span>Powered by <strong>IronixPay</strong></span>
            </div>
          </div>
        </Transition>
      </div>
    </div>
  </section>
</template>
