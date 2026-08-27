<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
      <div class="min-w-0">
        <h2 class="text-lg sm:text-xl font-bold tracking-tight flex items-center gap-1.5 truncate">
          Payout #{{ id.slice(0, 8) }}…{{ id.slice(-8) }}
          <CopyButton :value="id" :title="id" />
        </h2>
        <p class="text-sm text-muted-foreground mt-0.5">
          {{ t('payouts.detailSubtitle') }}
        </p>
      </div>
      <Button variant="outline" size="sm" :disabled="refreshing" @click="handleRefresh">
        <RefreshCw class="h-3.5 w-3.5 mr-1.5" :class="{ 'animate-spin': refreshing }" />
        {{ t('payouts.refresh') }}
      </Button>
    </div>

    <!-- Error Alert -->
    <div
      v-if="payout.status === 'Failed' && payout.error_reason"
      class="rounded-md bg-destructive/10 border border-destructive/20 p-3 text-sm"
    >
      🔴 <strong>{{ t('payouts.failedAlert') }}</strong>
      {{ payout.error_reason }}
    </div>
    <div
      v-if="payout.status === 'Cancelled' && payout.error_reason"
      class="rounded-md bg-destructive/10 border border-destructive/20 p-3 text-sm"
    >
      🚫 <strong>{{ t('payouts.rejectedAlert') }}</strong>
      {{ payout.error_reason }}
    </div>
    <div
      v-if="payout.status === 'ApprovalExpired'"
      class="rounded-md bg-muted border p-3 text-sm text-muted-foreground"
    >
      ⏰ <strong>{{ t('payouts.expiredAlert') }}</strong>
      {{ payout.error_reason || '' }}
    </div>

    <!-- Payout Info -->
    <Card>
      <CardHeader>
        <CardTitle>{{ t('payouts.payoutInfo') }}</CardTitle>
      </CardHeader>
      <CardContent>
        <dl class="grid grid-cols-1 sm:grid-cols-2 gap-x-6 gap-y-3 text-sm">
          <div>
            <dt class="text-muted-foreground">{{ t('table.status') }}</dt>
            <dd class="font-medium mt-0.5">
              <Badge :variant="getVariant(payout.status)">{{ t(`payouts.status.${payout.status}`) }}</Badge>
            </dd>
          </div>
          <div>
            <dt class="text-muted-foreground">{{ t('table.network') }}</dt>
            <dd class="font-medium mt-0.5">
              <Badge variant="outline">{{ networkDisplayName(payout.network ?? '', envStore.isSandbox) }}</Badge>
            </dd>
          </div>
          <div>
            <dt class="text-muted-foreground">{{ t('payouts.amount') }}</dt>
            <dd class="font-medium mt-0.5 tabular-nums">
              {{ formatUsdt(payout.amount) }} {{ payout.currency }}
            </dd>
          </div>
          <div>
            <dt class="text-muted-foreground">{{ t('payouts.fee') }}</dt>
            <dd class="font-medium mt-0.5 tabular-nums text-red-500">
              -{{ formatUsdt(payout.fee) }} {{ payout.currency }}
            </dd>
          </div>
          <div>
            <dt class="text-muted-foreground">{{ t('payouts.netAmount') }}</dt>
            <dd class="font-medium mt-0.5 tabular-nums text-green-600">
              {{ formatUsdt(payout.net_amount) }} {{ payout.currency }}
            </dd>
          </div>
          <div>
            <dt class="text-muted-foreground">{{ t('payouts.toAddress') }}</dt>
            <dd class="font-mono text-xs mt-0.5 flex items-center gap-1.5">
              {{ payout.to_address }}
              <CopyButton v-if="payout.to_address" :value="payout.to_address" show-toast />
            </dd>
          </div>
          <div>
            <dt class="text-muted-foreground">{{ t('table.created') }}</dt>
            <dd class="mt-0.5">{{ formatDate(payout.created_at) }}</dd>
          </div>
          <div v-if="payout.completed_at">
            <dt class="text-muted-foreground">{{ t('payouts.completedAt') }}</dt>
            <dd class="mt-0.5">{{ formatDate(payout.completed_at) }}</dd>
          </div>
          <div v-if="payout.reviewed_by">
            <dt class="text-muted-foreground">{{ t('payouts.reviewedBy') }}</dt>
            <dd class="mt-0.5">{{ payout.reviewed_by }}</dd>
          </div>
          <div v-if="payout.reviewed_at">
            <dt class="text-muted-foreground">{{ t('payouts.reviewedAt') }}</dt>
            <dd class="mt-0.5">{{ formatDate(payout.reviewed_at) }}</dd>
          </div>
        </dl>

        <!-- Approve / Reject actions -->
        <div v-if="payout.status === 'PendingApproval' && canApprove" class="flex gap-2 pt-4 border-t mt-4">
          <Button @click="openApproval('approve')">
            {{ t('approval.approve') }}
          </Button>
          <Button variant="destructive" @click="openApproval('reject')">
            {{ t('approval.reject') }}
          </Button>
        </div>
      </CardContent>
    </Card>

    <!-- Transaction -->
    <Card v-if="payout.tx_hash">
      <CardHeader>
        <CardTitle>{{ t('payouts.txInfo') }}</CardTitle>
      </CardHeader>
      <CardContent>
        <dl class="grid grid-cols-1 gap-y-3 text-sm">
          <div>
            <dt class="text-muted-foreground">{{ t('payouts.txHash') }}</dt>
            <dd class="font-mono text-xs mt-0.5 flex items-center gap-1.5">
              {{ payout.tx_hash }}
              <CopyButton :value="payout.tx_hash!" show-toast />
              <button
                class="text-muted-foreground hover:text-foreground"
                @click="openExplorer(payout.tx_hash!, 'tx', payout.network)"
              >
                <ExternalLink class="h-3 w-3" />
              </button>
            </dd>
          </div>
        </dl>
      </CardContent>
    </Card>

    <!-- Description & Metadata -->
    <Card v-if="payout.description">
      <CardHeader>
        <CardTitle>{{ t('payouts.description') }}</CardTitle>
      </CardHeader>
      <CardContent>
        <p class="text-sm">{{ payout.description }}</p>
      </CardContent>
    </Card>

    <!-- Webhook Logs -->
    <Card>
      <CardHeader>
        <CardTitle>{{ t('sessionDetail.webhookLogs') }}</CardTitle>
      </CardHeader>
      <CardContent>
        <div class="overflow-x-auto -mx-4 md:mx-0">
        <Table class="min-w-[340px]">
          <TableHeader>
            <TableRow>
              <TableHead class="w-[100px]">{{ t('table.status') }}</TableHead>
              <TableHead class="hidden md:table-cell">{{ t('table.event') }}</TableHead>
              <TableHead>{{ t('table.time') }}</TableHead>
              <TableHead class="hidden sm:table-cell w-[80px]">HTTP</TableHead>
              <TableHead class="text-right">{{ t('table.actions') }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="log in webhookLogs" :key="log.id">
              <TableCell>
                <Badge :variant="getWebhookVariant(log.status)">{{ log.status }}</Badge>
              </TableCell>
              <TableCell class="hidden md:table-cell text-sm">{{ log.eventType }}</TableCell>
              <TableCell class="text-sm text-muted-foreground">
                {{ formatDate(log.createdAt) }}
              </TableCell>
              <TableCell class="hidden sm:table-cell">
                <Badge variant="outline">{{ log.httpStatus || 'N/A' }}</Badge>
              </TableCell>
              <TableCell class="text-right space-x-1">
                <Button variant="ghost" size="sm" @click="selectedLog = log; drawerOpen = true">
                  {{ t('sessionDetail.view') }}
                </Button>
                <Button variant="ghost" size="sm" @click="retryLog(log.id)">
                  {{ t('developer.resend') }}
                </Button>
              </TableCell>
            </TableRow>
            <TableRow v-if="webhookLogs.length === 0">
              <TableCell colspan="5" class="text-center text-muted-foreground py-6">
                {{ t('sessionDetail.noWebhookLogs') }}
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
        </div>
      </CardContent>
    </Card>

    <!-- Webhook Detail Drawer -->
    <WebhookLogDrawer v-model:open="drawerOpen" :log="selectedLog" @resent="fetchWebhookLogs" />

    <!-- Approval Dialog -->
    <ApprovalDialog
      v-model:open="approvalOpen"
      :action="approvalAction"
      :target-id="id"
      target-type="payout"
      @done="fetchDetail"
    />
  </div>
</template>

<script lang="ts" setup>
import { ref, computed } from 'vue';
import { useRoute } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { toast } from 'vue-sonner';
import { ExternalLink, RefreshCw } from 'lucide-vue-next';
import { fmtAmt, formatUsdt } from '@/utils/currency';
import { networkDisplayName } from '@/utils/networkUtils';
import { openExplorer } from '@/utils/explorer';
import { useEnvironmentStore } from '@/stores/environment';
import { useUserStore } from '@/stores';
import { useCanApprove } from '@/composables/useCanApprove';
import { formatDateTimeFull } from '@/utils/date';
import CopyButton from '@/components/CopyButton.vue';
import ApprovalDialog from '@/components/ApprovalDialog.vue';
import { http } from '@/utils/request';
import { queryWebhookLogs, resendWebhook, type WebhookLog } from '@/api/developer';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import WebhookLogDrawer from '@/components/WebhookLogDrawer.vue';
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from '@/components/ui/table';

interface PayoutDetail {
  id: string;
  livemode: boolean;
  status: string;
  amount: string;
  fee: string;
  net_amount: string;
  currency: string;
  network: string;
  to_address: string;
  tx_hash?: string;
  description?: string;
  error_reason?: string;
  created_at: string;
  completed_at?: string;
  reviewed_by?: string;
  reviewed_at?: string;
}

const route = useRoute();
const { t } = useI18n();
const id = route.params.id as string;
const envStore = useEnvironmentStore();
const userStore = useUserStore();
const { canApprove } = useCanApprove();

// Approval dialog
const approvalOpen = ref(false);
const approvalAction = ref<'approve' | 'reject'>('approve');
const openApproval = (action: 'approve' | 'reject') => {
  approvalAction.value = action;
  approvalOpen.value = true;
};

const payout = ref<Partial<PayoutDetail>>({});
const refreshing = ref(false);
const webhookLogs = ref<WebhookLog[]>([]);
const drawerOpen = ref(false);
const selectedLog = ref<WebhookLog | null>(null);

const formatDate = (dateStr?: string) => formatDateTimeFull(dateStr);



const getVariant = (status?: string) => {
  const map: Record<string, 'default' | 'secondary' | 'destructive' | 'outline'> = {
    Completed: 'default',
    Pending: 'secondary',
    PendingApproval: 'outline',
    Processing: 'secondary',
    Failed: 'destructive',
    Cancelled: 'destructive',
    ApprovalExpired: 'outline',
  };
  return map[status || ''] || 'secondary';
};

const getWebhookVariant = (status: string) => {
  if (status === 'Success') return 'default' as const;
  if (status === 'Failed' || status === 'GivingUp') return 'destructive' as const;
  return 'secondary' as const;
};

const fetchDetail = async () => {
  try {
    const data = await http.get<PayoutDetail>(`/api/internal/merchants/payouts/${id}`);
    payout.value = data;
  } catch (e) {
    console.error(e);
  }
};

const fetchWebhookLogs = async () => {
  try {
    const res = await queryWebhookLogs({ source_id: id, pageSize: 20 });
    webhookLogs.value = res.list;
  } catch (e) { console.error(e); }
};

const retryLog = async (logId: string) => {
  try { await resendWebhook(logId); toast.success(t('developer.resendTriggered')); fetchWebhookLogs(); }
  catch { /* interceptor */ }
};

const handleRefresh = async () => {
  refreshing.value = true;
  try {
    await Promise.all([
      new Promise(r => setTimeout(r, 500)),
      fetchDetail(),
      fetchWebhookLogs(),
    ]);
    toast.success(t('payouts.refreshed'));
  } finally {
    refreshing.value = false;
  }
};

import { useSmartPolling } from '@/composables/useSmartPolling';

useSmartPolling(async () => {
  await Promise.all([fetchDetail(), fetchWebhookLogs()]);
});
</script>
