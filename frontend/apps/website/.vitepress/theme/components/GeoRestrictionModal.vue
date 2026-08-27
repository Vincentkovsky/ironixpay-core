<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useData } from 'vitepress'

const { lang } = useData()
const isEn = computed(() => lang.value === 'en-US' || lang.value === 'en')

const visible = ref(false)
const STORAGE_KEY = 'ironixpay-geo-dismissed'
const RESTRICTED_COUNTRIES = ['CN', 'US']

async function detectCountry(): Promise<string | null> {
  try {
    // Cloudflare's free endpoint — no API key, no quota, works on any CF-proxied domain
    const res = await fetch('/cdn-cgi/trace', { cache: 'no-store' })
    if (res.ok) {
      const text = await res.text()
      const match = text.match(/loc=(\w+)/)
      if (match) return match[1]
    }
    // Not on Cloudflare or unexpected response — try fallback
    throw new Error('cf trace unavailable')
  } catch {
    // Fallback: ip-api (free, no key, 45 req/min)
    try {
      const res = await fetch('https://ip-api.com/json/?fields=countryCode')
      if (!res.ok) return null
      const data = await res.json()
      return data.countryCode || null
    } catch {
      return null
    }
  }
}

function dismiss() {
  visible.value = false
  try {
    sessionStorage.setItem(STORAGE_KEY, '1')
  } catch { /* private browsing */ }
}

onMounted(async () => {
  // Don't show again in this session
  try {
    if (sessionStorage.getItem(STORAGE_KEY)) return
  } catch { /* private browsing */ }

  const country = await detectCountry()
  if (country && RESTRICTED_COUNTRIES.includes(country)) {
    visible.value = true
  }
})
</script>

<template>
  <Teleport to="body">
    <Transition name="geo-modal">
      <div v-if="visible" class="geo-overlay" @click.self="dismiss">
        <div class="geo-modal" role="alertdialog" aria-modal="true">
          <!-- Amber accent top bar -->
          <div class="geo-modal__accent" />

          <!-- Icon + Title row -->
          <div class="geo-modal__header">
            <div class="geo-modal__icon-wrap">
              <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
                <path d="M12 8v4" />
                <circle cx="12" cy="16" r="0.5" fill="currentColor" />
              </svg>
            </div>
            <h2 class="geo-modal__title">
              {{ isEn ? 'Important Notice' : '重要提示' }}
            </h2>
          </div>

          <div class="geo-modal__divider" />

          <div class="geo-modal__body">
            <p v-if="!isEn">
              为遵守适用的法律法规，IronixPay 不向您的司法辖区提供服务。本网站仅供一般信息用途。它不构成也不应被解释为在您的司法辖区的任何形式的服务招揽或要约。
            </p>
            <p>
              In compliance with applicable laws and regulations, IronixPay does not provide services in your jurisdiction. This website is for general informational purposes only. It does not constitute, and should not be construed as, any form of solicitation or offer of services in your jurisdiction.
            </p>
          </div>

          <button class="geo-modal__btn" @click="dismiss">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="20 6 9 17 4 12" />
            </svg>
            {{ isEn ? 'I Understand' : '本人已了解 (I Understand)' }}
          </button>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
/* ═══ Overlay ═══ */
.geo-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(7, 10, 20, 0.72);
  backdrop-filter: blur(12px) saturate(1.4);
  -webkit-backdrop-filter: blur(12px) saturate(1.4);
  padding: 24px;
}

/* ═══ Modal card ═══ */
.geo-modal {
  position: relative;
  overflow: hidden;
  max-width: 480px;
  width: 100%;
  border-radius: 20px;
  padding: 0 32px 32px;
  background: linear-gradient(168deg, #ffffff 0%, #f8fafc 100%);
  border: 1px solid rgba(0, 0, 0, 0.06);
  box-shadow:
    0 32px 64px -16px rgba(0, 0, 0, 0.18),
    0 0 0 1px rgba(0, 0, 0, 0.04),
    0 0 80px -20px rgba(245, 158, 11, 0.08);
}

:root.dark .geo-modal {
  background: linear-gradient(168deg, #141b2d 0%, #0f172a 100%);
  border: 1px solid rgba(255, 255, 255, 0.06);
  box-shadow:
    0 32px 64px -16px rgba(0, 0, 0, 0.5),
    0 0 0 1px rgba(255, 255, 255, 0.04),
    0 0 80px -20px rgba(245, 158, 11, 0.06);
}

/* ═══ Amber accent bar ═══ */
.geo-modal__accent {
  height: 3px;
  margin: 0 -32px 28px;
  background: linear-gradient(90deg, #f59e0b, #fbbf24 40%, transparent 100%);
  opacity: 0.9;
}

:root.dark .geo-modal__accent {
  opacity: 0.7;
}

/* ═══ Header ═══ */
.geo-modal__header {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-bottom: 20px;
}

.geo-modal__icon-wrap {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 42px;
  height: 42px;
  border-radius: 12px;
  background: rgba(245, 158, 11, 0.1);
  color: #d97706;
}

:root.dark .geo-modal__icon-wrap {
  background: rgba(245, 158, 11, 0.12);
  color: #fbbf24;
}

.geo-modal__title {
  font-family: 'Exo 2', sans-serif;
  font-size: 1.15rem;
  font-weight: 700;
  letter-spacing: -0.01em;
  color: #0f172a;
  margin: 0;
}

:root.dark .geo-modal__title {
  color: #f1f5f9;
}

/* ═══ Divider ═══ */
.geo-modal__divider {
  height: 1px;
  margin: 0 0 20px;
  background: rgba(0, 0, 0, 0.06);
}

:root.dark .geo-modal__divider {
  background: rgba(255, 255, 255, 0.06);
}

/* ═══ Body ═══ */
.geo-modal__body {
  margin-bottom: 24px;
}

.geo-modal__body p {
  font-size: 0.84rem;
  line-height: 1.75;
  color: #475569;
  margin: 0 0 10px;
}

.geo-modal__body p:last-child {
  margin-bottom: 0;
}

:root.dark .geo-modal__body p {
  color: #94a3b8;
}

/* ═══ Button ═══ */
.geo-modal__btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  width: 100%;
  padding: 13px 24px;
  border: none;
  border-radius: 12px;
  background: linear-gradient(135deg, #0f172a 0%, #1e293b 100%);
  color: #e2e8f0;
  font-family: 'Exo 2', sans-serif;
  font-size: 0.88rem;
  font-weight: 600;
  letter-spacing: 0.01em;
  cursor: pointer;
  transition: all 0.2s ease;
}

.geo-modal__btn:hover {
  background: linear-gradient(135deg, #1e293b 0%, #334155 100%);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

.geo-modal__btn:active {
  transform: translateY(1px);
}

:root.dark .geo-modal__btn {
  background: linear-gradient(135deg, #d97706 0%, #f59e0b 100%);
  color: #0f172a;
}

:root.dark .geo-modal__btn:hover {
  background: linear-gradient(135deg, #f59e0b 0%, #fbbf24 100%);
  box-shadow: 0 4px 20px rgba(245, 158, 11, 0.25);
}

/* ═══ Transitions ═══ */
.geo-modal-enter-active {
  transition: opacity 0.35s cubic-bezier(0.16, 1, 0.3, 1);
}

.geo-modal-enter-active .geo-modal {
  transition: transform 0.35s cubic-bezier(0.16, 1, 0.3, 1), opacity 0.35s cubic-bezier(0.16, 1, 0.3, 1);
}

.geo-modal-leave-active {
  transition: opacity 0.2s ease;
}

.geo-modal-leave-active .geo-modal {
  transition: transform 0.2s ease, opacity 0.2s ease;
}

.geo-modal-enter-from {
  opacity: 0;
}

.geo-modal-enter-from .geo-modal {
  opacity: 0;
  transform: scale(0.96) translateY(12px);
}

.geo-modal-leave-to {
  opacity: 0;
}

.geo-modal-leave-to .geo-modal {
  opacity: 0;
  transform: scale(0.96) translateY(8px);
}

/* ═══ Responsive ═══ */
@media (max-width: 540px) {
  .geo-modal {
    padding: 0 24px 24px;
    border-radius: 16px;
  }

  .geo-modal__accent {
    margin: 0 -24px 24px;
  }

  .geo-modal__header {
    gap: 12px;
  }

  .geo-modal__icon-wrap {
    width: 38px;
    height: 38px;
  }

  .geo-modal__title {
    font-size: 1.05rem;
  }

  .geo-modal__body p {
    font-size: 0.8rem;
  }
}

/* ═══ Reduce motion ═══ */
@media (prefers-reduced-motion: reduce) {
  .geo-modal-enter-active,
  .geo-modal-enter-active .geo-modal,
  .geo-modal-leave-active,
  .geo-modal-leave-active .geo-modal {
    transition: none;
  }
}
</style>
