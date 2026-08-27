<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue'

const container = ref<HTMLElement | null>(null)
const isVisible = ref(false)

let observer: IntersectionObserver | null = null

onMounted(() => {
  if (!container.value) return

  // Apply stagger delays dynamically (no nth-child cap)
  const steps = container.value.querySelectorAll('[data-flow-step]')
  steps.forEach((el, i) => {
    ;(el as HTMLElement).style.transitionDelay = `${0.05 + i * 0.07}s`
  })

  observer = new IntersectionObserver(
    ([entry]) => {
      if (entry.isIntersecting) {
        isVisible.value = true
        observer?.disconnect()
      }
    },
    { threshold: 0.15 }
  )
  observer.observe(container.value)
})

onUnmounted(() => {
  observer?.disconnect()
})
</script>

<template>
  <div
    ref="container"
    class="flow-chart"
    :class="{ 'flow-chart--visible': isVisible }"
  >
    <slot />
  </div>
</template>

<style scoped>
.flow-chart {
  display: flex;
  flex-direction: column;
  gap: 0;
  max-width: 560px;
  margin: 28px auto;
  position: relative;
}

/* ─── Connector between steps ─── */
.flow-chart :deep([data-flow-step]) {
  position: relative;
}

.flow-chart :deep([data-flow-step] + [data-flow-step]) {
  margin-top: 0;
}

/* Connector line */
.flow-chart :deep([data-flow-step] + [data-flow-step])::before {
  content: '';
  display: block;
  width: 2px;
  height: 32px;
  margin: 0 auto;
  background: linear-gradient(
    180deg,
    rgba(37, 99, 235, 0.4) 0%,
    rgba(6, 182, 212, 0.4) 100%
  );
  border-radius: 1px;
}

/* Pulsing dot on connector — positioned relative to the step element */
.flow-chart :deep([data-flow-step] + [data-flow-step])::after {
  content: '';
  position: absolute;
  top: 12px;
  left: 50%;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: linear-gradient(135deg, #2563eb, #06b6d4);
  box-shadow: 0 0 12px rgba(37, 99, 235, 0.5);
  transform: translateX(-50%);
  animation: flow-pulse 2s ease-in-out infinite;
  pointer-events: none;
}

@keyframes flow-pulse {
  0%, 100% { opacity: 0.5; transform: translateX(-50%) scale(0.8); }
  50% { opacity: 1; transform: translateX(-50%) scale(1.2); }
}

/* ─── Staggered entry animation ─── */
/* Delay is set dynamically in onMounted, no nth-child cap */
.flow-chart :deep([data-flow-step]) {
  opacity: 0;
  transform: translateY(20px);
  transition: opacity 0.5s cubic-bezier(0.4, 0, 0.2, 1),
              transform 0.5s cubic-bezier(0.4, 0, 0.2, 1);
}

.flow-chart--visible :deep([data-flow-step]) {
  opacity: 1;
  transform: translateY(0);
}

/* Reduced motion */
@media (prefers-reduced-motion: reduce) {
  .flow-chart :deep([data-flow-step]) {
    opacity: 1;
    transform: none;
    transition: none;
  }

  .flow-chart :deep([data-flow-step] + [data-flow-step])::after {
    animation: none;
    opacity: 1;
  }
}

/* ─── Dark mode connector ─── */
:global(.dark) .flow-chart :deep([data-flow-step] + [data-flow-step])::before {
  background: linear-gradient(
    180deg,
    rgba(59, 130, 246, 0.3) 0%,
    rgba(6, 182, 212, 0.3) 100%
  );
}
</style>
