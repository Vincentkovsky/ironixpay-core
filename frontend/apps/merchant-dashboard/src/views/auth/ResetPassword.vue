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

const token = ref('')
const newPassword = ref('')
const confirmPassword = ref('')
const status = ref<'form' | 'loading' | 'success' | 'error'>('form')
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

onMounted(() => {
  token.value = (route.query.token as string) || ''
  if (!token.value) {
    status.value = 'error'
    message.value = t('resetPassword.missingToken')
  }
})

async function handleSubmit() {
  if (newPassword.value.length < 8) {
    message.value = t('resetPassword.passwordTooShort')
    return
  }
  if (newPassword.value !== confirmPassword.value) {
    message.value = t('resetPassword.passwordMismatch')
    return
  }

  status.value = 'loading'
  message.value = ''

  try {
    await axios.post(`${API_BASE}/api/auth/reset-password`, {
      token: token.value,
      new_password: newPassword.value,
    })
    status.value = 'success'
    message.value = t('resetPassword.successDefault')
  } catch (err: any) {
    status.value = 'form'
    const code = getErrorCode(err)
    if (code === 'token_expired') {
      message.value = t('resetPassword.expired')
    } else if (code === 'weak_password') {
      // Show the specific backend message (e.g. password requirement details)
      message.value = err.response?.data?.error?.message || t('resetPassword.errorDefault')
    } else {
      message.value = t('resetPassword.errorDefault')
    }
  }
}

function goToLogin() {
  router.push('/login')
}
</script>

<template>
  <div class="reset-container">
    <div class="reset-card">
      <!-- Language switcher -->
      <button class="lang-switch" @click="toggleLocale">
        <Globe :size="16" />
        {{ currentLocaleName }}
      </button>

      <!-- Logo -->
      <div class="logo">
        <span class="logo-ironix">Ironix</span><span class="logo-pay">Pay</span>
      </div>

      <!-- Error: invalid token -->
      <template v-if="status === 'error'">
        <div class="icon-wrap error">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10" />
            <path d="M15 9l-6 6M9 9l6 6" />
          </svg>
        </div>
        <h2>{{ t('resetPassword.errorTitle') }}</h2>
        <p class="subtitle">{{ message }}</p>
        <button class="btn-primary" @click="goToLogin">{{ t('resetPassword.goToLogin') }}</button>
      </template>

      <!-- Success -->
      <template v-else-if="status === 'success'">
        <div class="icon-wrap success">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10" />
            <path d="M8 12l3 3 5-5" />
          </svg>
        </div>
        <h2>{{ t('resetPassword.successTitle') }}</h2>
        <p class="subtitle">{{ message }}</p>
        <button class="btn-primary" @click="goToLogin">{{ t('resetPassword.goToLogin') }}</button>
      </template>

      <!-- Form -->
      <template v-else>
        <h2>{{ t('resetPassword.title') }}</h2>
        <p class="subtitle">{{ t('resetPassword.subtitle') }}</p>

        <form @submit.prevent="handleSubmit">
          <div class="field">
            <label>{{ t('resetPassword.newPassword') }}</label>
            <input
              v-model="newPassword"
              type="password"
              :placeholder="t('resetPassword.newPasswordPlaceholder')"
              :disabled="status === 'loading'"
            />
          </div>
          <div class="field">
            <label>{{ t('resetPassword.confirmPassword') }}</label>
            <input
              v-model="confirmPassword"
              type="password"
              :placeholder="t('resetPassword.confirmPlaceholder')"
              :disabled="status === 'loading'"
            />
          </div>

          <p v-if="message" class="error-msg">{{ message }}</p>

          <button type="submit" class="btn-primary" :disabled="status === 'loading'">
            {{ status === 'loading' ? t('resetPassword.submitting') : t('resetPassword.submit') }}
          </button>
        </form>
      </template>
    </div>
  </div>
</template>

<style scoped>
.reset-container {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #f5f5f5;
  padding: 20px;
}

.reset-card {
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
}
.icon-wrap svg { width: 64px; height: 64px; }
.icon-wrap.success { color: #22c55e; }
.icon-wrap.error { color: #ef4444; }

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

form { text-align: left; }

.field {
  margin-bottom: 16px;
}
.field label {
  display: block;
  font-size: 14px;
  font-weight: 500;
  color: #374151;
  margin-bottom: 6px;
}
.field input {
  width: 100%;
  padding: 10px 14px;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  font-size: 15px;
  outline: none;
  transition: border-color 0.2s;
  box-sizing: border-box;
}
.field input:focus {
  border-color: #2563eb;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.1);
}

.error-msg {
  color: #ef4444;
  font-size: 14px;
  margin: 0 0 16px;
}

.btn-primary {
  display: block;
  width: 100%;
  background: #2563eb;
  color: white;
  font-size: 15px;
  font-weight: 600;
  padding: 12px;
  border: none;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.2s;
}
.btn-primary:hover { background: #1d4ed8; }
.btn-primary:disabled { opacity: 0.6; cursor: not-allowed; }
</style>
