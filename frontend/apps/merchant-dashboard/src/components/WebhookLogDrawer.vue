<template>
  <Sheet v-model:open="open">
    <SheetContent class="w-full sm:w-[420px] sm:max-w-[420px] flex flex-col overflow-hidden px-0">
      <SheetHeader class="px-4 sm:px-6">
        <SheetTitle>{{ t('webhookDrawer.title') }}</SheetTitle>
      </SheetHeader>

      <template v-if="log">
        <!-- Scrollable content -->
        <div class="flex-1 overflow-y-auto px-4 sm:px-6 space-y-3 mt-2 pb-4">
          <!-- Status Hero -->
          <div class="flex items-center gap-2">
            <div
              class="h-7 w-7 rounded-full flex items-center justify-center"
              :class="log.status === 'Success' ? 'bg-green-500/10' : 'bg-destructive/10'"
            >
              <CheckCircle2 v-if="log.status === 'Success'" class="h-4 w-4 text-green-600" />
              <XCircle v-else class="h-4 w-4 text-destructive" />
            </div>
            <div>
              <Badge :variant="statusVariant(log.status)">{{ log.status }}</Badge>
              <p class="text-xs text-muted-foreground mt-0.5">{{ log.eventType }}</p>
            </div>
          </div>

          <!-- Detail Fields -->
          <dl class="space-y-2 text-sm">
            <div class="flex justify-between">
              <dt class="text-muted-foreground shrink-0">ID</dt>
              <dd class="font-mono text-xs text-right break-all max-w-[260px]">{{ log.id }}</dd>
            </div>
            <Separator />
            <div class="flex justify-between items-center">
              <dt class="text-muted-foreground shrink-0 mr-4">{{ t('webhookDrawer.targetUrl') }}</dt>
              <dd class="text-xs text-right break-all max-w-[260px]">{{ log.targetUrl }}</dd>
            </div>
            <Separator />
            <div class="flex justify-between items-center">
              <dt class="text-muted-foreground">HTTP</dt>
              <dd><Badge variant="outline">{{ log.httpStatus || 'N/A' }}</Badge></dd>
            </div>
            <Separator />
            <div class="flex justify-between items-center">
              <dt class="text-muted-foreground">{{ t('table.time') }}</dt>
              <dd>{{ formatDateTime(log.createdAt) }}</dd>
            </div>
            <Separator />
            <div class="flex justify-between items-center">
              <dt class="text-muted-foreground">{{ t('webhookDrawer.nextRetry') }}</dt>
              <dd>{{ log.nextRetryAt ? formatDateTime(log.nextRetryAt) : 'N/A' }}</dd>
            </div>
          </dl>

          <!-- Request Payload -->
          <div>
            <div class="flex items-center justify-between mb-2">
              <h4 class="text-sm font-medium">{{ t('webhookDrawer.requestPayload') }}</h4>
              <Button variant="ghost" size="sm" class="h-6 px-2" @click="copyPayload">
                <Copy class="h-3 w-3" />
              </Button>
            </div>
            <pre class="bg-muted rounded-md p-3 text-xs overflow-auto max-h-60 font-mono">{{ payloadFormatted }}</pre>
          </div>

        </div>

        <!-- Sticky bottom Resend -->
        <div class="shrink-0 border-t px-4 sm:px-6 py-3">
          <Button
            id="btn-drawer-resend"
            variant="outline"
            size="sm"
            class="w-full"
            :disabled="resending"
            @click="handleResend"
          >
            <RefreshCw class="h-3.5 w-3.5 mr-1.5" :class="{ 'animate-spin': resending }" />
            {{ t('developer.resend') }}
          </Button>
        </div>
      </template>

      <div v-else class="flex-1 flex justify-center items-center">
        <Loader2 class="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    </SheetContent>
  </Sheet>
</template>

<script lang="ts" setup>
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { CheckCircle2, XCircle, RefreshCw, Copy, Loader2 } from 'lucide-vue-next';
import { toast } from 'vue-sonner';
import { formatDateTime } from '@/utils/date';
import { resendWebhook, type WebhookLog } from '@/api/developer';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import { Sheet, SheetContent, SheetHeader, SheetTitle } from '@/components/ui/sheet';

const props = defineProps<{
  log: WebhookLog | null;
}>();

const open = defineModel<boolean>('open', { default: false });

const emit = defineEmits<{
  resent: [];
}>();

const { t } = useI18n();
const resending = ref(false);

const payloadFormatted = computed(() => {
  if (!props.log?.requestPayload) return '';
  return typeof props.log.requestPayload === 'string'
    ? props.log.requestPayload
    : JSON.stringify(props.log.requestPayload, null, 2);
});

const statusVariant = (status: string) => {
  if (status === 'Success') return 'default' as const;
  if (status === 'Failed' || status === 'GivingUp') return 'destructive' as const;
  return 'secondary' as const;
};

const copyPayload = async () => {
  try {
    await navigator.clipboard.writeText(payloadFormatted.value);
    toast.success(t('sessionDetail.copied'));
  } catch { /* */ }
};

const handleResend = async () => {
  if (!props.log) return;
  resending.value = true;
  try {
    await resendWebhook(props.log.id);
    toast.success(t('developer.resendTriggered'));
    emit('resent');
  } catch { /* interceptor */ }
  finally { resending.value = false; }
};
</script>
