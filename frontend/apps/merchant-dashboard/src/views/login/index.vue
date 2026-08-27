<template>
  <div class="flex min-h-screen">
    <!-- Left panel: gradient brand -->
    <div
      class="hidden lg:flex w-[480px] flex-col justify-between p-10 relative overflow-hidden"
      :class="'bg-gradient-to-br from-zinc-900 via-indigo-950 to-zinc-900'"
    >
      <!-- Subtle grid pattern -->
      <div class="absolute inset-0 opacity-[0.04]" style="background-image: radial-gradient(circle, rgba(255,255,255,0.3) 1px, transparent 1px); background-size: 24px 24px;" />

      <!-- Subtle accent orbs -->
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

    <!-- Right panel: form -->
    <div class="flex flex-1 items-center justify-center p-8 relative">
      <!-- Language switcher -->
      <button
        class="absolute top-6 right-6 flex items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground transition-colors px-2.5 py-1.5 rounded-md hover:bg-accent"
        @click="toggleLocale"
      >
        <Globe class="h-4 w-4" />
        {{ currentLocaleName }}
      </button>
      <div class="w-full max-w-sm space-y-6 animate-fade-in-up">
        <!-- Mobile logo -->
        <img
          src="/brand/logo-wordmark.svg"
          alt="IronixPay"
          class="h-6 lg:hidden mb-4"
        />

        <div>
          <h1 class="text-2xl font-bold tracking-tight">{{ title }}</h1>
          <p class="text-sm text-muted-foreground mt-1">
            {{ subtitle }}
          </p>
        </div>

        <!-- Error message -->
        <div
          v-if="errorMessage"
          class="rounded-md bg-destructive/10 border border-destructive/20 p-3 text-sm text-destructive"
        >
          {{ errorMessage }}
        </div>

        <!-- 2FA Form -->
        <form v-if="is2FA" class="space-y-4" @submit.prevent="handle2FASubmit">
          <div class="space-y-2">
            <Label for="code">{{ t('login.2faCode') }}</Label>
            <Input
              id="code"
              v-model="twoFAInfo.code"
              placeholder="000000"
              maxlength="6"
              class="font-mono text-center text-lg tracking-[0.3em]"
              autofocus
            />
          </div>
          <Button type="submit" class="w-full" :disabled="loading">
            <Loader2 v-if="loading" class="mr-2 h-4 w-4 animate-spin" />
            {{ t('login.verify') }}
          </Button>
          <Button
            type="button"
            variant="ghost"
            class="w-full"
            @click="cancel2FA"
          >
            {{ t('login.backToLogin') }}
          </Button>
        </form>

        <!-- Login / Register Forms -->
        <form v-else class="space-y-4" @submit.prevent="handleSubmit">
          <!-- Name (register only) -->
          <div v-if="!isLoginMode" class="space-y-2">
            <Label for="name">{{ t('login.fullName') }}</Label>
            <Input
              id="name"
              v-model="userInfo.name"
            />
          </div>

          <div class="space-y-2">
            <Label for="email">{{ t('login.email') }}</Label>
            <Input
              id="email"
              v-model="userInfo.email"
              type="email"
              autofocus
            />
          </div>

          <div class="space-y-2">
            <div class="flex items-center justify-between">
              <Label for="password">{{ t('login.password') }}</Label>
              <router-link
                v-if="isLoginMode"
                to="/forgot-password"
                class="text-xs text-muted-foreground hover:text-foreground underline-offset-4 hover:underline transition-colors"
              >
                {{ t('login.forgotPassword', 'Forgot password?') }}
              </router-link>
            </div>
            <Input
              id="password"
              v-model="userInfo.password"
              type="password"
              autocomplete="current-password"
            />
          </div>

          <!-- Referral Code (register only, collapsible) -->
          <div v-if="!isLoginMode" class="space-y-2">
            <button
              v-if="!showReferralInput"
              type="button"
              class="text-sm text-muted-foreground hover:text-foreground underline-offset-4 hover:underline transition-colors cursor-pointer"
              @click="showReferralInput = true"
            >
              {{ t('login.haveReferralCode') }}
            </button>
            <div v-if="showReferralInput" class="space-y-2 animate-fade-in-up">
              <Label for="referral-code">{{ t('login.referralCode') }}</Label>
              <Input
                id="referral-code"
                v-model="referralCode"
                placeholder=""
                class="font-mono uppercase tracking-wider"
              />
            </div>
          </div>

          <div v-if="isLoginMode" class="flex items-center space-x-2">
            <Checkbox
              id="remember"
              :model-value="loginConfig.rememberPassword"
              @update:model-value="(value) => (loginConfig.rememberPassword = value === true)"
            />
            <label
              for="remember"
              class="text-sm text-muted-foreground cursor-pointer select-none"
            >
              {{ t('login.rememberMe') }}
            </label>
          </div>

          <!-- Terms of Service agreement (register only) -->
          <div v-if="!isLoginMode" class="flex items-start space-x-2">
            <Checkbox
              id="agree-terms"
              :model-value="agreedToTerms"
              @update:model-value="(value) => (agreedToTerms = value === true)"
              class="mt-0.5"
            />
            <label
              for="agree-terms"
              class="text-sm text-muted-foreground cursor-pointer select-none leading-snug"
              v-html="t('login.agreeToTerms')"
            />
          </div>

          <TurnstileWidget
            v-if="!isLoginMode && turnstileSiteKey"
            ref="turnstileWidget"
            :site-key="turnstileSiteKey"
            action="register"
            @verified="handleTurnstileVerified"
            @expired="handleTurnstileExpired"
            @error="handleTurnstileError"
          />

          <Button
            type="submit"
            class="w-full"
            :disabled="loading || (!isLoginMode && (!agreedToTerms || !turnstileToken))"
          >
            <Loader2 v-if="loading" class="mr-2 h-4 w-4 animate-spin" />
            {{ isLoginMode ? t('login.signIn') : t('login.createAccount') }}
          </Button>

          <div class="text-center">
            <button
              type="button"
              class="text-sm text-muted-foreground hover:text-foreground underline-offset-4 hover:underline transition-colors cursor-pointer"
              @click="toggleLoginMode"
            >
              {{ isLoginMode ? t('login.noAccount') : t('login.hasAccount') }}
            </button>
          </div>
        </form>
      </div>
    </div>
  </div>
</template>

<script lang="ts" setup>
import { ref, reactive, computed, onMounted } from 'vue';
import { useRouter, useRoute } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { useStorage } from '@vueuse/core';
import { toast } from 'vue-sonner';
import { Loader2, Globe } from 'lucide-vue-next';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Checkbox } from '@/components/ui/checkbox';
import TurnstileWidget from '@/components/TurnstileWidget.vue';
import { useUserStore } from '@/stores';
import useLoading from '@/hooks/loading';
import { LOCALE_OPTIONS } from '@/locale';
import type { LoginRequest, RegisterRequest } from '@ironix-pay/api-client';

const { t, locale } = useI18n();

const currentLocaleName = computed(() =>
  LOCALE_OPTIONS.find((o) => o.value === locale.value)?.label || 'English',
);

const toggleLocale = () => {
  const next = locale.value === 'en-US' ? 'zh-CN' : 'en-US';
  locale.value = next;
  localStorage.setItem('app-locale', next);
};
const router = useRouter();
const route = useRoute();
const errorMessage = ref('');
const { loading, setLoading } = useLoading();
const userStore = useUserStore();
const isLoginMode = ref(true);
const is2FA = ref(false);
const twoFATempToken = ref('');
const agreedToTerms = ref(false);
const inviteToken = ref<string | null>(null);
const referralCode = ref('');
const showReferralInput = ref(false);
const turnstileSiteKey = (import.meta.env.VITE_TURNSTILE_SITE_KEY as string | undefined) || '';
const turnstileToken = ref('');
const turnstileWidget = ref<InstanceType<typeof TurnstileWidget> | null>(null);

// Auto-switch to register mode if coming from invite or referral flow
onMounted(() => {
  if (route.query.mode === 'register') {
    isLoginMode.value = false;
  }
  if (route.query.token) {
    inviteToken.value = route.query.token as string;
  }
  if (route.query.ref) {
    referralCode.value = (route.query.ref as string).toUpperCase();
    showReferralInput.value = true;
    isLoginMode.value = false;
  }
});

const loginConfig = useStorage('login-config', {
  rememberPassword: true,
  email: '',
});

const userInfo = reactive({
  name: '',
  email: loginConfig.value.email,
  password: '',
  collection_address: null as string | null,
});

const twoFAInfo = reactive({ code: '' });

const title = computed(() => {
  if (is2FA.value) return t('login.2faTitle');
  return isLoginMode.value ? t('login.title') : t('login.registerTitle');
});

const subtitle = computed(() => {
  if (is2FA.value) return t('login.2faSubtitle');
  return isLoginMode.value ? t('login.subtitle') : t('login.registerSubtitle');
});

const toggleLoginMode = () => {
  isLoginMode.value = !isLoginMode.value;
  errorMessage.value = '';
  agreedToTerms.value = false;
  referralCode.value = '';
  showReferralInput.value = false;
  turnstileToken.value = '';
};

const cancel2FA = () => {
  is2FA.value = false;
  twoFATempToken.value = '';
  twoFAInfo.code = '';
  errorMessage.value = '';
};

import { getErrorCode } from '@/utils/error-utils';

const mapLoginError = (err: unknown): string => {
  const code = getErrorCode(err);
  const codeMap: Record<string, string> = {
    authentication_failed: 'login.error.invalidCredentials',
    email_not_verified: 'login.error.emailNotVerified',
    invalid_2fa_code: 'login.error.invalid2faCode',
    token_expired: 'login.error.expired2faToken',
    conflict: 'login.error.emailAlreadyRegistered',
    disposable_email_not_allowed: 'login.error.disposableEmail',
    human_verification_failed: 'login.error.humanVerificationFailed',
    rate_limited: 'login.error.rateLimited',
    service_unavailable: 'login.error.verificationUnavailable',
  };
  if (code && codeMap[code]) return t(codeMap[code]);
  // Fallback: try error.api.* generic lookup
  const i18nKey = code ? `error.api.${code}` : '';
  const translated = i18nKey ? t(i18nKey) : '';
  if (translated && translated !== i18nKey) return translated;
  return t('login.error.generic');
};

const handleLoginSuccess = () => {
  const { redirect, ...othersQuery } = router.currentRoute.value.query;
  const targetName = (redirect as string) || 'Dashboard';
  router
    .push({ name: targetName, query: { ...othersQuery } })
    .catch(() => router.push({ name: 'Dashboard' }));
  toast.success(t('login.welcomeBack'));
  const { rememberPassword } = loginConfig.value;
  const { email } = userInfo;
  loginConfig.value.email = rememberPassword ? email : '';
};

const handle2FASubmit = async () => {
  if (loading.value || !twoFAInfo.code) return;
  setLoading(true);
  try {
    await userStore.verify2fa(twoFATempToken.value, twoFAInfo.code);
    handleLoginSuccess();
  } catch (err) {
    errorMessage.value = mapLoginError(err);
  } finally {
    setLoading(false);
  }
};

const handleTurnstileVerified = (token: string) => {
  turnstileToken.value = token;
  errorMessage.value = '';
};

const handleTurnstileExpired = () => {
  turnstileToken.value = '';
};

const handleTurnstileError = () => {
  turnstileToken.value = '';
  errorMessage.value = t('login.error.verificationUnavailable');
};

const handleSubmit = async () => {
  if (loading.value) return;
  setLoading(true);
  try {
    if (isLoginMode.value) {
      const tempToken = await userStore.login(userInfo as LoginRequest);
      if (tempToken) {
        is2FA.value = true;
        twoFATempToken.value = tempToken;
        errorMessage.value = '';
        return;
      }
      handleLoginSuccess();
    } else {
      if (!turnstileToken.value) {
        errorMessage.value = t('login.error.humanVerificationFailed');
        return;
      }
      // Build register payload, include invite_token and referral_code if present
      const registerPayload: any = { ...userInfo };
      registerPayload.turnstile_token = turnstileToken.value;
      if (inviteToken.value) {
        registerPayload.invite_token = inviteToken.value;
      }
      if (referralCode.value.trim()) {
        registerPayload.referral_code = referralCode.value.trim().toUpperCase();
      }
      await userStore.register(registerPayload);

      if (inviteToken.value) {
        // Invited registration: email is auto-verified, auto-login immediately
        const tempToken = await userStore.login({
          email: userInfo.email,
          password: userInfo.password,
        } as LoginRequest);
        if (tempToken) {
          is2FA.value = true;
          twoFATempToken.value = tempToken;
          errorMessage.value = '';
          return;
        }
        // Invite was already accepted during registration, go direct to Dashboard
        toast.success(t('login.welcomeBack'));
        router.push({ name: 'Dashboard' });
      } else {
        // Self-registration: go to email verification pending page
        router.push({ name: 'VerifyPending', query: { email: userInfo.email } });
      }
    }
  } catch (err) {
    errorMessage.value = mapLoginError(err);
    if (!isLoginMode.value) {
      turnstileToken.value = '';
      turnstileWidget.value?.reset();
    }
  } finally {
    setLoading(false);
  }
};
</script>
