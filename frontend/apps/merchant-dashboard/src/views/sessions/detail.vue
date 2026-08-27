<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
      <div class="min-w-0">
        <h2 class="text-lg sm:text-xl font-bold tracking-tight flex items-center gap-1.5 truncate">
          Session #{{ id.slice(0, 8) }}…{{ id.slice(-8) }}
          <CopyButton :value="id" :title="id" />
        </h2>
        <p class="text-sm text-muted-foreground mt-0.5 flex items-center gap-1.5 truncate">
          {{ t('sessionDetail.ref') }}: {{ session.clientReferenceId || 'N/A' }}
          <CopyButton v-if="session.clientReferenceId" :value="session.clientReferenceId" show-toast />
        </p>
      </div>
      <div class="flex gap-2">
        <Button variant="outline" size="sm" as="a" :href="session.url" target="_blank" :disabled="!session.url">
          <ExternalLink class="h-3.5 w-3.5 mr-1.5" />
          {{ t('sessionDetail.checkoutPage') }}
        </Button>
        <Button variant="outline" size="sm" :disabled="refreshing" @click="handleRefresh">
          <RefreshCw class="h-3.5 w-3.5 mr-1.5" :class="{ 'animate-spin': refreshing }" />
          {{ t('sessionDetail.refresh') }}
        </Button>
      </div>
    </div>

    <!-- Status Alert -->
    <div
      v-if="session.status === 'Underpaid'"
      class="rounded-md bg-orange-500/10 border border-orange-500/20 p-3 text-sm"
    >
      ⚠️ <strong>{{ t('sessionDetail.underpaidAlert') }}</strong>
      {{ t('sessionDetail.underpaidMsg', { received: fmtAmt(session.amountReceived), expected: fmtAmt(session.amountExpected), currency: session.currency }) }}
    </div>
    <!-- Expired + uncredited: guide to Resolution Center -->
    <div
      v-if="session.status === 'Expired' && (session.amountReceived || 0) > 0 && session.netAmount == null"
      class="rounded-md bg-destructive/10 border border-destructive/20 p-3 text-sm"
    >
      🔴 <strong>{{ t('sessionDetail.expiredAlert') }}</strong>
      {{ t('sessionDetail.expiredUncreditedMsg', { received: fmtAmt(session.amountReceived), currency: session.currency }) }}
      <router-link to="/resolution" class="underline font-medium text-primary hover:text-primary/80 ml-1">
        {{ t('sessionDetail.goToResolution') }} →
      </router-link>
    </div>
    <!-- Expired + credited (resolved via Resolution Center) -->
    <div
      v-if="session.status === 'Expired' && (session.amountReceived || 0) > 0 && session.netAmount != null"
      class="rounded-md bg-blue-500/10 border border-blue-500/20 p-3 text-sm"
    >
      ✅ <strong>{{ t('sessionDetail.expiredCreditedAlert') }}</strong>
      {{ t('sessionDetail.expiredCreditedMsg', { net: fmtAmt(session.netAmount), fee: fmtAmt(session.feeAmount), currency: session.currency }) }}
    </div>

    <!-- Payment Info -->
    <Card>
      <CardHeader>
        <CardTitle>{{ t('sessionDetail.paymentInfo') }}</CardTitle>
      </CardHeader>
      <CardContent>
        <dl class="grid grid-cols-1 sm:grid-cols-2 gap-x-6 gap-y-3 text-sm">
          <div>
            <dt class="text-muted-foreground">{{ t('sessionDetail.status') }}</dt>
            <dd class="font-medium mt-0.5">
              <Badge :variant="getVariant(session.status)">{{ t(`sessions.status.${session.status}`) }}</Badge>
            </dd>
          </div>
          <div>
            <dt class="text-muted-foreground">{{ t('sessionDetail.network') }}</dt>
            <dd class="font-medium mt-0.5">
              <Badge variant="outline">{{ networkDisplayName(session.network ?? '', envStore.isSandbox) }}</Badge>
            </dd>
          </div>
          <div>
            <dt class="text-muted-foreground">{{ t('sessionDetail.amountExpected') }}</dt>
            <dd class="font-medium mt-0.5 tabular-nums">
              {{ fmtAmt(session.amountExpected) }} {{ session.currency }}
            </dd>
          </div>
          <div>
            <dt class="text-muted-foreground">{{ t('sessionDetail.amountReceived') }}</dt>
            <dd
              class="font-medium mt-0.5 tabular-nums"
              :class="session.amountReceived !== session.amountExpected ? 'text-orange-500' : ''"
            >
              {{ fmtAmt(session.amountReceived) }} {{ session.currency }}
            </dd>
          </div>
          <!-- Fiat Pricing (only shown for fiat-priced sessions) -->
          <template v-if="session.pricing && session.pricing.currency !== session.currency">
            <div class="sm:col-span-2 border-t pt-3 mt-1">
              <dt class="text-muted-foreground text-xs uppercase tracking-wider font-medium">
                {{ t('sessionDetail.fiatPricing') }}
              </dt>
            </div>
            <div>
              <dt class="text-muted-foreground">{{ t('sessionDetail.pricingAmount') }}</dt>
              <dd class="font-medium mt-0.5 tabular-nums">
                {{ session.pricing.amount }} {{ session.pricing.currency }}
              </dd>
            </div>
            <div>
              <dt class="text-muted-foreground">{{ t('sessionDetail.exchangeRate') }}</dt>
              <dd class="font-medium mt-0.5 tabular-nums">
                1 {{ session.currency }} = {{ session.pricing.exchange_rate }} {{ session.pricing.currency }}
              </dd>
            </div>
          </template>
          <div v-if="session.feeAmount != null">
            <dt class="text-muted-foreground">{{ t('sessionDetail.feeAmount') }}</dt>
            <dd class="font-medium mt-0.5 tabular-nums text-red-500">
              -{{ fmtAmt(session.feeAmount) }} {{ session.currency }}
            </dd>
          </div>
          <div v-if="session.netAmount != null">
            <dt class="text-muted-foreground">{{ t('sessionDetail.netAmount') }}</dt>
            <dd class="font-medium mt-0.5 tabular-nums text-green-600">
              {{ fmtAmt(session.netAmount) }} {{ session.currency }}
            </dd>
          </div>
          <div class="sm:col-span-2">
            <dt class="text-muted-foreground">{{ t('sessionDetail.payAddress') }}</dt>
            <dd class="font-mono text-xs mt-0.5 flex items-center gap-1.5">
              {{ session.payAddress }}
              <CopyButton :value="session.payAddress" show-toast />
            </dd>
          </div>
          <div>
            <dt class="text-muted-foreground">{{ t('sessionDetail.created') }}</dt>
            <dd class="mt-0.5">{{ formatDate(session.createdTime) }}</dd>
          </div>
          <div>
            <dt class="text-muted-foreground">{{ t('sessionDetail.expires') }}</dt>
            <dd class="mt-0.5">{{ formatDate(session.expiresAt) }}</dd>
          </div>
        </dl>
      </CardContent>
    </Card>

    <!-- Xero Sync -->
    <Card v-if="xeroConnected">
      <CardHeader class="flex flex-row items-center justify-between gap-2 space-y-0">
        <CardTitle>{{ t('sessionDetail.xeroSync') }}</CardTitle>
        <Button variant="ghost" size="sm" :disabled="loadingXeroSync || retryingXero" @click="fetchXeroSyncInfo">
          <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': loadingXeroSync }" />
        </Button>
      </CardHeader>
      <CardContent>
        <div v-if="loadingXeroSync && !xeroSyncLog" class="text-sm text-muted-foreground">
          {{ t('sessionDetail.xeroLoading') }}
        </div>
        <div v-else-if="!xeroSyncLog" class="text-sm text-muted-foreground">
          {{ t('sessionDetail.xeroNoLog') }}
        </div>
        <div v-else class="space-y-3">
          <dl class="grid grid-cols-1 sm:grid-cols-2 gap-x-6 gap-y-3 text-sm">
            <div>
              <dt class="text-muted-foreground">{{ t('sessionDetail.xeroStatus') }}</dt>
              <dd class="mt-0.5">
                <Badge variant="outline" :class="getXeroStatusClass(xeroSyncLog.status)">
                  {{ getXeroStatusText(xeroSyncLog.status) }}
                </Badge>
              </dd>
            </div>
            <div>
              <dt class="text-muted-foreground">{{ t('sessionDetail.xeroAttempts') }}</dt>
              <dd class="font-medium mt-0.5 tabular-nums">{{ xeroSyncLog.attempt_count }}</dd>
            </div>
            <div>
              <dt class="text-muted-foreground">{{ t('sessionDetail.xeroInvoiceId') }}</dt>
              <dd class="font-mono text-xs mt-0.5 break-all">{{ xeroSyncLog.xero_invoice_id || '—' }}</dd>
            </div>
            <div>
              <dt class="text-muted-foreground">{{ t('sessionDetail.xeroPaymentId') }}</dt>
              <dd class="font-mono text-xs mt-0.5 break-all">{{ xeroSyncLog.xero_payment_id || '—' }}</dd>
            </div>
            <div>
              <dt class="text-muted-foreground">{{ t('sessionDetail.xeroUpdatedAt') }}</dt>
              <dd class="mt-0.5">{{ formatDate(xeroSyncLog.updated_at) }}</dd>
            </div>
            <div v-if="xeroSyncLog.last_error" class="sm:col-span-2">
              <dt class="text-muted-foreground">{{ t('sessionDetail.xeroError') }}</dt>
              <dd class="mt-0.5 text-destructive break-words">{{ xeroSyncLog.last_error }}</dd>
            </div>
          </dl>

          <div v-if="xeroSyncLog.status === 'failed'" class="pt-1">
            <Button variant="outline" size="sm" :disabled="retryingXero" @click="retryXeroLog">
              <RefreshCw class="h-3.5 w-3.5 mr-1.5" :class="{ 'animate-spin': retryingXero }" />
              {{ retryingXero ? t('xero.retrying') : t('xero.retry') }}
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>

    <!-- Transaction History -->
    <Card>
      <CardHeader>
        <CardTitle>{{ t('sessionDetail.txHistory') }}</CardTitle>
      </CardHeader>
      <CardContent>
        <div class="overflow-x-auto -mx-4 md:mx-0">
        <Table class="min-w-[320px]">
          <TableHeader>
            <TableRow>
              <TableHead>{{ t('sessionDetail.txHash') }}</TableHead>
              <TableHead>{{ t('sessionDetail.network') }}</TableHead>
              <TableHead>{{ t('table.amount') }}</TableHead>
              <TableHead class="hidden sm:table-cell">{{ t('table.time') }}</TableHead>
              <TableHead>{{ t('table.status') }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="tx in session.transactions" :key="tx.txHash">
              <TableCell>
                <div class="flex items-center gap-1.5">
                  <span class="font-mono text-xs">{{ tx.txHash.slice(0, 10) }}…</span>
                  <button
                    class="text-muted-foreground hover:text-foreground"
                    @click="openBlockchainBrowser(tx.txHash, tx.network)"
                  >
                    <ExternalLink class="h-3 w-3" />
                  </button>
                </div>
              </TableCell>
              <TableCell>
                <Badge variant="outline" :class="tx.network !== session.network ? 'border-orange-500/50 text-orange-600' : ''">
                  {{ networkDisplayName(tx.network, envStore.isSandbox) }}
                </Badge>
              </TableCell>
              <TableCell class="tabular-nums">
                + {{ fmtAmt(tx.amount) }} {{ session.currency }}
              </TableCell>
              <TableCell class="hidden sm:table-cell text-sm text-muted-foreground">
                {{ formatDate(tx.time) }}
              </TableCell>
              <TableCell>
                <Badge variant="default">{{ tx.status }}</Badge>
              </TableCell>
            </TableRow>
            <TableRow v-if="!session.transactions?.length">
              <TableCell colspan="5" class="text-center text-muted-foreground py-6">
                {{ t('sessionDetail.noTx') }}
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
        </div>
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
  </div>
</template>

<script lang="ts" setup>
import { ref, onMounted } from 'vue';
import { useRoute } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { useClipboard } from '@vueuse/core';
import { formatDateTimeFull } from '@/utils/date';
import { fmtAmt } from '@/utils/currency';
import { toast } from 'vue-sonner';
import { ExternalLink, RefreshCw } from 'lucide-vue-next';
import { networkDisplayName } from '@/utils/networkUtils';
import { useEnvironmentStore } from '@/stores/environment';
import CopyButton from '@/components/CopyButton.vue';
import { querySessionDetail, type SessionDetail } from '@/api/session';
import { queryWebhookLogs, resendWebhook, type WebhookLog } from '@/api/developer';
import { getXeroConnection, getXeroSyncLogs, retryXeroSync, type XeroSyncLog } from '@/api/xero';
import { txUrl } from '@/utils/explorer';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import WebhookLogDrawer from '@/components/WebhookLogDrawer.vue';
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from '@/components/ui/table';

const route = useRoute();
const { t } = useI18n();
const id = route.params.id as string;
const { copy } = useClipboard();
const envStore = useEnvironmentStore();

const session = ref<Partial<SessionDetail>>({});
const webhookLogs = ref<WebhookLog[]>([]);
const xeroConnected = ref(false);
const xeroSyncLog = ref<XeroSyncLog | null>(null);
const loadingXeroSync = ref(false);
const retryingXero = ref(false);
const drawerOpen = ref(false);
const selectedLog = ref<WebhookLog | null>(null);
const refreshing = ref(false);

const formatDate = (dateStr?: string) => formatDateTimeFull(dateStr);

const copyToClipboard = (text?: string) => {
  if (!text) return;
  copy(text);
  toast.success(t('sessionDetail.copied'));
};

const openBlockchainBrowser = (hash?: string, network?: string) => {
  if (!hash) return;
  window.open(txUrl(hash, network || session.value.network), '_blank');
};

const getVariant = (status?: string) => {
  const map: Record<string, 'default' | 'secondary' | 'destructive' | 'outline'> = {
    Paid: 'default', Pending: 'secondary', Expired: 'outline',
    Underpaid: 'destructive', Overpaid: 'default', Blocked: 'destructive',
  };
  return map[status || ''] || 'secondary';
};

const getWebhookVariant = (status: string) => {
  if (status === 'Success') return 'default' as const;
  if (status === 'Failed' || status === 'GivingUp') return 'destructive' as const;
  return 'secondary' as const;
};

const getXeroStatusClass = (status?: string) => {
  switch (status) {
    case 'synced':
      return 'border-emerald-200 bg-emerald-50 text-emerald-700';
    case 'failed':
      return 'border-red-200 bg-red-50 text-red-700';
    case 'pending':
      return 'border-amber-200 bg-amber-50 text-amber-700';
    case 'skipped':
      return 'border-slate-200 bg-slate-50 text-slate-700';
    default:
      return '';
  }
};

const getXeroStatusText = (status?: string) => {
  if (!status) return '—';
  const syncKey = `xero.syncStatus_${status}`;
  const syncText = t(syncKey);
  if (syncText !== syncKey) return syncText;
  const fallbackKey = `xero.${status}`;
  const fallbackText = t(fallbackKey);
  if (fallbackText !== fallbackKey) return fallbackText;
  return status;
};

const fetchDetails = async () => {
  try {
    const { data } = await querySessionDetail(id);
    session.value = data;
  } catch (e) { console.error(e); }
};

const fetchXeroSyncInfo = async () => {
  loadingXeroSync.value = true;
  try {
    const connection = await getXeroConnection();
    xeroConnected.value = connection?.status === 'active';
    if (!xeroConnected.value) {
      xeroSyncLog.value = null;
      return;
    }

    const logs = await getXeroSyncLogs(1, 1, id);
    xeroSyncLog.value = logs.data[0] ?? null;
  } catch (e) {
    console.error(e);
    xeroConnected.value = false;
    xeroSyncLog.value = null;
  } finally {
    loadingXeroSync.value = false;
  }
};

const handleRefresh = async () => {
  refreshing.value = true;
  try {
    await Promise.all([
      new Promise(r => setTimeout(r, 500)),
      fetchDetails(),
      fetchWebhookLogs(),
      fetchXeroSyncInfo(),
    ]);
    toast.success(t('sessionDetail.refreshed'));
  } finally {
    refreshing.value = false;
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

const retryXeroLog = async () => {
  if (!xeroSyncLog.value) return;
  retryingXero.value = true;
  try {
    await retryXeroSync(xeroSyncLog.value.id);
    toast.success(t('xero.retryQueued'));
    await fetchXeroSyncInfo();
  } catch {
    toast.error(t('xero.retryFailed'));
  } finally {
    retryingXero.value = false;
  }
};

import { useSmartPolling } from '@/composables/useSmartPolling';

useSmartPolling(async () => {
  await Promise.all([fetchDetails(), fetchXeroSyncInfo()]);
});

onMounted(() => { fetchWebhookLogs(); });
</script>
