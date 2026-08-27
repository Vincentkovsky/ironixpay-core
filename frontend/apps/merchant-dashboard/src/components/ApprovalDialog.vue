<template>
  <Dialog v-model:open="localOpen">
    <DialogContent class="max-w-sm">
      <DialogHeader>
        <DialogTitle>{{ isApprove ? t('approval.approveTitle') : t('approval.rejectTitle') }}</DialogTitle>
        <DialogDescription>
          {{ isApprove ? t('approval.approveDesc') : t('approval.rejectDesc') }}
        </DialogDescription>
      </DialogHeader>
      <form class="space-y-4" @submit.prevent="submit">
        <!-- TOTP -->
        <div class="space-y-2">
          <Label for="approval-totp">{{ t('approval.totpCode') }}</Label>
          <Input
            id="approval-totp"
            v-model="totpCode"
            maxlength="6"
            placeholder="000000"
            class="font-mono tracking-widest text-center"
            autofocus
          />
        </div>
        <!-- Reason (reject only) -->
        <div v-if="!isApprove" class="space-y-2">
          <Label for="approval-reason">{{ t('approval.reason') }}</Label>
          <textarea
            id="approval-reason"
            v-model="reason"
            rows="2"
            class="flex w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
            :placeholder="t('approval.reasonPlaceholder')"
          />
        </div>
        <!-- Actions -->
        <div class="flex justify-end gap-2 pt-2">
          <Button variant="outline" type="button" @click="localOpen = false">
            {{ t('approval.cancel') }}
          </Button>
          <Button
            type="submit"
            :variant="isApprove ? 'default' : 'destructive'"
            :disabled="loading || totpCode.length < 6"
          >
            <Loader2 v-if="loading" class="h-4 w-4 mr-1.5 animate-spin" />
            {{ isApprove ? t('approval.approve') : t('approval.reject') }}
          </Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>

<script lang="ts" setup>
import { ref, computed, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { toast } from 'vue-sonner';
import { Loader2 } from 'lucide-vue-next';
import {
  approvePayout, rejectPayout,
  approveWithdrawal, rejectWithdrawal,
} from '@/api/payout-settings';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

const props = defineProps<{
  open: boolean;
  action: 'approve' | 'reject';
  targetId: string;
  targetType: 'payout' | 'withdrawal';
}>();

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void;
  (e: 'done'): void;
}>();

const { t } = useI18n();
const isApprove = computed(() => props.action === 'approve');

const localOpen = computed({
  get: () => props.open,
  set: (v) => emit('update:open', v),
});

const totpCode = ref('');
const reason = ref('');
const loading = ref(false);

// Reset form when dialog opens
watch(() => props.open, (open) => {
  if (open) {
    totpCode.value = '';
    reason.value = '';
  }
});

const submit = async () => {
  if (totpCode.value.length < 6) return;
  loading.value = true;
  try {
    if (props.targetType === 'payout') {
      if (isApprove.value) {
        await approvePayout(props.targetId, totpCode.value);
      } else {
        await rejectPayout(props.targetId, totpCode.value, reason.value || undefined);
      }
    } else {
      if (isApprove.value) {
        await approveWithdrawal(props.targetId, totpCode.value);
      } else {
        await rejectWithdrawal(props.targetId, totpCode.value, reason.value || undefined);
      }
    }
    toast.success(isApprove.value ? t('approval.approveSuccess') : t('approval.rejectSuccess'));
    localOpen.value = false;
    emit('done');
  } catch {
    // interceptor shows backend error
  } finally {
    loading.value = false;
  }
};
</script>
