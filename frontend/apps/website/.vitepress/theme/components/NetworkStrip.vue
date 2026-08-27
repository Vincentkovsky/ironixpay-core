<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useData } from 'vitepress'

const { lang } = useData()
const isZh = computed(() => lang.value === 'zh-CN')
const label = computed(() => (isZh.value ? '支持的网络' : 'Supported Networks'))
const visible = ref(false)

onMounted(() => {
  setTimeout(() => {
    visible.value = true
  }, 1200)
})

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
</script>

<template>
  <section class="ix-networks" :class="{ 'ix-networks--visible': visible }">
    <div class="ix-networks__container">
      <div class="ix-networks__header">
        <div class="ix-networks__line" />
        <p class="ix-networks__label">{{ label }}</p>
        <div class="ix-networks__line" />
      </div>
      <div class="ix-networks__row">
        <div
          v-for="(net, i) in networks"
          :key="net.name"
          class="ix-networks__item"
          :style="{ '--delay': `${i * 80}ms` }"
        >
          <img
            :src="net.icon"
            :alt="net.name"
            class="ix-networks__icon"
            width="44"
            height="44"
            loading="eager"
          />
          <span class="ix-networks__name">{{ net.name }}</span>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.ix-networks {
  position: relative;
  z-index: 1;
  padding: 0 24px 64px;
  opacity: 0;
  transform: translateY(16px);
  transition: opacity 0.7s ease, transform 0.7s ease;
}

.ix-networks--visible {
  opacity: 1;
  transform: translateY(0);
}

.ix-networks__container {
  max-width: 800px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 28px;
}

/* ─── Header with horizontal lines ─── */
.ix-networks__header {
  display: flex;
  align-items: center;
  gap: 16px;
  width: 100%;
  max-width: 400px;
}

.ix-networks__line {
  flex: 1;
  height: 1px;
  background: linear-gradient(
    90deg,
    transparent 0%,
    rgba(148, 163, 184, 0.2) 50%,
    transparent 100%
  );
}

.ix-networks__label {
  font-family: 'Exo 2', sans-serif;
  font-size: 0.8rem;
  font-weight: 600;
  letter-spacing: 0.16em;
  text-transform: uppercase;
  color: #64748b;
  margin: 0;
  white-space: nowrap;
}

/* ─── Logo row ─── */
.ix-networks__row {
  display: flex;
  align-items: flex-start;
  justify-content: center;
  gap: 40px;
  flex-wrap: wrap;
}

.ix-networks__item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  cursor: default;
  opacity: 0;
  transform: translateY(8px);
  transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
}

.ix-networks--visible .ix-networks__item {
  opacity: 1;
  transform: translateY(0);
  transition-delay: var(--delay);
}

.ix-networks__item:hover {
  transform: translateY(-3px);
}

.ix-networks__icon {
  width: 44px;
  height: 44px;
  flex-shrink: 0;
  transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1),
              filter 0.3s ease;
}

.ix-networks__item:hover .ix-networks__icon {
  transform: scale(1.15);
  filter: drop-shadow(0 4px 12px rgba(255, 255, 255, 0.1));
}

.ix-networks__name {
  font-family: 'Exo 2', sans-serif;
  font-size: 0.82rem;
  font-weight: 500;
  color: #64748b;
  transition: color 0.3s ease;
  white-space: nowrap;
}

.ix-networks__item:hover .ix-networks__name {
  color: #e2e8f0;
}

/* ─── Light mode ─── */
:root:not(.dark) .ix-networks__line {
  background: linear-gradient(
    90deg,
    transparent 0%,
    rgba(100, 116, 139, 0.2) 50%,
    transparent 100%
  );
}

:root:not(.dark) .ix-networks__label {
  color: #94a3b8;
}

:root:not(.dark) .ix-networks__name {
  color: #94a3b8;
}

:root:not(.dark) .ix-networks__item:hover .ix-networks__name {
  color: #1e293b;
}

:root:not(.dark) .ix-networks__item:hover .ix-networks__icon {
  filter: drop-shadow(0 4px 12px rgba(0, 0, 0, 0.1));
}

/* ─── Responsive ─── */
@media (max-width: 640px) {
  .ix-networks__row {
    gap: 24px;
  }

  .ix-networks__icon {
    width: 32px;
    height: 32px;
  }

  .ix-networks__name {
    font-size: 0.72rem;
  }
}

/* ─── Reduced motion ─── */
@media (prefers-reduced-motion: reduce) {
  .ix-networks,
  .ix-networks__item {
    transition: none;
    opacity: 1;
    transform: none;
  }
}
</style>
