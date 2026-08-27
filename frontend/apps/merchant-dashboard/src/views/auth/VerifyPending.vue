<template>
  <div class="flex min-h-screen">
    <!-- Left panel: gradient brand (same as login) -->
    <div
      class="hidden lg:flex w-[480px] flex-col justify-between p-10 relative overflow-hidden"
      :class="'bg-gradient-to-br from-zinc-900 via-indigo-950 to-zinc-900'"
    >
      <div class="absolute inset-0 opacity-[0.04]" style="background-image: radial-gradient(circle, rgba(255,255,255,0.3) 1px, transparent 1px); background-size: 24px 24px;" />
      <div class="absolute -top-20 -left-20 h-64 w-64 rounded-full bg-blue-500/10 blur-3xl" />
      <div class="absolute -bottom-32 -right-20 h-80 w-80 rounded-full bg-indigo-500/10 blur-3xl" />

      <div class="relative z-10">
        <img
          src="/brand/logo-wordmark-white.svg"
          alt="IronixPay"
          class="h-7 opacity-90"
        />
      </div>
      <div class="relative z-10 space-y-3 text-white/80">
        <p class="text-2xl font-semibold text-white">
          {{ t('login.brandHeadline') }}
        </p>
        <p class="text-sm leading-relaxed">
          {{ t('login.brandDescription') }}
        </p>
      </div>
      <p class="relative z-10 text-xs text-white/40">© {{ new Date().getFullYear() }} IronixPay</p>
    </div>

    <!-- Right panel: verification pending -->
    <div class="flex flex-1 items-center justify-center p-8 relative">
      <!-- Language switcher -->
      <button
        class="absolute top-6 right-6 flex items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground transition-colors px-2.5 py-1.5 rounded-md hover:bg-accent"
        @click="toggleLocale"
      >
        <Globe class="h-4 w-4" />
        {{ currentLocaleName }}
      </button>
      <div class="w-full max-w-md space-y-6 animate-fade-in-up text-center">
        <!-- Mobile logo -->
        <img
          src="/brand/logo-wordmark.svg"
          alt="IronixPay"
          class="h-6 lg:hidden mb-4 mx-auto"
        />

        <!-- Mail icon -->
        <div class="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-blue-50">
          <MailCheck class="h-8 w-8 text-blue-600" />
        </div>

        <div class="space-y-2">
          <h1 class="text-2xl font-bold tracking-tight">{{ t('verifyPending.title') }}</h1>
          <p class="text-sm text-muted-foreground leading-relaxed">
            {{ t('verifyPending.description') }}
          </p>
          <p class="text-sm font-medium text-foreground">
            {{ email }}
          </p>
        </div>

        <!-- Success message -->
        <div
          v-if="successMessage"
          class="rounded-md bg-green-50 border border-green-200 p-3 text-sm text-green-700"
        >
          {{ successMessage }}
        </div>

        <!-- Error message -->
        <div
          v-if="errorMessage"
          class="rounded-md bg-destructive/10 border border-destructive/20 p-3 text-sm text-destructive"
        >
          {{ errorMessage }}
        </div>

        <!-- Check instructions -->
        <div class="rounded-lg border border-border bg-muted/30 p-4 space-y-2 text-left">
          <p class="text-sm text-muted-foreground">
            {{ t('verifyPending.instructions') }}
          </p>
          <ul class="text-sm text-muted-foreground space-y-1 list-disc list-inside">
            <li>{{ t('verifyPending.checkInbox') }}</li>
            <li>{{ t('verifyPending.checkSpam') }}</li>
          </ul>
        </div>

        <!-- Resend button -->
        <Button
          variant="outline"
          class="w-full"
          :disabled="resendDisabled"
          @click="handleResend"
        >
          <Loader2 v-if="loading" class="mr-2 h-4 w-4 animate-spin" />
          <MailPlus v-else class="mr-2 h-4 w-4" />
          {{ resendDisabled && countdown > 0
            ? t('verifyPending.resendCountdown', { seconds: countdown })
            : t('verifyPending.resend')
          }}
        </Button>

        <!-- Back to login -->
        <router-link
          to="/login"
          class="inline-flex items-center text-sm text-muted-foreground hover:text-foreground underline-offset-4 hover:underline transition-colors"
        >
          <ArrowLeft class="mr-1 h-4 w-4" />
          {{ t('verifyPending.backToLogin') }}
        </router-link>
      </div>
    </div>
  </div>
</template>

<script lang="ts" setup>
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useRoute } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { MailCheck, MailPlus, ArrowLeft, Loader2, Globe } from 'lucide-vue-next';
import { Button } from '@/components/ui/button';
import { AUTH_BASE_URL } from '@/utils/request';
import { LOCALE_OPTIONS } from '@/locale';
import axios from 'axios';

const { t, locale } = useI18n();
const route = useRoute();

const currentLocaleName = computed(() =>
  LOCALE_OPTIONS.find((o) => o.value === locale.value)?.label || 'English',
);

const toggleLocale = () => {
  const next = locale.value === 'en-US' ? 'zh-CN' : 'en-US';
  locale.value = next;
  localStorage.setItem('app-locale', next);
};

const email = computed(() => (route.query.email as string) || '');
const loading = ref(false);
const errorMessage = ref('');
const successMessage = ref('');
const countdown = ref(0);

let timer: ReturnType<typeof setInterval> | null = null;

const resendDisabled = computed(() => loading.value || countdown.value > 0);

const startCountdown = (seconds = 60) => {
  countdown.value = seconds;
  if (timer) clearInterval(timer);
  timer = setInterval(() => {
    countdown.value--;
    if (countdown.value <= 0 && timer) {
      clearInterval(timer);
      timer = null;
    }
  }, 1000);
};

const handleResend = async () => {
  if (resendDisabled.value || !email.value) return;
  loading.value = true;
  errorMessage.value = '';
  successMessage.value = '';

  try {
    await axios.post(`${AUTH_BASE_URL}/api/auth/resend-verification`, {
      email: email.value,
    });
    successMessage.value = t('verifyPending.resendSuccess');
    startCountdown(60);
  } catch (err: any) {
    errorMessage.value = t('verifyPending.resendError');
  } finally {
    loading.value = false;
  }
};


onMounted(() => {
  // Start with a 5-second countdown to prevent immediate resend
  startCountdown(5);
});

onUnmounted(() => {
  if (timer) clearInterval(timer);
});
</script>
