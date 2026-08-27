<template>
  <div class="space-y-6">
    <!-- Billing Logs -->
    <Card class="animate-fade-in-up">
      <CardHeader>
        <div class="flex flex-col gap-3">
          <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
            <CardTitle>{{ t('billing.billingHistory') }}</CardTitle>
            <Button variant="outline" size="sm" @click="fetchBilling">
              <RefreshCw class="h-3.5 w-3.5" />
            </Button>
          </div>
          <!-- Network Tabs -->
          <div class="flex flex-wrap gap-1.5">
            <Button
              v-for="tab in billingNetworkTabs"
              :key="tab.value"
              :variant="billingNetwork === tab.value ? 'default' : 'outline'"
              size="sm"
              class="h-7 text-xs px-3"
              @click="switchBillingNetwork(tab.value)"
            >
              {{ tab.label }}
            </Button>
          </div>
          <div v-if="hasSubMerchants" class="space-y-1.5 w-full sm:w-48">
            <Label>{{ t('subMerchantFilter.label') }}</Label>
            <Select v-model="smSelected" @update:modelValue="() => { billingPagination.current = 1; fetchBilling(); }">
              <SelectTrigger class="w-full min-w-0 [&>span]:truncate">
                <SelectValue :placeholder="t('subMerchantFilter.label')" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="_all">{{ t('subMerchantFilter.all') }}</SelectItem>
                <SelectItem value="_self">{{ t('subMerchantFilter.self') }}</SelectItem>
                <SelectItem v-for="sm in smList" :key="sm.sub_merchant_code" :value="sm.sub_merchant_code">
                  {{ sm.display_name || sm.sub_merchant_code }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div class="flex flex-wrap items-end gap-2">
            <div class="flex flex-col gap-1">
              <Label class="text-xs text-muted-foreground">{{ t('billing.startDate') }}</Label>
              <Input type="date" v-model="exportStartDate" class="w-36 h-8 text-sm" />
            </div>
            <div class="flex flex-col gap-1">
              <Label class="text-xs text-muted-foreground">{{ t('billing.endDate') }}</Label>
              <Input type="date" v-model="exportEndDate" class="w-36 h-8 text-sm" />
            </div>
            <Button variant="outline" size="sm" class="h-8" :disabled="exporting" @click="exportCsv">
              <Download class="h-3.5 w-3.5 mr-1.5" />
              {{ exporting ? t('billing.exporting') : t('billing.exportCsv') }}
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <div class="overflow-x-auto -mx-4 md:mx-0">
        <Table class="min-w-[320px]">
          <TableHeader>
            <TableRow>
              <TableHead>{{ t('table.time') }}</TableHead>
              <TableHead v-if="!billingNetwork" class="hidden sm:table-cell w-[70px]">{{ t('table.network') }}</TableHead>
              <TableHead>{{ t('table.type') }}</TableHead>
              <TableHead class="hidden md:table-cell">{{ t('billing.reference') }}</TableHead>
              <TableHead v-if="hasSubMerchants" class="hidden md:table-cell">{{ t('table.subMerchant') }}</TableHead>
              <TableHead class="text-right">{{ t('table.amount') }}</TableHead>
              <TableHead v-if="billingNetwork" class="hidden sm:table-cell text-right">{{ t('billing.balanceAfter') }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="log in billingLogs" :key="(log as any).id" class="cursor-pointer hover:bg-muted/50" @click="openBillingDetail((log as any).id)">
              <TableCell class="text-sm">{{ formatTime((log as any).createdAt) }}</TableCell>
              <TableCell v-if="!billingNetwork" class="hidden sm:table-cell">
                <Badge variant="secondary" class="text-[10px] px-1.5 py-0">{{ (log as any).network }}</Badge>
              </TableCell>
              <TableCell>
                <Badge variant="outline">{{ smartTypeLabel(log) }}</Badge>
              </TableCell>
              <TableCell class="hidden md:table-cell font-mono text-xs text-muted-foreground">
                {{ refDisplayText(log) || '—' }}
              </TableCell>
              <TableCell v-if="hasSubMerchants" class="hidden md:table-cell">
                <Badge v-if="(log as any).subMerchantCode" variant="outline" :class="['text-[10px] px-1.5 py-0', smColorClass((log as any).subMerchantCode)]">
                  {{ (log as any).subMerchantCode }}
                </Badge>
                <span v-else class="text-muted-foreground">—</span>
              </TableCell>
              <TableCell class="text-right tabular-nums font-medium"
                :class="parseFloat((log as any).amountChange) >= 0 ? 'text-green-600' : 'text-red-500'"
              >
                {{ formatUsdtSigned((log as any).amountChange) }}
              </TableCell>
              <TableCell v-if="billingNetwork" class="hidden sm:table-cell text-right tabular-nums">
                {{ formatUsdt((log as any).balanceAfter) }}
              </TableCell>
            </TableRow>
            <TableRow v-if="billingLogs.length === 0">
              <TableCell colspan="5" class="text-center text-muted-foreground py-12">
                <div class="flex flex-col items-center gap-2">
                  <Inbox class="h-8 w-8 text-muted-foreground/30" />
                  <span>{{ t('billing.noBilling') }}</span>
                </div>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
        </div>

        <!-- Pagination -->
        <div
          v-if="billingPagination.total > billingPagination.pageSize"
          class="flex flex-col sm:flex-row items-center justify-between gap-3 pt-4 border-t mt-4"
        >
          <span class="text-sm text-muted-foreground">
            {{ t('sessions.results', { count: billingPagination.total }) }}
          </span>
          <div class="flex gap-1">
            <Button
              variant="outline"
              size="sm"
              :disabled="billingPagination.current <= 1"
              @click="onBillingPageChange(billingPagination.current - 1)"
            >
              {{ t('sessions.previous') }}
            </Button>
            <Button
              variant="outline"
              size="sm"
              :disabled="billingPagination.current * billingPagination.pageSize >= billingPagination.total"
              @click="onBillingPageChange(billingPagination.current + 1)"
            >
              {{ t('sessions.next') }}
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>

    <!-- Billing Log Detail Drawer -->
    <Sheet v-model:open="blDetailOpen">
      <SheetContent class="w-full sm:w-[420px] sm:max-w-[420px] overflow-y-auto px-4 sm:px-6">
        <SheetHeader>
          <SheetTitle>{{ t('billing.logDetail') }}</SheetTitle>
        </SheetHeader>
        <div v-if="blDetail" class="space-y-5 mt-6">
          <!-- Balance Flow Visual -->
          <div class="rounded-lg border bg-muted/30 p-4">
            <div class="flex items-center justify-between">
              <div class="text-center">
                <p class="text-xs text-muted-foreground mb-1">{{ t('billing.previousBalance') }}</p>
                <p class="font-mono tabular-nums text-sm">{{ formatUsdt((blDetail as any).previousBalance) }}</p>
              </div>
              <div class="text-center px-3">
                <p class="text-xs text-muted-foreground mb-1">{{ t('billing.amountChange') }}</p>
                <p class="font-mono tabular-nums text-sm font-semibold"
                  :class="parseFloat((blDetail as any).amountChange) >= 0 ? 'text-green-600' : 'text-red-500'"
                >{{ formatUsdtSigned((blDetail as any).amountChange) }}</p>
              </div>
              <div class="text-center">
                <p class="text-xs text-muted-foreground mb-1">{{ t('billing.balanceAfter') }}</p>
                <p class="font-mono tabular-nums text-sm font-semibold">{{ formatUsdt((blDetail as any).balanceAfter) }}</p>
              </div>
            </div>
          </div>

          <!-- Detail Fields -->
          <dl class="space-y-3 text-sm">
            <div class="flex justify-between">
              <dt class="text-muted-foreground shrink-0">ID</dt>
              <dd class="font-mono text-xs text-right break-all max-w-[260px]">{{ (blDetail as any).id }}</dd>
            </div>
            <Separator />
            <div class="flex justify-between items-center">
              <dt class="text-muted-foreground">{{ t('table.type') }}</dt>
              <dd><Badge variant="outline">{{ smartTypeLabel(blDetail) }}</Badge></dd>
            </div>
            <Separator />
            <div class="flex justify-between items-center">
              <dt class="text-muted-foreground shrink-0 mr-4">{{ t('billing.reference') }}</dt>
              <dd class="text-right">
                <router-link
                  v-if="refLink(blDetail)"
                  :to="refLink(blDetail)!"
                  class="text-blue-500 hover:underline font-mono text-xs"
                  @click="blDetailOpen = false"
                >
                  {{ refDisplayText(blDetail) }}
                </router-link>
                <span v-else class="font-mono text-xs text-muted-foreground">
                  {{ (blDetail as any).externalRefId || '—' }}
                </span>
              </dd>
            </div>
            <Separator />
            <div class="flex justify-between">
              <dt class="text-muted-foreground">{{ t('table.time') }}</dt>
              <dd>{{ formatTime((blDetail as any).createdAt) }}</dd>
            </div>
          </dl>
        </div>
        <div v-else class="flex justify-center py-12">
          <Loader2 class="h-6 w-6 animate-spin text-muted-foreground" />
        </div>
      </SheetContent>
    </Sheet>
  </div>
</template>

<script lang="ts" setup>
import { ref, reactive, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { RefreshCw, Loader2, Inbox, Download } from 'lucide-vue-next';
import { toast } from 'vue-sonner';
import { useUserStore } from '@/stores';
import { http } from '@/utils/request';
import { formatUsdt, formatUsdtSigned } from '@/utils/currency';
import { formatDateTimeFull } from '@/utils/date';
import { queryBillingLogs, queryBillingLogDetail, type BillingLogResponse } from '@/api/finance';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Sheet, SheetContent, SheetHeader, SheetTitle } from '@/components/ui/sheet';
import { Separator } from '@/components/ui/separator';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { useSubMerchantFilter, smColorClass } from '@/composables/useSubMerchantFilter';

const { t } = useI18n();
const userStore = useUserStore();
const smFilter = useSubMerchantFilter();
const smSelected = smFilter.selected;
const smList = smFilter.subMerchants;
const hasSubMerchants = smFilter.hasSubMerchants;

smFilter.loadSubMerchants();

// Chain rows for tab generation
const chainRows = computed(() => {
  const balances = userStore.chainBalances;
  return Object.keys(balances).map((network) => ({ network }));
});

// CSV Export
const exportStartDate = ref('');
const exportEndDate = ref('');
const exporting = ref(false);

const exportCsv = async () => {
  exporting.value = true;
  try {
    const params = new URLSearchParams();
    if (exportStartDate.value) params.set('start_date', exportStartDate.value);
    if (exportEndDate.value) params.set('end_date', exportEndDate.value);
    const smParams = smFilter.filterParams.value;
    if (smParams.include_sub_merchants) params.set('include_sub_merchants', 'true');
    if (smParams.sub_merchant_code) params.set('sub_merchant_code', smParams.sub_merchant_code);

    const blob = await http.get<Blob>(`/api/internal/billing/logs/export?${params.toString()}`, {
      responseType: 'blob',
      skipErrorToast: true,
    });

    const parts = ['billing'];
    if (exportStartDate.value) parts.push(exportStartDate.value);
    if (exportEndDate.value) parts.push(exportEndDate.value);
    const filename = parts.length > 1 ? `${parts.join('_')}.csv` : 'billing_export.csv';

    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);

    toast.success(t('billing.exportCsv') + ' ✓');
  } catch {
    toast.error(t('billing.exportEmpty'));
  } finally {
    exporting.value = false;
  }
};

// Billing
const billingLogs = ref<BillingLogResponse[]>([]);
const billingLoading = ref(false);
const billingPagination = reactive({ current: 1, pageSize: 20, total: 0 });
const billingNetwork = ref('');

const billingNetworkTabs = computed(() => {
  const tabs = [{ value: '', label: t('common.all') || 'All' }];
  for (const row of chainRows.value) {
    tabs.push({ value: row.network, label: row.network });
  }
  return tabs;
});

const switchBillingNetwork = (network: string) => {
  billingNetwork.value = network;
  billingPagination.current = 1;
  fetchBilling();
};

const fetchBilling = async () => {
  billingLoading.value = true;
  try {
    const res = await queryBillingLogs({
      page: billingPagination.current,
      pageSize: billingPagination.pageSize,
      network: billingNetwork.value || undefined,
      ...smFilter.filterParams.value,
    });
    billingLogs.value = res.data;
    billingPagination.total = res.meta?.total ?? res.total ?? 0;
  } catch { /* */ } finally { billingLoading.value = false; }
};

const onBillingPageChange = (p: number) => { billingPagination.current = p; fetchBilling(); };

const smartTypeLabel = (log: any): string => {
  const billingType = log?.type as string;
  const refId = (log?.externalRefId || '') as string;
  if (billingType === 'PaymentCredit') {
    if (refId.startsWith('session_')) return t('billing.type.sessionPayment');
    if (refId.startsWith('exception_')) return t('billing.type.exceptionResolved');
    return t('billing.type.credit');
  }
  if (billingType === 'Withdrawal') return t('billing.type.withdrawal');
  if (billingType === 'Refund') {
    if (refId.startsWith('wd_refund_')) return t('billing.type.withdrawalRefund');
    return t('billing.type.refund');
  }
  if (billingType === 'Payout') return t('billing.type.payout');
  return billingType || '—';
};

const refLink = (log: any): string | null => {
  const refId = (log?.externalRefId || '') as string;
  if (refId.startsWith('session_')) return `/session/${refId.replace('session_', '')}`;
  if (refId.startsWith('exception_')) return `/resolution`;
  return null;
};

const refDisplayText = (log: any): string => {
  const refId = (log?.externalRefId || '') as string;
  if (refId.startsWith('session_')) {
    const id = refId.replace('session_', '');
    return id.length > 16 ? id.slice(0, 16) + '…' : id;
  }
  if (refId.startsWith('exception_')) {
    const id = refId.replace('exception_', '');
    return id.length > 16 ? id.slice(0, 16) + '…' : id;
  }
  if (refId.startsWith('wd_refund_')) {
    const id = refId.replace('wd_refund_', '');
    return id.length > 16 ? id.slice(0, 16) + '…' : id;
  }
  if (refId.startsWith('wd_')) {
    const id = refId.replace('wd_', '');
    return id.length > 16 ? id.slice(0, 16) + '…' : id;
  }
  return refId;
};

const formatTime = (t: string) => formatDateTimeFull(t);

// Billing log detail dialog
const blDetailOpen = ref(false);
const blDetail = ref<BillingLogResponse | null>(null);
const openBillingDetail = async (id: string) => {
  blDetailOpen.value = true;
  blDetail.value = null;
  try {
    blDetail.value = await queryBillingLogDetail(id) as any;
  } catch (e) {
    console.error(e);
  }
};

import { useSmartPolling } from '@/composables/useSmartPolling';

useSmartPolling(async () => {
  await fetchBilling();
});
</script>
