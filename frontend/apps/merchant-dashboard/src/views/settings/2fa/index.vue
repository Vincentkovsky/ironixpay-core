<template>
  <div class="space-y-6">
    <!-- Back link -->
    <Button variant="ghost" size="sm" @click="$router.push({ name: 'Settings' })">
      <ArrowLeft class="h-3.5 w-3.5 mr-1.5" />
      {{ t('twoFactor.back') }}
    </Button>

    <Card class="animate-fade-in-up">
      <CardHeader>
        <div class="flex items-center gap-2">
          <div class="stat-icon-box h-8 w-8 flex items-center justify-center rounded-md">
            <ShieldCheck class="h-4 w-4 text-brand" />
          </div>
          <div>
            <CardTitle>{{ t('twoFactor.title') }}</CardTitle>
            <p class="text-sm text-muted-foreground mt-0.5">{{ t('twoFactor.description') }}</p>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <!-- Current Status -->
        <div class="flex items-center gap-3 mb-6">
          <Badge :variant="userStore.is_2fa_enabled ? 'default' : 'secondary'">
            {{ userStore.is_2fa_enabled ? t('twoFactor.enabled') : t('twoFactor.disabled') }}
          </Badge>
        </div>

        <!-- Already enabled → show disable form -->
        <div v-if="userStore.is_2fa_enabled" class="space-y-4 max-w-sm">
          <p class="text-sm text-muted-foreground">{{ t('twoFactor.disableConfirm') }}</p>
          <div class="space-y-1.5">
            <Label>{{ t('twoFactor.code') }}</Label>
            <Input v-model="disableCode" type="text" inputmode="numeric" maxlength="6" :placeholder="t('twoFactor.code')" />
          </div>
          <Button variant="destructive" :disabled="disabling || disableCode.length < 6" @click="handleDisable">
            {{ disabling ? t('twoFactor.disabling') : t('twoFactor.disableBtn') }}
          </Button>
        </div>

        <!-- Not enabled → show setup flow -->
        <div v-else>
          <!-- Step 0: Start button -->
          <div v-if="!setupData" class="my-2">
            <Button @click="startSetup" :disabled="settingUp">
              {{ t('twoFactor.enable') }}
            </Button>
          </div>

          <!-- Setup steps -->
          <div v-if="setupData" class="space-y-6">
            <!-- Step 1: QR Code -->
            <div class="space-y-2">
              <h3 class="text-sm font-medium">{{ t('twoFactor.step1') }}</h3>
              <p class="text-sm text-muted-foreground">{{ t('twoFactor.step1Desc') }}</p>
              <div class="bg-white border rounded-lg p-4 inline-block">
                <img :src="qrDataUrl" alt="QR Code" class="w-48 h-48" v-if="qrDataUrl" />
              </div>
            </div>

            <!-- Step 2: Backup codes -->
            <div class="space-y-2">
              <h3 class="text-sm font-medium">{{ t('twoFactor.step2') }}</h3>
              <p class="text-sm text-muted-foreground">{{ t('twoFactor.step2Desc') }}</p>
              <div class="bg-muted rounded-md p-3 font-mono text-xs grid grid-cols-2 gap-1 max-w-xs">
                <span v-for="code in setupData.backup_codes" :key="code">{{ code }}</span>
              </div>
              <Button variant="outline" size="sm" @click="copyBackupCodes">
                <Copy class="h-3.5 w-3.5 mr-1.5" />
                Copy
              </Button>
            </div>

            <!-- Step 3: Verify -->
            <div class="space-y-2 max-w-xs">
              <h3 class="text-sm font-medium">{{ t('twoFactor.step3') }}</h3>
              <p class="text-sm text-muted-foreground">{{ t('twoFactor.step3Desc') }}</p>
              <div class="space-y-1.5">
                <Label>{{ t('twoFactor.code') }}</Label>
                <Input v-model="verifyCode" type="text" inputmode="numeric" maxlength="6" :placeholder="t('twoFactor.code')" @keyup.enter="handleEnable" />
              </div>
              <Button :disabled="verifying || verifyCode.length < 6" @click="handleEnable">
                {{ verifying ? t('twoFactor.verifying') : t('twoFactor.verify') }}
              </Button>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  </div>
</template>

<script lang="ts" setup>
import { ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { useClipboard } from '@vueuse/core';
import { toast } from 'vue-sonner';
import { ArrowLeft, ShieldCheck, Copy } from 'lucide-vue-next';
import { useUserStore } from '@/stores';
import { http } from '@/utils/request';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

const { t } = useI18n();
const { copy } = useClipboard();
const userStore = useUserStore();

interface SetupData {
  secret: string;
  qr_code_uri: string;
  backup_codes: string[];
}

// Setup flow
const setupData = ref<SetupData | null>(null);
const qrDataUrl = ref('');
const settingUp = ref(false);
const verifyCode = ref('');
const verifying = ref(false);

// Disable flow
const disableCode = ref('');
const disabling = ref(false);

async function startSetup() {
  settingUp.value = true;
  try {
    const res = await http.post<SetupData>('/api/internal/merchants/2fa/setup');
    setupData.value = res;
    // Generate QR code from URI using a simple canvas-free approach
    // The qr_code_uri is an otpauth:// URI — we use Google Charts API for QR rendering
    qrDataUrl.value = `https://api.qrserver.com/v1/create-qr-code/?size=192x192&data=${encodeURIComponent(res.qr_code_uri)}`;
  } catch {
    // interceptor shows backend error
  } finally {
    settingUp.value = false;
  }
}

async function handleEnable() {
  if (verifyCode.value.length < 6) return;
  verifying.value = true;
  try {
    await http.post('/api/internal/merchants/2fa/enable', { code: verifyCode.value });
    toast.success(t('twoFactor.enableSuccess'));
    userStore.setInfo({ is_2fa_enabled: true });
    setupData.value = null;
    verifyCode.value = '';
  } catch {
    // interceptor shows backend error
  } finally {
    verifying.value = false;
  }
}

async function handleDisable() {
  if (disableCode.value.length < 6) return;
  disabling.value = true;
  try {
    await http.post('/api/internal/merchants/2fa/disable', { code: disableCode.value });
    toast.success(t('twoFactor.disableSuccess'));
    userStore.setInfo({ is_2fa_enabled: false });
    disableCode.value = '';
  } catch {
    // interceptor shows backend error
  } finally {
    disabling.value = false;
  }
}

function copyBackupCodes() {
  if (!setupData.value) return;
  copy(setupData.value.backup_codes.join('\n'));
  toast.success(t('twoFactor.copied'));
}
</script>
