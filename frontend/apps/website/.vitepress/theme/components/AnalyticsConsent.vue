<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useData } from 'vitepress'
import {
  ANALYTICS_PREFERENCES_EVENT,
  getAnalyticsConsent,
  setAnalyticsConsent,
  type AnalyticsConsent,
} from '../analytics'

const { lang } = useData()
const isEn = computed(() => lang.value === 'en-US' || lang.value === 'en')
const visible = ref(false)
const currentConsent = ref<AnalyticsConsent | null>(null)

const copy = computed(() =>
  isEn.value
    ? {
        label: 'Analytics preferences',
        title: 'Help us improve IronixPay',
        description:
          'We use analytics cookies only on this website and its documentation to understand which content is useful. We do not track the dashboard, checkout, or payment data.',
        privacy: 'Privacy policy',
        decline: 'Decline',
        accept: 'Allow analytics',
      }
    : {
        label: '分析偏好设置',
        title: '帮助我们改进 IronixPay',
        description:
          '我们仅在官网和文档中使用分析 Cookie，了解哪些内容真正有帮助。不会追踪控制台、收银台或支付数据。',
        privacy: '隐私政策',
        decline: '拒绝',
        accept: '允许分析',
      },
)

function choose(consent: AnalyticsConsent) {
  setAnalyticsConsent(consent)
  currentConsent.value = consent
  visible.value = false
}

function openPreferences() {
  currentConsent.value = getAnalyticsConsent()
  visible.value = true
}

onMounted(() => {
  currentConsent.value = getAnalyticsConsent()
  visible.value = currentConsent.value === null
  window.addEventListener(ANALYTICS_PREFERENCES_EVENT, openPreferences)
})

onUnmounted(() => {
  window.removeEventListener(ANALYTICS_PREFERENCES_EVENT, openPreferences)
})
</script>

<template>
  <Teleport to="body">
    <Transition name="ix-consent">
      <section
        v-if="visible"
        class="ix-consent"
        role="dialog"
        aria-modal="false"
        :aria-label="copy.label"
      >
        <div class="ix-consent__copy">
          <strong>{{ copy.title }}</strong>
          <p>{{ copy.description }}</p>
          <a :href="isEn ? '/en/privacy' : '/privacy'">{{ copy.privacy }}</a>
        </div>

        <div class="ix-consent__actions">
          <button
            type="button"
            class="ix-consent__button ix-consent__button--secondary"
            :aria-pressed="currentConsent === 'denied'"
            @click="choose('denied')"
          >
            {{ copy.decline }}
          </button>
          <button
            type="button"
            class="ix-consent__button ix-consent__button--primary"
            :aria-pressed="currentConsent === 'granted'"
            @click="choose('granted')"
          >
            {{ copy.accept }}
          </button>
        </div>
      </section>
    </Transition>
  </Teleport>
</template>

<style scoped>
.ix-consent {
  box-sizing: border-box;
  position: fixed;
  z-index: 9000;
  right: 16px;
  bottom: 16px;
  left: 16px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 28px;
  width: min(920px, calc(100% - 32px));
  margin: 0 auto;
  padding: 18px 20px;
  border: 1px solid #dbe3ec;
  border-radius: 8px;
  background: #ffffff;
  box-shadow: 0 16px 40px rgba(15, 23, 42, 0.18);
  color: #172033;
  font-family: 'Exo 2', sans-serif;
}

.ix-consent__copy {
  min-width: 0;
}

.ix-consent__copy strong {
  display: block;
  margin-bottom: 4px;
  color: #111827;
  font-size: 0.95rem;
  font-weight: 700;
}

.ix-consent__copy p {
  margin: 0;
  color: #526078;
  font-size: 0.8rem;
  line-height: 1.55;
}

.ix-consent__copy a {
  display: inline-block;
  margin-top: 5px;
  color: #1d4ed8;
  font-size: 0.78rem;
  font-weight: 600;
  text-decoration: none;
}

.ix-consent__copy a:hover {
  text-decoration: underline;
}

.ix-consent__actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.ix-consent__button {
  min-height: 38px;
  padding: 0 15px;
  border: 1px solid transparent;
  border-radius: 6px;
  font: inherit;
  font-size: 0.8rem;
  font-weight: 700;
  letter-spacing: 0;
  cursor: pointer;
  transition: background-color 0.15s ease, border-color 0.15s ease, color 0.15s ease;
}

.ix-consent__button--secondary {
  border-color: #cbd5e1;
  background: #ffffff;
  color: #344054;
}

.ix-consent__button--secondary:hover {
  background: #f5f7fa;
}

.ix-consent__button--primary {
  background: #2563eb;
  color: #ffffff;
}

.ix-consent__button--primary:hover {
  background: #1d4ed8;
}

.ix-consent__button:focus-visible,
.ix-consent__copy a:focus-visible {
  outline: 3px solid rgba(37, 99, 235, 0.28);
  outline-offset: 2px;
}

:global(.dark) .ix-consent {
  border-color: #334155;
  background: #111827;
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.34);
  color: #e5e7eb;
}

:global(.dark) .ix-consent__copy strong {
  color: #f8fafc;
}

:global(.dark) .ix-consent__copy p {
  color: #a8b3c5;
}

:global(.dark) .ix-consent__copy a {
  color: #93c5fd;
}

:global(.dark) .ix-consent__button--secondary {
  border-color: #475569;
  background: #1f2937;
  color: #e2e8f0;
}

:global(.dark) .ix-consent__button--secondary:hover {
  background: #293548;
}

.ix-consent-enter-active,
.ix-consent-leave-active {
  transition: opacity 0.18s ease, transform 0.18s ease;
}

.ix-consent-enter-from,
.ix-consent-leave-to {
  opacity: 0;
  transform: translateY(10px);
}

@media (max-width: 700px) {
  .ix-consent {
    grid-template-columns: 1fr;
    gap: 14px;
    padding: 16px;
  }

  .ix-consent__actions {
    justify-content: flex-end;
  }
}

@media (max-width: 420px) {
  .ix-consent {
    right: 10px;
    bottom: 10px;
    left: 10px;
    width: calc(100% - 20px);
  }

  .ix-consent__actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }

  .ix-consent__button {
    width: 100%;
    padding: 0 10px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .ix-consent-enter-active,
  .ix-consent-leave-active {
    transition: none;
  }
}
</style>
