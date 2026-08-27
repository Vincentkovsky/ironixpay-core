<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { useData } from 'vitepress'

const { lang } = useData()
const isZh = computed(() => lang.value === 'zh-CN')

const stats = computed(() =>
  isZh.value
    ? [
        { value: '8', suffix: ' 条链', label: '多链覆盖' },
        { value: '0.5', suffix: '%', label: '收款费率' },
        { value: '99.9', suffix: '%', label: '系统可用性' },
        { value: '<3', suffix: ' 分钟', label: '结算时间' },
      ]
    : [
        { value: '8', suffix: ' Chains', label: 'Multi-chain support' },
        { value: '0.5', suffix: '%', label: 'Checkout fee' },
        { value: '99.9', suffix: '%', label: 'Uptime' },
        { value: '<3', suffix: ' min', label: 'Settlement time' },
      ],
)

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
    { threshold: 0.3 },
  )
  if (sectionRef.value) observer.observe(sectionRef.value)
})

onUnmounted(() => observer?.disconnect())
</script>

<template>
  <section ref="sectionRef" class="ix-stats" :class="{ 'ix-stats--visible': isVisible }">
    <div class="ix-stats__inner">
      <div
        v-for="(s, i) in stats"
        :key="i"
        class="ix-stats__item"
        :style="{ '--delay': `${i * 100}ms` }"
      >
        <div class="ix-stats__value">
          <span>{{ s.value }}</span><span class="ix-stats__suffix">{{ s.suffix }}</span>
        </div>
        <div class="ix-stats__label">{{ s.label }}</div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.ix-stats {
  position: relative;
  padding: 48px 24px;
  background: linear-gradient(180deg, #f8fafc 0%, #ffffff 100%);
  border-top: 1px solid #e5e7eb;
  border-bottom: 1px solid #e5e7eb;
}

.ix-stats__inner {
  max-width: 960px;
  margin: 0 auto;
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 0;
}

.ix-stats__item {
  text-align: center;
  padding: 16px 12px;
  position: relative;
  opacity: 0;
  transform: translateY(12px);
}

.ix-stats__item + .ix-stats__item::before {
  content: '';
  position: absolute;
  left: 0;
  top: 50%;
  transform: translateY(-50%);
  width: 1px;
  height: 48px;
  background: linear-gradient(180deg, transparent, #cbd5e1, transparent);
}

.ix-stats--visible .ix-stats__item {
  opacity: 1;
  transform: translateY(0);
  transition: opacity 0.6s ease-out var(--delay), transform 0.6s ease-out var(--delay);
}

.ix-stats__value {
  font-family: 'Exo 2', sans-serif;
  font-size: 2.8rem;
  font-weight: 800;
  letter-spacing: -0.04em;
  line-height: 1;
  color: #0f172a;
  margin-bottom: 8px;
}

.ix-stats__suffix {
  font-size: 1.6rem;
  font-weight: 700;
  color: #2563eb;
}

.ix-stats__label {
  font-family: 'Inter', sans-serif;
  font-size: 0.82rem;
  font-weight: 500;
  color: #64748b;
  letter-spacing: 0.02em;
  text-transform: uppercase;
}

/* ═══ Dark Mode ═══ */
.dark .ix-stats {
  background: linear-gradient(180deg, #0a0e17 0%, #0f1420 100%);
  border-color: rgba(255, 255, 255, 0.06);
}

.dark .ix-stats__item::before {
  background: linear-gradient(180deg, transparent, rgba(255, 255, 255, 0.1), transparent);
}

.dark .ix-stats__value {
  color: #f1f5f9;
}

.dark .ix-stats__suffix {
  color: #60a5fa;
}

.dark .ix-stats__label {
  color: #94a3b8;
}

/* ═══ Responsive ═══ */
@media (max-width: 640px) {
  .ix-stats__inner {
    grid-template-columns: repeat(2, 1fr);
    gap: 24px 0;
  }

  .ix-stats__item:nth-child(3)::before {
    display: none;
  }

  .ix-stats__value {
    font-size: 2.2rem;
  }

  .ix-stats__suffix {
    font-size: 1.3rem;
  }
}

/* ═══ Reduced Motion ═══ */
@media (prefers-reduced-motion: reduce) {
  .ix-stats__item {
    opacity: 1;
    transform: none;
  }
}
</style>
