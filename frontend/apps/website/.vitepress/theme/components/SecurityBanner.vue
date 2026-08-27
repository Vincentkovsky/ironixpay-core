<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { useData } from 'vitepress'

const { lang } = useData()
const isZh = computed(() => lang.value === 'zh-CN')
const prefix = computed(() => (isZh.value ? '' : '/en'))

const t = computed(() =>
  isZh.value
    ? {
        badge: '安全与合规',
        title: '为金融级安全标准而构建',
        cta: '了解更多',
        items: [
          { icon: 'shield', title: '反洗钱审查', desc: '每笔交易的地址自动经过双重风控筛查，命中风险立即冻结待审。' },
          { icon: 'key', title: '数据全程加密', desc: '所有密钥和敏感信息均加密存储，运行时屏蔽日志输出，杜绝泄露风险。' },
          { icon: 'wallet', title: '地址隔离', desc: '每笔支付使用独立的收款地址，资金自动归集，不在中间地址留存余额。' },
          { icon: 'signature', title: '资金自主可控', desc: '商户余额随时可提现至自有钱包，平台专注提供支付通道与风控能力。' },
        ],
      }
    : {
        badge: 'Security & Compliance',
        title: 'Built for financial-grade security',
        cta: 'Learn more',
        items: [
          { icon: 'shield', title: 'Anti-money laundering', desc: 'Every transaction address is automatically screened through dual-layer risk checks. Flagged payments are frozen for review.' },
          { icon: 'key', title: 'End-to-end encryption', desc: 'All keys and sensitive data are encrypted at rest. Runtime values are masked from logs — zero exposure risk.' },
          { icon: 'wallet', title: 'Address isolation', desc: 'Each payment uses a unique deposit address. Funds auto-sweep after confirmation — no balance left behind.' },
          { icon: 'signature', title: 'Your funds, your control', desc: 'Withdraw your balance to your own wallet anytime. We provide payment rails and risk controls for every transfer.' },
        ],
      },
)

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
  <section ref="sectionRef" class="ix-sec" :class="{ 'ix-sec--visible': isVisible }">
    <div class="ix-sec__inner">
      <div class="ix-sec__header">
        <span class="ix-sec__badge">{{ t.badge }}</span>
        <h2 class="ix-sec__title">{{ t.title }}</h2>
      </div>

      <div class="ix-sec__grid">
        <div
          v-for="(item, i) in t.items"
          :key="i"
          class="ix-sec__card"
          :style="{ '--delay': `${i * 80}ms` }"
        >
          <div class="ix-sec__icon">
            <svg v-if="item.icon === 'shield'" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><polyline points="9 12 11 14 15 10"/></svg>
            <svg v-else-if="item.icon === 'key'" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="m21 2-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.78 7.78 5.5 5.5 0 0 1 7.78-7.78Zm0 0L15.5 7.5m0 0 3 3L22 7l-3-3m-3.5 3.5L19 4"/></svg>
            <svg v-else-if="item.icon === 'wallet'" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12V7H5a2 2 0 0 1 0-4h14v4"/><path d="M3 5v14a2 2 0 0 0 2 2h16v-5"/><path d="M18 12a2 2 0 0 0 0 4h4v-4Z"/></svg>
            <svg v-else-if="item.icon === 'signature'" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2 17a5 5 0 0 0 10 0c0-2.76-2.5-5-5-3l-1.5 1.5"/><path d="M14 3v4a1 1 0 0 0 1 1h4"/><path d="M14 3 6 3a2 2 0 0 0-2 2v1"/><path d="M22 21V8l-6-5H6"/><path d="M10 12h4"/></svg>
          </div>
          <h3 class="ix-sec__card-title">{{ item.title }}</h3>
          <p class="ix-sec__card-desc">{{ item.desc }}</p>
        </div>
      </div>

      <div class="ix-sec__footer">
        <a :href="`${prefix}/trust`" class="ix-sec__cta">
          {{ t.cta }}
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none"><path d="M3 8h10m0 0L9 4m4 4L9 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
        </a>
      </div>
    </div>
  </section>
</template>

<style scoped>
.ix-sec {
  padding: 80px 24px;
}

.ix-sec__inner {
  max-width: 1100px;
  margin: 0 auto;
}

.ix-sec__header {
  text-align: center;
  margin-bottom: 48px;
}

.ix-sec__badge {
  display: inline-block;
  font-family: 'Exo 2', sans-serif;
  font-size: 0.75rem;
  font-weight: 600;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: #2563eb;
  margin-bottom: 14px;
}

.ix-sec__title {
  font-family: 'Exo 2', sans-serif;
  font-size: 2.2rem;
  font-weight: 800;
  letter-spacing: -0.03em;
  line-height: 1.15;
  color: #0f172a;
  margin: 0;
}

.ix-sec__grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 20px;
}

.ix-sec__card {
  padding: 28px 22px;
  border-radius: 16px;
  background: #ffffff;
  border: 1px solid #e5e7eb;
  opacity: 0;
  transform: translateY(16px);
  transition: transform 0.25s ease-out, border-color 0.25s ease-out, box-shadow 0.25s ease-out;
}

.ix-sec--visible .ix-sec__card {
  opacity: 1;
  transform: translateY(0);
  transition: opacity 0.5s ease-out var(--delay), transform 0.5s ease-out var(--delay), border-color 0.25s ease, box-shadow 0.25s ease;
}

.ix-sec__card:hover {
  transform: translateY(-4px);
  border-color: #bfdbfe;
  box-shadow: 0 8px 30px rgba(37, 99, 235, 0.08);
}

.ix-sec__icon {
  width: 40px;
  height: 40px;
  border-radius: 10px;
  background: #eff6ff;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #2563eb;
  margin-bottom: 16px;
}

.ix-sec__card-title {
  font-family: 'Exo 2', sans-serif;
  font-size: 1rem;
  font-weight: 700;
  color: #111827;
  margin: 0 0 8px;
  line-height: 1.3;
}

.ix-sec__card-desc {
  font-size: 0.84rem;
  line-height: 1.65;
  color: #6b7280;
  margin: 0;
}

.ix-sec__footer {
  text-align: center;
  margin-top: 36px;
}

.ix-sec__cta {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 0.88rem;
  font-weight: 600;
  color: #2563eb;
  text-decoration: none;
  transition: gap 0.2s ease;
}

.ix-sec__cta:hover {
  gap: 10px;
}

/* ═══ Dark Mode ═══ */
.dark .ix-sec__badge { color: #60a5fa; }
.dark .ix-sec__title { color: #f3f4f6; }

.dark .ix-sec__card {
  background: #131a2b;
  border-color: rgba(255, 255, 255, 0.06);
}
.dark .ix-sec__card:hover {
  border-color: rgba(59, 130, 246, 0.35);
  box-shadow: 0 8px 30px rgba(59, 130, 246, 0.08);
}

.dark .ix-sec__icon {
  background: rgba(59, 130, 246, 0.1);
  color: #60a5fa;
}
.dark .ix-sec__card-title { color: #f3f4f6; }
.dark .ix-sec__card-desc { color: #94a3b8; }
.dark .ix-sec__cta { color: #60a5fa; }

/* ═══ Responsive ═══ */
@media (max-width: 900px) {
  .ix-sec__grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (max-width: 540px) {
  .ix-sec__grid {
    grid-template-columns: 1fr;
  }

  .ix-sec__title {
    font-size: 1.7rem;
  }
}

/* ═══ Reduced Motion ═══ */
@media (prefers-reduced-motion: reduce) {
  .ix-sec__card {
    opacity: 1;
    transform: none;
  }
}

/* ═══ VitePress overrides ═══ */
.vp-doc .ix-sec h2,
.vp-doc .ix-sec h3 {
  border: none;
  margin-top: 0;
  padding-top: 0;
}
</style>
