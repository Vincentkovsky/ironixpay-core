<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import axios from 'axios'
import { AUTH_BASE_URL } from '@/utils/request'
import { getErrorCode } from '@/utils/error-utils'
import { LOCALE_OPTIONS } from '@/locale'
import { Globe } from 'lucide-vue-next'

const { t, locale } = useI18n()
const route = useRoute()
const router = useRouter()

const status = ref<'loading' | 'success' | 'error'>('loading')
const message = ref('')

const API_BASE = AUTH_BASE_URL

const currentLocaleName = computed(() =>
  LOCALE_OPTIONS.find((o) => o.value === locale.value)?.label || 'English',
)

const toggleLocale = () => {
  const next = locale.value === 'en-US' ? 'zh-CN' : 'en-US'
  locale.value = next
  localStorage.setItem('app-locale', next)
}

onMounted(async () => {
  const token = route.query.token as string
  if (!token) {
    status.value = 'error'
    message.value = t('verifyEmail.missingToken')
    return
  }

  try {
    const res = await axios.post(`${API_BASE}/api/auth/verify-email`, { token })
    status.value = 'success'
    message.value = t('verifyEmail.successDefault')
  } catch (err: any) {
    const code = getErrorCode(err)
    if (code === 'already_verified') {
      status.value = 'success'
      message.value = t('verifyEmail.alreadyVerified')
    } else if (code === 'token_expired') {
      status.value = 'error'
      message.value = t('verifyEmail.expired')
    } else {
      status.value = 'error'
      message.value = t('verifyEmail.errorDefault')
    }
  }
})

function goToLogin() {
  router.push('/login')
}
</script>

<template>
  <div class="verify-container">
    <div class="verify-card">
      <!-- Language switcher -->
      <button class="lang-switch" @click="toggleLocale">
        <Globe :size="16" />
        {{ currentLocaleName }}
      </button>

      <!-- Logo -->
      <div class="logo">
        <span class="logo-ironix">Ironix</span><span class="logo-pay">Pay</span>
      </div>

      <!-- Loading -->
      <template v-if="status === 'loading'">
        <div class="icon-wrap">
          <svg class="spinner" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10" stroke-opacity="0.25" />
            <path d="M12 2a10 10 0 0 1 10 10" stroke-linecap="round" />
          </svg>
        </div>
        <h2>{{ t('verifyEmail.loadingTitle') }}</h2>
        <p class="subtitle">{{ t('verifyEmail.loadingSubtitle') }}</p>
      </template>

      <!-- Success -->
      <template v-else-if="status === 'success'">
        <div class="icon-wrap success">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10" />
            <path d="M8 12l3 3 5-5" />
          </svg>
        </div>
        <h2>{{ t('verifyEmail.successTitle') }}</h2>
        <p class="subtitle">{{ message }}</p>
        <button class="btn-primary" @click="goToLogin">{{ t('verifyEmail.goToLogin') }}</button>
      </template>

      <!-- Error -->
      <template v-else-if="status === 'error'">
        <div class="icon-wrap error">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10" />
            <path d="M15 9l-6 6M9 9l6 6" />
          </svg>
        </div>
        <h2>{{ t('verifyEmail.errorTitle') }}</h2>
        <p class="subtitle">{{ message }}</p>
        <button class="btn-primary" @click="goToLogin">{{ t('verifyEmail.goToLogin') }}</button>
      </template>
    </div>
  </div>
</template>

<style scoped>
.verify-container {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #f5f5f5;
  padding: 20px;
}

.verify-card {
  background: white;
  border-radius: 16px;
  padding: 48px 40px;
  max-width: 440px;
  width: 100%;
  text-align: center;
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.08);
  position: relative;
}

.lang-switch {
  position: absolute;
  top: 16px;
  right: 16px;
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: #6b7280;
  background: none;
  border: none;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 6px;
  transition: color 0.2s, background 0.2s;
}
.lang-switch:hover { color: #374151; background: #f3f4f6; }

.logo {
  font-size: 28px;
  font-weight: 700;
  margin-bottom: 32px;
}
.logo-ironix { color: #1d2939; }
.logo-pay { color: #2563eb; }

.icon-wrap {
  width: 64px;
  height: 64px;
  margin: 0 auto 20px;
  color: #6b7280;
}
.icon-wrap svg {
  width: 64px;
  height: 64px;
}
.icon-wrap.success { color: #22c55e; }
.icon-wrap.error { color: #ef4444; }

.spinner {
  animation: spin 1s linear infinite;
}
@keyframes spin {
  to { transform: rotate(360deg); }
}

h2 {
  font-size: 22px;
  font-weight: 600;
  color: #1d2939;
  margin: 0 0 8px;
}

.subtitle {
  font-size: 15px;
  color: #6b7280;
  margin: 0 0 28px;
  line-height: 1.5;
}

.btn-primary {
  display: inline-block;
  background: #2563eb;
  color: white;
  font-size: 15px;
  font-weight: 600;
  padding: 12px 32px;
  border: none;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.2s;
}
.btn-primary:hover {
  background: #1d4ed8;
}
</style>
