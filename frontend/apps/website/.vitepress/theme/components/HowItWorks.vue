<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { useData } from 'vitepress'

const { lang } = useData()
const isZh = computed(() => lang.value === 'zh-CN')

const t = computed(() =>
  isZh.value
    ? {
        badge: '工作原理',
        title: '4 步完成链上收款',
        subtitle: '从创建收银台到资金入账，全流程自动化。',
        steps: [
          {
            num: '01',
            title: '创建 Checkout',
            desc: '一次 API 调用自动派生独立收款地址，生成付款页面。',
          },
          {
            num: '02',
            title: '客户支付',
            desc: '客户选择链和币种，扫码或转账 USDT/USDC 到指定地址。',
          },
          {
            num: '03',
            title: '链上确认',
            desc: 'Indexer 实时监听链上事件，自动检测并确认到账。',
          },
          {
            num: '04',
            title: '资金归集',
            desc: 'Sweeper 自动将资金归集至您的 treasury 钱包，随时可提现。',
          },
        ],
      }
    : {
        badge: 'How it works',
        title: 'Accept crypto in 4 steps',
        subtitle: 'From checkout creation to fund settlement — fully automated.',
        steps: [
          {
            num: '01',
            title: 'Create Checkout',
            desc: 'One API call derives a unique deposit address and generates a payment page.',
          },
          {
            num: '02',
            title: 'Customer Pays',
            desc: 'Customer picks their preferred chain and sends USDT or USDC to the address.',
          },
          {
            num: '03',
            title: 'Auto Confirmation',
            desc: 'Our indexer monitors on-chain events and confirms the payment in real-time.',
          },
          {
            num: '04',
            title: 'Funds Settled',
            desc: 'Sweeper auto-collects funds to your treasury wallet. Withdraw anytime.',
          },
        ],
      },
)

// Scroll-triggered animation
const sectionRef = ref<HTMLElement | null>(null)
const isVisible = ref(false)
let observer: IntersectionObserver | null = null

onMounted(() => {
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
  if (sectionRef.value) observer.observe(sectionRef.value)
})

onUnmounted(() => observer?.disconnect())
</script>

<template>
  <section ref="sectionRef" class="ix-how" :class="{ 'ix-how--visible': isVisible }">
    <div class="ix-how__container">
      <div class="ix-how__header">
        <span class="ix-how__badge">{{ t.badge }}</span>
        <h2 class="ix-how__title">{{ t.title }}</h2>
        <p class="ix-how__subtitle">{{ t.subtitle }}</p>
      </div>

      <div class="ix-how__grid">
        <div
          v-for="(step, i) in t.steps"
          :key="step.num"
          class="ix-how__step"
          :style="{ '--delay': `${i * 120}ms` }"
        >
          <div class="ix-how__num">{{ step.num }}</div>

          <!-- Connector line (except last) -->
          <div v-if="i < t.steps.length - 1" class="ix-how__connector">
            <svg width="100%" height="2" preserveAspectRatio="none">
              <line x1="0" y1="1" x2="100%" y2="1" stroke="currentColor" stroke-width="2" stroke-dasharray="6 4" />
            </svg>
          </div>

          <h3 class="ix-how__step-title">{{ step.title }}</h3>
          <p class="ix-how__step-desc">{{ step.desc }}</p>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.ix-how {
  position: relative;
  padding: 80px 24px;
  color: #334155;
}

.ix-how__container {
  max-width: 1100px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  align-items: center;
}

/* ─── Header ─── */
.ix-how__header {
  text-align: center;
  margin-bottom: 56px;
  max-width: 560px;
}

.ix-how__badge {
  display: inline-block;
  font-family: 'Exo 2', sans-serif;
  font-size: 0.75rem;
  font-weight: 600;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: #2563eb;
  margin-bottom: 14px;
}

.ix-how__title {
  font-family: 'Exo 2', sans-serif;
  font-size: 2.2rem;
  font-weight: 800;
  letter-spacing: -0.03em;
  line-height: 1.15;
  color: #0f172a;
  margin: 0 0 12px;
}

.ix-how__subtitle {
  font-size: 1rem;
  color: #64748b;
  margin: 0;
  line-height: 1.6;
}

/* ─── Grid ─── */
.ix-how__grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 0;
  width: 100%;
}

/* ─── Step Card ─── */
.ix-how__step {
  position: relative;
  padding: 0 20px;
  text-align: center;

  /* Animation */
  opacity: 0;
  transform: translateY(16px);
}

.ix-how--visible .ix-how__step {
  opacity: 1;
  transform: translateY(0);
  transition:
    opacity 0.5s ease-out var(--delay),
    transform 0.5s ease-out var(--delay);
}

.ix-how__num {
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

/* ─── Connector ─── */
.ix-how__connector {
  position: absolute;
  top: 24px;
  left: calc(50% + 32px);
  right: calc(-50% + 32px);
  color: rgba(37, 99, 235, 0.2);
  pointer-events: none;
}

.ix-how__step-title {
  font-family: 'Exo 2', sans-serif;
  font-size: 1.05rem;
  font-weight: 700;
  color: #0f172a;
  margin: 0 0 8px;
  padding: 0;
  border: none;
}

.ix-how__step-desc {
  font-size: 0.85rem;
  color: #64748b;
  line-height: 1.6;
  margin: 0;
}

/* ═══ Dark Mode ═══ */
.dark .ix-how {
  background: var(--vp-c-bg);
  color: #e2e8f0;
}

.dark .ix-how__badge {
  color: #60a5fa;
}

.dark .ix-how__title {
  color: #f1f5f9;
}

.dark .ix-how__subtitle {
  color: #94a3b8;
}

.dark .ix-how__num {
  background: linear-gradient(135deg, rgba(59, 130, 246, 0.15), rgba(99, 102, 241, 0.1));
  border-color: rgba(59, 130, 246, 0.25);
  color: #60a5fa;
}

.dark .ix-how__connector {
  color: rgba(59, 130, 246, 0.25);
}

.dark .ix-how__step-title {
  color: #f1f5f9;
}

.dark .ix-how__step-desc {
  color: #94a3b8;
}

/* ═══ Responsive ═══ */
@media (max-width: 768px) {
  .ix-how__grid {
    grid-template-columns: repeat(2, 1fr);
    gap: 32px 0;
  }

  .ix-how__connector {
    display: none;
  }

  .ix-how__title {
    font-size: 1.7rem;
  }
}

@media (max-width: 480px) {
  .ix-how__grid {
    grid-template-columns: 1fr;
    gap: 28px;
  }
}

/* ═══ Reduced Motion ═══ */
@media (prefers-reduced-motion: reduce) {
  .ix-how__step {
    opacity: 1;
    transform: none;
  }
}

/* ═══ VitePress overrides ═══ */
.vp-doc .ix-how h2,
.vp-doc .ix-how h3 {
  border: none;
  margin-top: 0;
  padding-top: 0;
}
</style>
