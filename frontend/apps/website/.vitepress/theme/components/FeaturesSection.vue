<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { useData } from 'vitepress'

const { lang } = useData()
const isZh = computed(() => lang.value === 'zh-CN')

// ─── i18n ───
const t = computed(() =>
  isZh.value
    ? {
        badge: '核心能力',
        title: '为什么选择 IronixPay',
        subtitle: '从收款到出金，为数字资产商户提供完整的支付基础设施。',
        learnMore: '了解更多 →',
      }
    : {
        badge: 'Core Features',
        title: 'Why IronixPay',
        subtitle: 'End-to-end payment infrastructure for digital asset merchants — from checkout to payout.',
        learnMore: 'Learn more →',
      },
)

const features = computed(() => {
  const prefix = isZh.value ? '' : '/en'
  return [
    {
      icon: 'checkout',
      title: isZh.value ? '一键收款' : 'Instant Checkout',
      desc: isZh.value
        ? '一次 API 调用创建收银台，客户扫码即付，链上到账自动确认。'
        : 'One API call creates a checkout. Customer scans, pays, and on-chain confirmation is automatic.',
      link: `${prefix}/checkout`,
    },
    {
      icon: 'payout',
      title: isZh.value ? 'API 出金' : 'API Payouts',
      desc: isZh.value
        ? '余额秒出至任意链上地址，完全可编程，支持幂等重试。'
        : 'Send USDT or USDC to any on-chain address programmatically with idempotent retry support.',
      link: `${prefix}/payouts`,
    },
    {
      icon: 'chain',
      title: isZh.value ? '统一接口' : 'Unified API',
      desc: isZh.value
        ? '一套 API 接入所有链，无需逐链对接。新链上线无缝生效。'
        : 'One API for all chains — no per-chain integration. New networks go live seamlessly.',
      link: `${prefix}/guide/networks`,
    },
    {
      icon: 'shield',
      title: isZh.value ? '沙盒测试' : 'Sandbox Testing',
      desc: isZh.value
        ? '使用 TRON Nile 测试网零风险验证完整支付流程，sk_test_ 密钥即开即用。'
        : 'Test the full payment flow risk-free on TRON Nile testnet. sk_test_ keys work out of the box.',
      link: `${prefix}/guide/testing`,
    },
    {
      icon: 'dashboard',
      title: isZh.value ? '商户面板' : 'Merchant Dashboard',
      desc: isZh.value
        ? '实时交易监控、Resolution Center 处理异常支付、完整 Webhook 日志。'
        : 'Real-time monitoring, resolution center for edge cases, and full webhook audit logs.',
      link: 'https://app.ironixpay.com',
      external: true,
    },
    {
      icon: 'webhook',
      title: isZh.value ? '可靠通知' : 'Reliable Webhooks',
      desc: isZh.value
        ? '签名验证 + 7 级指数退避重试，确保事件零丢失。'
        : 'Signature verification + 7-level exponential backoff — zero event loss guaranteed.',
      link: `${prefix}/guide/webhooks`,
    },
  ]
})

// ─── Scroll-triggered stagger animation ───
const sectionRef = ref<HTMLElement | null>(null)
const isVisible = ref(false)
let observer: IntersectionObserver | null = null

onMounted(() => {
  // Respect prefers-reduced-motion
  const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)')
  if (motionQuery.matches) {
    isVisible.value = true
    return
  }

  observer = new IntersectionObserver(
    ([entry]) => {
      if (entry.isIntersecting) {
        isVisible.value = true
        observer?.disconnect()
      }
    },
    { threshold: 0.15 },
  )
  if (sectionRef.value) {
    observer.observe(sectionRef.value)
  }
})

onUnmounted(() => {
  observer?.disconnect()
})
</script>

<template>
  <section ref="sectionRef" class="ix-features" :class="{ 'ix-features--visible': isVisible }">
    <div class="ix-features__container">
      <!-- Header -->
      <div class="ix-features__header">
        <span class="ix-features__badge">{{ t.badge }}</span>
        <h2 class="ix-features__title">{{ t.title }}</h2>
        <p class="ix-features__subtitle">{{ t.subtitle }}</p>
      </div>

      <!-- 3×2 Grid -->
      <div class="ix-features__grid">
        <component
          :is="f.link ? 'a' : 'div'"
          v-for="(f, i) in features"
          :key="f.icon"
          class="ix-features__card"
          :style="{ '--delay': `${i * 80}ms` }"
          :href="f.link || undefined"
          :target="f.external ? '_blank' : undefined"
          :rel="f.external ? 'noopener noreferrer' : undefined"
        >
          <!-- Icon -->
          <div class="ix-features__icon-wrap">
            <!-- Checkout -->
            <svg v-if="f.icon === 'checkout'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
              <rect x="1" y="4" width="22" height="16" rx="3" />
              <line x1="1" y1="10" x2="23" y2="10" />
              <line x1="6" y1="15" x2="10" y2="15" />
            </svg>
            <!-- Payout -->
            <svg v-else-if="f.icon === 'payout'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
              <line x1="7" y1="17" x2="17" y2="7" />
              <polyline points="7 7 17 7 17 17" />
            </svg>
            <!-- Chain / Unified -->
            <svg v-else-if="f.icon === 'chain'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
              <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
            </svg>
            <!-- Shield -->
            <svg v-else-if="f.icon === 'shield'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
              <polyline points="9 12 11 14 15 10" />
            </svg>
            <!-- Dashboard -->
            <svg v-else-if="f.icon === 'dashboard'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
              <rect x="3" y="3" width="7" height="9" rx="1.5" />
              <rect x="14" y="3" width="7" height="5" rx="1.5" />
              <rect x="14" y="12" width="7" height="9" rx="1.5" />
              <rect x="3" y="16" width="7" height="5" rx="1.5" />
            </svg>
            <!-- Webhook / Bell -->
            <svg v-else-if="f.icon === 'webhook'" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9" />
              <path d="M13.73 21a2 2 0 0 1-3.46 0" />
              <line x1="12" y1="2" x2="12" y2="4" />
            </svg>
          </div>

          <!-- Text -->
          <h3 class="ix-features__card-title">{{ f.title }}</h3>
          <p class="ix-features__card-desc">{{ f.desc }}</p>

          <!-- Learn more link indicator -->
          <span v-if="f.link" class="ix-features__card-link">
            {{ t.learnMore }}
          </span>
        </component>
      </div>
    </div>
  </section>
</template>

<style scoped>
.ix-features {
  position: relative;
  z-index: 1;
  padding: 0 24px 80px;
}

.ix-features__container {
  max-width: 1100px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  align-items: center;
}

/* ─── Header ─── */
.ix-features__header {
  text-align: center;
  margin-bottom: 48px;
  max-width: 600px;
}

.ix-features__badge {
  display: inline-block;
  font-family: 'Exo 2', sans-serif;
  font-size: 0.78rem;
  font-weight: 600;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: #3b82f6;
  margin-bottom: 16px;
}

.ix-features__title {
  font-family: 'Exo 2', sans-serif;
  font-size: 2.4rem;
  font-weight: 800;
  letter-spacing: -0.03em;
  line-height: 1.15;
  color: #0f172a;
  margin: 0 0 12px;
}

.ix-features__subtitle {
  font-size: 1.05rem;
  line-height: 1.6;
  color: #64748b;
  margin: 0;
}

/* ─── Grid ─── */
.ix-features__grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 20px;
  width: 100%;
}

/* ─── Card ─── */
.ix-features__card {
  display: flex;
  flex-direction: column;
  padding: 28px 24px;
  border-radius: 20px;
  background: #ffffff;
  border: 1px solid #e5e7eb;
  cursor: pointer;
  text-decoration: none;
  color: inherit;
  transition: transform 0.25s ease-out, border-color 0.25s ease-out, box-shadow 0.25s ease-out;

  /* Stagger animation */
  opacity: 0;
  transform: translateY(20px);
}

.ix-features--visible .ix-features__card {
  opacity: 1;
  transform: translateY(0);
  transition: opacity 0.5s ease-out var(--delay),
              transform 0.5s ease-out var(--delay),
              border-color 0.25s ease-out,
              box-shadow 0.25s ease-out;
}

.ix-features__card:hover {
  transform: translateY(-4px);
  border-color: #bfdbfe;
  box-shadow: 0 8px 30px rgba(37, 99, 235, 0.08);
}

.ix-features__card:focus-visible {
  outline: 2px solid #3b82f6;
  outline-offset: 2px;
}

/* ─── Icon ─── */
.ix-features__icon-wrap {
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

/* ─── Text ─── */
.ix-features__card-title {
  font-family: 'Exo 2', sans-serif;
  font-size: 1.15rem;
  font-weight: 700;
  color: #111827;
  margin: 0 0 8px;
  line-height: 1.3;
}

.ix-features__card-desc {
  font-size: 0.88rem;
  line-height: 1.6;
  color: #6b7280;
  margin: 0;
  flex: 1;
}

.ix-features__card-link {
  display: inline-block;
  margin-top: 14px;
  font-size: 0.82rem;
  font-weight: 600;
  color: #2563eb;
  transition: color 0.2s ease;
}

.ix-features__card:hover .ix-features__card-link {
  color: #1d4ed8;
}

/* ═══ Dark Mode ═══ */
.dark .ix-features__title {
  color: #f3f4f6;
}

.dark .ix-features__subtitle {
  color: #94a3b8;
}

.dark .ix-features__card {
  background: #131a2b;
  border-color: rgba(255, 255, 255, 0.06);
}

.dark .ix-features__card:hover {
  border-color: rgba(59, 130, 246, 0.35);
  box-shadow: 0 8px 30px rgba(59, 130, 246, 0.08);
}

.dark .ix-features__icon-wrap {
  background: rgba(59, 130, 246, 0.1);
  color: #60a5fa;
}

.dark .ix-features__card-title {
  color: #f3f4f6;
}

.dark .ix-features__card-desc {
  color: #94a3b8;
}

.dark .ix-features__card-link {
  color: #60a5fa;
}

.dark .ix-features__card:hover .ix-features__card-link {
  color: #93bbfd;
}

/* ═══ Responsive ═══ */
@media (max-width: 1023px) {
  .ix-features__grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (max-width: 639px) {
  .ix-features__grid {
    grid-template-columns: 1fr;
  }

  .ix-features__title {
    font-size: 1.75rem;
  }

  .ix-features__subtitle {
    font-size: 0.95rem;
  }

  .ix-features__card {
    padding: 24px 20px;
  }
}

/* ═══ Reduced Motion ═══ */
@media (prefers-reduced-motion: reduce) {
  .ix-features__card {
    opacity: 1;
    transform: none;
    transition: border-color 0.2s ease, box-shadow 0.2s ease;
  }

  .ix-features--visible .ix-features__card {
    transition: border-color 0.2s ease, box-shadow 0.2s ease;
  }

  .ix-features__card:hover {
    transform: none;
  }
}
</style>
