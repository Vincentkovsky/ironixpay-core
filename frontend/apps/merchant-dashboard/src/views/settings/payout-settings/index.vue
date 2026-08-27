<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="animate-fade-in-up">
      <h2 class="text-lg sm:text-xl font-bold tracking-tight">{{ t('payoutSettings.title') }}</h2>
      <p class="text-sm text-muted-foreground mt-1">{{ t('payoutSettings.subtitle') }}</p>
    </div>

    <!-- Read-only notice for non-owners -->
    <div v-if="!isOwner" class="rounded-md border bg-muted/50 p-3 text-sm text-muted-foreground">
      🔒 {{ t('payoutSettings.readOnly') }}
    </div>

    <!-- New Address Approval Card -->
    <Card class="animate-fade-in-up" style="animation-delay: 60ms">
      <CardContent class="pt-6">
        <div class="flex items-center justify-between">
          <div class="space-y-0.5">
            <Label class="text-sm font-medium">{{ t('payoutSettings.newAddressApproval') }}</Label>
            <p class="text-xs text-muted-foreground">{{ t('payoutSettings.newAddressApprovalDesc') }}</p>
          </div>
          <Switch
            v-model="form.requireNewAddressApproval"
            :disabled="!isOwner || saving"
          />
        </div>
      </CardContent>
    </Card>

    <!-- Threshold Approval Card -->
    <Card class="animate-fade-in-up" style="animation-delay: 120ms">
      <CardContent class="pt-6 space-y-4">
        <div class="flex items-center justify-between">
          <div class="space-y-0.5">
            <Label class="text-sm font-medium">{{ t('payoutSettings.thresholdToggle') }}</Label>
            <p class="text-xs text-muted-foreground">{{ t('payoutSettings.thresholdToggleDesc') }}</p>
          </div>
          <Switch
            v-model="form.thresholdEnabled"
            :disabled="!isOwner || saving"
          />
        </div>
        <!-- Threshold input (visible when toggle is on) -->
        <div v-if="form.thresholdEnabled" class="max-w-xs pt-1">
          <Label class="text-xs text-muted-foreground">{{ t('payoutSettings.threshold') }} (USDT)</Label>
          <Input
            v-model="form.approvalThreshold"
            type="number"
            step="0.01"
            min="0"
            placeholder="5000"
            :disabled="!isOwner || saving"
            class="tabular-nums mt-1"
          />
          <p class="text-[11px] text-muted-foreground mt-1">{{ t('payoutSettings.thresholdDesc') }}</p>
        </div>
      </CardContent>
    </Card>

    <!-- Approver Roles Card -->
    <Card class="animate-fade-in-up" style="animation-delay: 180ms">
      <CardContent class="pt-6 space-y-3">
        <div>
          <Label class="text-sm font-medium">{{ t('payoutSettings.approverRoles') }}</Label>
          <p class="text-xs text-muted-foreground mt-0.5">{{ t('payoutSettings.approverRolesDesc') }}</p>
        </div>
        <div class="flex flex-wrap gap-3">
          <label
            v-for="role in availableRoles"
            :key="role"
            class="flex items-center gap-2 text-sm"
          >
            <Checkbox
              :model-value="role === 'owner' ? true : form.approverRoles.includes(role)"
              :disabled="role === 'owner' || !isOwner || saving"
              @update:model-value="toggleRole(role, $event as boolean)"
            />
            {{ t(`team.role.${role}`) }}
          </label>
        </div>
      </CardContent>
    </Card>

    <!-- Save -->
    <div v-if="isOwner" class="animate-fade-in-up" style="animation-delay: 240ms">
      <Button :disabled="saving" @click="save">
        <Loader2 v-if="saving" class="h-4 w-4 mr-1.5 animate-spin" />
        {{ saving ? t('settings.saving') : t('settings.save') }}
      </Button>
    </div>
  </div>
</template>

<script lang="ts" setup>
import { ref, reactive, computed, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { toast } from 'vue-sonner';
import { Loader2 } from 'lucide-vue-next';
import { useUserStore } from '@/stores';
import { getPayoutSettings, updatePayoutSettings } from '@/api/payout-settings';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Checkbox } from '@/components/ui/checkbox';

const { t } = useI18n();
const userStore = useUserStore();
const isOwner = computed(() => userStore.orgRole === 'owner');
const saving = ref(false);

const availableRoles = ['owner', 'admin', 'finance'];

const form = reactive({
  requireNewAddressApproval: true,
  thresholdEnabled: false,
  approvalThreshold: '5000',
  approverRoles: ['owner'] as string[],
});

const toggleRole = (role: string, checked: boolean) => {
  if (role === 'owner') return;
  if (checked) {
    if (!form.approverRoles.includes(role)) form.approverRoles.push(role);
  } else {
    form.approverRoles = form.approverRoles.filter(r => r !== role);
  }
};

const fetchSettings = async () => {
  try {
    const data = await getPayoutSettings();
    form.requireNewAddressApproval = data.requireNewAddressApproval ?? true;
    const threshold = data.approvalThreshold ?? '-1';
    // -1 = disabled, 0 = all amounts, >0 = exceeding threshold
    form.thresholdEnabled = parseFloat(threshold) >= 0;
    form.approvalThreshold = form.thresholdEnabled ? threshold : '5000';
    const roles = data.approverRoles ?? ['owner'];
    if (!roles.includes('owner')) roles.unshift('owner');
    form.approverRoles = roles;
  } catch {
    // use defaults
  }
};

const save = async () => {
  saving.value = true;
  try {
    const approvalThreshold = form.thresholdEnabled ? String(form.approvalThreshold) : '-1';

    await updatePayoutSettings({
      requireNewAddressApproval: form.requireNewAddressApproval,
      approvalThreshold,
      approverRoles: form.approverRoles,
    });
    toast.success(t('payoutSettings.saved'));
  } catch {
    // interceptor shows backend error
  } finally {
    saving.value = false;
  }
};

onMounted(fetchSettings);
</script>
