<script setup lang="ts">
import { computed } from 'vue'
import { useData } from 'vitepress'

const { lang } = useData()
const isZh = computed(() => lang.value === 'zh-CN')
const label = computed(() => (isZh.value ? '技术驱动' : 'POWERED BY'))

const partners = [
  { name: 'TronGrid', logo: '/partners/trongrid.svg' },
  { name: 'Helius', logo: '/partners/helius.svg' },
  { name: 'Cloudflare', logo: '/partners/cloudflare.svg' },
  { name: 'Alchemy', logo: '/partners/alchemy.svg' },
  { name: 'Ankr', logo: '/partners/ankr.svg' },
  { name: 'GoPlus', logo: '/partners/goplus.svg' },
]
</script>

<template>
  <section class="ix-marquee">
    <div class="ix-marquee__header">
      <div class="ix-marquee__line" />
      <p class="ix-marquee__label">{{ label }}</p>
      <div class="ix-marquee__line" />
    </div>
    <div class="ix-marquee__viewport">
      <div class="ix-marquee__track">
        <!-- 4 identical sets to ensure content always fills the viewport -->
        <template v-for="set in 4" :key="set">
          <div
            v-for="(p, i) in partners"
            :key="set + '-' + i"
            class="ix-marquee__item"
            :aria-hidden="set > 1 ? 'true' : undefined"
          >
            <img
              :src="p.logo"
              :alt="p.name"
              class="ix-marquee__icon"
              loading="lazy"
            />
            <span class="ix-marquee__name">{{ p.name }}</span>
          </div>
        </template>
      </div>
    </div>
  </section>
</template>

<style scoped>
.ix-marquee {
  overflow: hidden;
  padding: 40px 0 48px;
  position: relative;
}

/* ─── Section header ─── */
.ix-marquee__header {
  display: flex;
  align-items: center;
  gap: 16px;
  max-width: 300px;
  margin: 0 auto 32px;
  padding: 0 24px;
}

.ix-marquee__line {
  flex: 1;
  height: 1px;
  background: linear-gradient(
    90deg,
    transparent 0%,
    rgba(148, 163, 184, 0.2) 50%,
    transparent 100%
  );
}

.ix-marquee__label {
  font-family: 'Exo 2', sans-serif;
  font-size: 0.8rem;
  font-weight: 600;
  letter-spacing: 0.16em;
  text-transform: uppercase;
  color: #64748b;
  margin: 0;
  white-space: nowrap;
}

/* ─── Viewport with edge fade ─── */
.ix-marquee__viewport {
  overflow: hidden;
  mask-image: linear-gradient(
    to right,
    transparent 0%,
    black 15%,
    black 85%,
    transparent 100%
  );
  -webkit-mask-image: linear-gradient(
    to right,
    transparent 0%,
    black 15%,
    black 85%,
    transparent 100%
  );
}

/* ─── Track: single flex row, animated as one unit ─── */
/* Using margin-right instead of gap ensures N items have N spacings,
   making translateX(-50%) perfectly seamless at the loop point. */
.ix-marquee__track {
  display: flex;
  align-items: center;
  width: max-content;
  animation: marquee-scroll 25s linear infinite;
}

.ix-marquee__item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  cursor: default;
  flex-shrink: 0;
  min-width: 80px;
  margin-right: 80px;
}

.ix-marquee__icon {
  width: 48px;
  height: 48px;
  flex-shrink: 0;
  object-fit: contain;
  transition: transform 0.3s ease, filter 0.3s ease;
}

.ix-marquee__item:hover .ix-marquee__icon {
  transform: scale(1.15);
}

.ix-marquee__name {
  font-family: 'Exo 2', sans-serif;
  font-size: 0.88rem;
  font-weight: 500;
  color: #64748b;
  white-space: nowrap;
  transition: color 0.3s ease;
}

.ix-marquee__item:hover .ix-marquee__name {
  color: #1e293b;
}

/* Pause on hover */
.ix-marquee:hover .ix-marquee__track {
  animation-play-state: paused;
}

/*
  The track has 24 items (4 identical sets of 6).
  We translate exactly -25% (one full set) so it loops perfectly.
*/
@keyframes marquee-scroll {
  0% {
    transform: translateX(0);
  }
  100% {
    transform: translateX(-25%);
  }
}

/* ═══ Light mode ═══ */
:root:not(.dark) .ix-marquee__line {
  background: linear-gradient(
    90deg,
    transparent 0%,
    rgba(100, 116, 139, 0.2) 50%,
    transparent 100%
  );
}

:root:not(.dark) .ix-marquee__label {
  color: #94a3b8;
}

/* ═══ Dark Mode ═══ */
.dark .ix-marquee__name {
  color: #64748b;
}

.dark .ix-marquee__item:hover .ix-marquee__name {
  color: #e2e8f0;
}

.dark .ix-marquee__item:hover .ix-marquee__icon {
  filter: drop-shadow(0 4px 12px rgba(255, 255, 255, 0.1));
}

/* ═══ Responsive ═══ */
@media (max-width: 640px) {
  .ix-marquee__item {
    margin-right: 48px;
  }

  .ix-marquee__icon {
    width: 36px;
    height: 36px;
  }

  .ix-marquee__item {
    min-width: 60px;
  }

  .ix-marquee__name {
    font-size: 0.75rem;
  }
}

/* ═══ Reduced Motion ═══ */
@media (prefers-reduced-motion: reduce) {
  .ix-marquee__track {
    animation: none;
  }
}
</style>
