<template>
  <div class="flex min-h-screen items-center justify-center p-8">
    <div class="w-full max-w-md space-y-6 animate-fade-in-up">
      <!-- Logo -->
      <img src="/brand/logo-wordmark.svg" alt="IronixPay" class="h-6 mb-6" />

      <!-- Loading state -->
      <Card v-if="status === 'loading'">
        <CardContent class="flex flex-col items-center py-12">
          <Loader2 class="h-8 w-8 animate-spin text-brand mb-4" />
          <p class="text-sm text-muted-foreground">{{ t('acceptInvite.processing') }}</p>
        </CardContent>
      </Card>

      <!-- Success -->
      <Card v-else-if="status === 'success'">
        <CardContent class="flex flex-col items-center py-12 text-center">
          <div class="flex h-12 w-12 items-center justify-center rounded-full bg-green-100 mb-4">
            <CheckCircle2 class="h-6 w-6 text-green-600" />
          </div>
          <h2 class="text-lg font-semibold mb-1">{{ t('acceptInvite.successTitle') }}</h2>
          <p class="text-sm text-muted-foreground mb-6">{{ successMessage }}</p>
          <Button @click="goToDashboard">{{ t('acceptInvite.goToDashboard') }}</Button>
        </CardContent>
      </Card>

      <!-- Error -->
      <Card v-else-if="status === 'error'">
        <CardContent class="flex flex-col items-center py-12 text-center">
          <div class="flex h-12 w-12 items-center justify-center rounded-full bg-destructive/10 mb-4">
            <AlertCircle class="h-6 w-6 text-destructive" />
          </div>
          <h2 class="text-lg font-semibold mb-1">{{ t('acceptInvite.errorTitle') }}</h2>
          <p class="text-sm text-muted-foreground mb-6">{{ errorMessage }}</p>
          <div class="flex gap-3">
            <Button variant="outline" @click="$router.push('/login')">{{ t('acceptInvite.goToLogin') }}</Button>
            <Button @click="$router.push('/dashboard')">{{ t('acceptInvite.goToDashboard') }}</Button>
          </div>
        </CardContent>
      </Card>

      <!-- Not logged in -->
      <Card v-else-if="status === 'need-login'">
        <CardContent class="flex flex-col items-center py-12 text-center">
          <div class="flex h-12 w-12 items-center justify-center rounded-full bg-brand/10 mb-4">
            <UserPlus class="h-6 w-6 text-brand" />
          </div>
          <h2 class="text-lg font-semibold mb-1">{{ t('acceptInvite.invitedTitle') }}</h2>
          <p class="text-sm text-muted-foreground mb-6">{{ t('acceptInvite.invitedDesc') }}</p>
          <div class="flex gap-3">
            <Button variant="outline" @click="goToLogin">{{ t('acceptInvite.signIn') }}</Button>
            <Button @click="goToRegister">{{ t('acceptInvite.createAccount') }}</Button>
          </div>
        </CardContent>
      </Card>
    </div>
  </div>
</template>

<script lang="ts" setup>
import { ref, onMounted } from 'vue';
import { useRouter, useRoute } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { Loader2, CheckCircle2, AlertCircle, UserPlus } from 'lucide-vue-next';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { getToken, clearToken } from '@/utils/auth';
import { acceptInvite } from '@/api/team';

const { t } = useI18n();
const router = useRouter();
const route = useRoute();

type Status = 'loading' | 'success' | 'error' | 'need-login';
const status = ref<Status>('loading');
const successMessage = ref('');
const errorMessage = ref('');

onMounted(async () => {
  const token = route.query.token as string;
  if (!token) {
    status.value = 'error';
    errorMessage.value = t('acceptInvite.missingToken');
    return;
  }

  // Check if user is logged in with a valid (non-expired) token
  const authToken = getToken();
  if (!authToken) {
    status.value = 'need-login';
    return;
  }

  // Check if token is expired
  try {
    const payload = authToken.split('.')[1] as string;
    const decoded = JSON.parse(atob(payload));
    if (decoded.exp && Date.now() / 1000 > decoded.exp) {
      clearToken();
      status.value = 'need-login';
      return;
    }
  } catch {
    clearToken();
    status.value = 'need-login';
    return;
  }

  // Accept the invitation
  try {
    const res = await acceptInvite(token);
    status.value = 'success';
    successMessage.value = res.message || t('acceptInvite.successDefault');
  } catch (err: any) {
    status.value = 'error';
    errorMessage.value = err._backendMessage || err.message || t('acceptInvite.errorDefault');
  }
});

function goToDashboard() {
  router.push('/dashboard');
}

function goToLogin() {
  const token = route.query.token;
  router.push({ path: '/login', query: { redirect: 'AcceptInvite', token: token as string } });
}

function goToRegister() {
  const token = route.query.token;
  router.push({ path: '/login', query: { redirect: 'AcceptInvite', token: token as string, mode: 'register' } });
}
</script>
