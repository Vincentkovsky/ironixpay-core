<script setup lang="ts">
import { ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import axios from 'axios'
import { AUTH_BASE_URL } from '@/utils/request'
import { LOCALE_OPTIONS } from '@/locale'
import { Globe } from 'lucide-vue-next'
import TurnstileWidget from '@/components/TurnstileWidget.vue'

const { t, locale } = useI18n()
const router = useRouter()
const email = ref('')
const status = ref<'form' | 'loading' | 'success' | 'error'>('form')
const message = ref('')
const turnstileToken = ref('')
const turnstileWidget = ref<InstanceType<typeof TurnstileWidget> | null>(null)

const API_BASE = AUTH_BASE_URL
const turnstileSiteKey = (import.meta.env.VITE_TURNSTILE_SITE_KEY as string | undefined) || ''

const currentLocaleName = computed(() =>
  LOCALE_OPTIONS.find((o) => o.value === locale.value)?.label || 'English',
)

const toggleLocale = () => {
  const next = locale.value === 'en-US' ? 'zh-CN' : 'en-US'
  locale.value = next
  localStorage.setItem('app-locale', next)
}

async function handleSubmit() {
  if (!email.value || !email.value.includes('@')) {
    message.value = t('forgotPassword.invalidEmail')
    return
  }

  if (turnstileSiteKey && !turnstileToken.value) {
    message.value = t('forgotPassword.humanVerificationFailed')
    return
  }

  status.value = 'loading'
  message.value = ''

  try {
    await axios.post(`${API_BASE}/api/auth/forgot-password`, {
      email: email.value,
      turnstile_token: turnstileToken.value || undefined,
    })
    status.value = 'success'
    message.value = t('forgotPassword.successDefault')
  } catch (err: any) {
    turnstileToken.value = ''
    turnstileWidget.value?.reset()
    status.value = 'error'
    const errorCode = err?.response?.data?.error?.code
    if (errorCode === 'human_verification_failed') {
      message.value = t('forgotPassword.humanVerificationFailed')
    } else if (errorCode === 'service_unavailable') {
      message.value = t('forgotPassword.verificationUnavailable')
    } else {
      message.value = t('forgotPassword.errorDefault')
    }
  }
}

function handleTurnstileVerified(token: string) {
  turnstileToken.value = token
  message.value = ''
}

function handleTurnstileExpired() {
  turnstileToken.value = ''
}

function handleTurnstileError() {
  turnstileToken.value = ''
  message.value = t('forgotPassword.verificationUnavailable')
}

function tryAgain() {
  turnstileToken.value = ''
  message.value = ''
  status.value = 'form'
}

function goToLogin() {
  router.push('/login')
}
</script>

<template>
  <div class="forgot-container">
    <div class="forgot-card">
      <!-- Language switcher -->
      <button class="lang-switch" @click="toggleLocale">
        <Globe :size="16" />
        {{ currentLocaleName }}
      </button>

      <!-- Logo -->
      <div class="logo">
        <span class="logo-ironix">Ironix</span><span class="logo-pay">Pay</span>
      </div>

      <!-- Success -->
      <template v-if="status === 'success'">
        <div class="icon-wrap success">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10" />
            <path d="M8 12l3 3 5-5" />
          </svg>
        </div>
        <h2>{{ t('forgotPassword.successTitle') }}</h2>
        <p class="subtitle">{{ message }}</p>
        <button class="btn-primary" @click="goToLogin">{{ t('login.backToLogin') }}</button>
      </template>

      <!-- Error -->
      <template v-else-if="status === 'error'">
        <div class="icon-wrap error">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10" />
            <path d="M15 9l-6 6M9 9l6 6" />
          </svg>
        </div>
        <h2>{{ t('forgotPassword.errorTitle') }}</h2>
        <p class="subtitle">{{ message }}</p>
        <button class="btn-secondary" @click="tryAgain">{{ t('forgotPassword.tryAgain') }}</button>
      </template>

      <!-- Form -->
      <template v-else>
        <h2>{{ t('forgotPassword.title') }}</h2>
        <p class="subtitle">{{ t('forgotPassword.subtitle') }}</p>

        <form @submit.prevent="handleSubmit">
          <div class="field">
            <label>{{ t('forgotPassword.emailLabel') }}</label>
            <input
              v-model="email"
              type="email"
              :placeholder="t('forgotPassword.emailPlaceholder')"
              :disabled="status === 'loading'"
              autofocus
            />
          </div>

          <p v-if="message" class="error-msg">{{ message }}</p>

          <div v-if="turnstileSiteKey" class="turnstile-wrap">
            <TurnstileWidget
              ref="turnstileWidget"
              :site-key="turnstileSiteKey"
              action="forgot_password"
              @verified="handleTurnstileVerified"
              @expired="handleTurnstileExpired"
              @error="handleTurnstileError"
            />
          </div>

          <button
            type="submit"
            class="btn-primary"
            :disabled="status === 'loading' || (!!turnstileSiteKey && !turnstileToken)"
          >
            {{ status === 'loading' ? t('forgotPassword.sending') : t('forgotPassword.submit') }}
          </button>
        </form>

        <button class="btn-link" @click="goToLogin">
          {{ t('forgotPassword.backToLogin') }}
        </button>
      </template>
    </div>
  </div>
</template>

<style scoped>
.forgot-container {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #f5f5f5;
  padding: 20px;
}

.forgot-card {
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

.turnstile-wrap {
  display: flex;
  justify-content: center;
  min-height: 65px;
  margin-bottom: 20px;
}

.field {
  margin-bottom: 20px;
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

.btn-secondary {
  display: block;
  width: 100%;
  background: #f3f4f6;
  color: #374151;
  font-size: 15px;
  font-weight: 600;
  padding: 12px;
  border: none;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.2s;
}
.btn-secondary:hover { background: #e5e7eb; }

.btn-link {
  display: block;
  width: 100%;
  margin-top: 16px;
  background: none;
  border: none;
  color: #6b7280;
  font-size: 14px;
  cursor: pointer;
  transition: color 0.2s;
}
.btn-link:hover { color: #374151; }
</style>
