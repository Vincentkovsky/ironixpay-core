<template>
  <div class="space-y-6">
    <!-- Filters -->
    <Card class="animate-fade-in-up">
      <CardContent class="pt-6">
        <!-- Row 1: Filters -->
        <div class="flex flex-col sm:flex-row sm:flex-wrap sm:items-end gap-3 sm:gap-4">
          <div class="space-y-1.5 w-full sm:w-40">
            <Label>{{ t('table.status') }}</Label>
            <Select v-model="formModel.status" @update:modelValue="search">
              <SelectTrigger>
                <SelectValue :placeholder="t('sessions.all')" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">{{ t('sessions.all') }}</SelectItem>
                <SelectItem v-for="opt in statusOptions" :key="opt" :value="opt">
                  <div class="flex items-center gap-2">
                    <span :class="['status-dot', `status-dot--${opt.toLowerCase()}`]" />
                    {{ t(`sessions.status.${opt}`) }}
                  </div>
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div class="space-y-1.5 w-full sm:w-32">
            <Label>{{ t('table.network') }}</Label>
            <Select v-model="formModel.network" @update:modelValue="search">
              <SelectTrigger>
                <SelectValue :placeholder="t('sessions.all')" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">{{ t('sessions.all') }}</SelectItem>
                <SelectItem v-for="net in networkOptions" :key="net" :value="net">{{ net }}</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div v-if="hasSubMerchants" class="space-y-1.5 w-full sm:w-48">
            <Label>{{ t('subMerchantFilter.label') }}</Label>
            <Select v-model="smSelected" @update:modelValue="search">
              <SelectTrigger class="w-full min-w-0 [&>span]:truncate">
                <SelectValue :placeholder="t('subMerchantFilter.all')" />
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
          <div class="flex-1 min-w-0 sm:min-w-[200px] sm:max-w-sm space-y-1.5">
            <Label>{{ t('sessions.search') }}</Label>
            <Input
              v-model="formModel.id"
              :placeholder="t('sessions.searchPlaceholder')"
              @keyup.enter="search"
            />
          </div>
          <div class="flex gap-2">
            <Button variant="outline" size="sm" @click="reset">{{ t('sessions.reset') }}</Button>
            <Button variant="outline" size="sm" @click="fetchData">
              <RefreshCw class="h-3.5 w-3.5 mr-1.5" :class="{ 'animate-spin': loading }" />
              {{ t('sessions.refresh') }}
            </Button>
          </div>
        </div>
        <!-- Row 2: Export -->
        <div class="flex flex-wrap items-end gap-2 pt-3 mt-3 border-t">
          <div class="flex flex-col gap-1">
            <Label class="text-xs text-muted-foreground">{{ t('billing.startDate') }}</Label>
            <Input type="date" v-model="exportStartDate" class="w-36 h-8 text-sm" />
          </div>
          <div class="flex flex-col gap-1">
            <Label class="text-xs text-muted-foreground">{{ t('billing.endDate') }}</Label>
            <Input type="date" v-model="exportEndDate" class="w-36 h-8 text-sm" />
          </div>
          <Button variant="outline" size="sm" class="h-8" :disabled="exporting" @click="exportPayments">
            <Download class="h-3.5 w-3.5 mr-1.5" />
            {{ exporting ? t('sessions.exporting') : t('sessions.exportCsv') }}
          </Button>
        </div>
      </CardContent>
    </Card>

    <!-- Table -->
    <Card class="animate-fade-in-up delay-1">
      <CardContent class="pt-6">
        <div class="overflow-x-auto -mx-4 md:mx-0">
        <Table class="min-w-[360px]">
          <TableHeader>
            <TableRow>
              <TableHead class="w-[100px] md:w-[140px]">{{ t('table.status') }}</TableHead>
              <TableHead class="hidden sm:table-cell w-[70px]">{{ t('table.network') }}</TableHead>
              <TableHead>{{ t('table.amount') }}</TableHead>
              <TableHead class="hidden md:table-cell">{{ t('table.refId') }}</TableHead>
              <TableHead class="hidden lg:table-cell">{{ t('table.created') }}</TableHead>
              <TableHead v-if="hasSubMerchants" class="hidden md:table-cell">{{ t('table.subMerchant') }}</TableHead>
              <TableHead class="hidden md:table-cell">{{ t('table.sessionId') }}</TableHead>
              <TableHead class="text-right">{{ t('table.actions') }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <template v-if="loading">
              <TableRow>
                <TableCell colspan="7" class="text-center py-12 text-muted-foreground">
                  <div class="flex items-center justify-center gap-2">
                    <RefreshCw class="h-4 w-4 animate-spin" />
                    {{ t('sessions.loading') }}
                  </div>
                </TableCell>
              </TableRow>
            </template>
            <template v-else-if="renderData.length === 0">
              <TableRow>
                <TableCell colspan="7" class="text-center py-12 text-muted-foreground">
                  <div class="flex flex-col items-center gap-2">
                    <Inbox class="h-8 w-8 text-muted-foreground/30" />
                    <span>{{ t('sessions.noSessions') }}</span>
                  </div>
                </TableCell>
              </TableRow>
            </template>
            <template v-else>
              <TableRow
                v-for="row in renderData"
                :key="row.id"
                class="cursor-pointer transition-colors hover:bg-muted/50"
                @click="$router.push({ name: 'SessionDetail', params: { id: row.id } })"
              >
                <TableCell>
                  <div class="flex items-center gap-2">
                    <span :class="['status-dot', `status-dot--${row.status.toLowerCase()}`]" />
                    <Badge :variant="getVariant(row.status)">
                      {{ t(`sessions.status.${row.status}`) }}
                    </Badge>
                  </div>
                </TableCell>
                <TableCell class="hidden sm:table-cell">
                  <Badge variant="outline" class="text-[10px] px-1.5 py-0">{{ networkDisplayName(row.network, envStore.isSandbox) }}</Badge>
                </TableCell>
                <TableCell>
                  <span class="font-semibold tabular-nums">
                    {{ fmtAmt(row.amount) }}
                  </span>
                  <span class="text-xs text-muted-foreground ml-1">{{ row.currency }}</span>
                  <div
                    v-if="row.pricing && row.pricing.currency !== row.currency"
                    class="text-xs text-muted-foreground/70 mt-0.5"
                  >
                    ≈ {{ row.pricing.amount }} {{ row.pricing.currency }}
                  </div>
                  <div
                    v-if="row.amountReceived !== row.amount"
                    class="text-xs text-orange-500 mt-0.5"
                  >
                    {{ t('sessions.received') }}: {{ fmtAmt(row.amountReceived) }}
                  </div>
                </TableCell>
                <TableCell class="hidden md:table-cell">
                  <span v-if="row.clientReferenceId" class="font-mono text-xs">
                    {{ row.clientReferenceId }}
                  </span>
                  <span v-else class="text-muted-foreground">—</span>
                </TableCell>
                <TableCell class="hidden lg:table-cell text-sm text-muted-foreground">
                  {{ formatDateTime(row.createdTime) }}
                </TableCell>
                <TableCell v-if="hasSubMerchants" class="hidden md:table-cell">
                  <Badge v-if="row.sub_merchant_code" variant="outline" :class="['text-[10px] px-1.5 py-0', smColorClass(row.sub_merchant_code)]">
                    {{ row.sub_merchant_code }}
                  </Badge>
                  <span v-else class="text-muted-foreground">—</span>
                </TableCell>
                <TableCell class="hidden md:table-cell">
                  <span class="font-mono text-xs text-muted-foreground">
                    …{{ row.id.slice(-6) }}
                  </span>
                </TableCell>
                <TableCell class="text-right" @click.stop>
                  <Button variant="ghost" size="sm" class="text-muted-foreground hover:text-foreground" @click="$router.push({ name: 'SessionDetail', params: { id: row.id } })">
                    {{ t('sessions.detail') }}
                  </Button>
                </TableCell>
              </TableRow>
            </template>
          </TableBody>
        </Table>
        </div>

        <!-- Pagination -->
        <div
          v-if="pagination.total > pagination.pageSize"
          class="flex flex-col sm:flex-row items-center justify-between gap-3 pt-4 border-t mt-4"
        >
          <span class="text-sm text-muted-foreground">
            {{ t('sessions.results', { count: pagination.total }) }}
          </span>
          <div class="flex gap-1">
            <Button
              variant="outline"
              size="sm"
              :disabled="pagination.current <= 1"
              @click="onPageChange(pagination.current - 1)"
            >
              {{ t('sessions.previous') }}
            </Button>
            <Button
              variant="outline"
              size="sm"
              :disabled="pagination.current * pagination.pageSize >= pagination.total"
              @click="onPageChange(pagination.current + 1)"
            >
              {{ t('sessions.next') }}
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  </div>
</template>

<script lang="ts" setup>
import { ref, reactive } from 'vue';
import { useI18n } from 'vue-i18n';
import { RefreshCw, Inbox, Download } from 'lucide-vue-next';
import { toast } from 'vue-sonner';
import { networkDisplayName } from '@/utils/networkUtils';
import { useEnvironmentStore } from '@/stores/environment';
import useLoading from '@/hooks/loading';
import { formatDateTime } from '@/utils/date';
import { fmtAmt } from '@/utils/currency';
import { http } from '@/utils/request';
import { querySessionList, type SessionRecord, type SessionParams } from '@/api/session';
import { useSubMerchantFilter, smColorClass } from '@/composables/useSubMerchantFilter';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select';
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from '@/components/ui/table';

const { t } = useI18n();
const { loading, setLoading } = useLoading(true);
const envStore = useEnvironmentStore();
const smFilter = useSubMerchantFilter();
const smSelected = smFilter.selected;
const smList = smFilter.subMerchants;
const hasSubMerchants = smFilter.hasSubMerchants;

smFilter.loadSubMerchants();

const formModel = reactive({ id: '', status: 'all', network: 'all' });
const statusOptions = ['Pending', 'Paid', 'Underpaid', 'Overpaid', 'Expired', 'Blocked'];
const networkOptions = ['TRON', 'BSC', 'ETHEREUM', 'POLYGON', 'ARBITRUM', 'BASE', 'OPTIMISM', 'SOLANA'];
const renderData = ref<SessionRecord[]>([]);
const pagination = reactive({ current: 1, pageSize: 10, total: 0 });

const getVariant = (status: string) => {
  const map: Record<string, 'default' | 'secondary' | 'destructive' | 'outline'> = {
    Paid: 'default',
    Pending: 'secondary',
    Expired: 'outline',
    Underpaid: 'destructive',
    Overpaid: 'default',
    Blocked: 'destructive',
  };
  return map[status] || 'secondary';
};

const fetchData = async (silent = false) => {
  if (!silent) setLoading(true);
  try {
    const params: SessionParams = {
      current: pagination.current,
      pageSize: pagination.pageSize,
      id: formModel.id,
      ...(formModel.status !== 'all' && { status: formModel.status }),
      ...(formModel.network !== 'all' && { network: formModel.network }),
      ...smFilter.filterParams.value,
    };
    const data = await querySessionList(params);
    renderData.value = data.list;
    pagination.total = data.total;
  } catch {
    // silent
  } finally {
    setLoading(false);
  }
};

const search = () => { pagination.current = 1; fetchData(); };
const reset = () => { formModel.id = ''; formModel.status = 'all'; formModel.network = 'all'; smFilter.resetFilter(); search(); };
const onPageChange = (p: number) => { pagination.current = p; fetchData(); };

// CSV Export
const exportStartDate = ref('');
const exportEndDate = ref('');
const exporting = ref(false);

const exportPayments = async () => {
  exporting.value = true;
  try {
    const params = new URLSearchParams();
    if (exportStartDate.value) params.set('start_date', exportStartDate.value);
    if (exportEndDate.value) params.set('end_date', exportEndDate.value);
    if (formModel.status !== 'all') params.set('status', formModel.status);
    const smParams = smFilter.filterParams.value;
    if (smParams.include_sub_merchants) params.set('include_sub_merchants', 'true');
    if (smParams.sub_merchant_code) params.set('sub_merchant_code', smParams.sub_merchant_code);

    const blob = await http.get<Blob>(`/api/internal/billing/payments/export?${params.toString()}`, {
      responseType: 'blob',
      skipErrorToast: true,
    });

    const parts = ['payments'];
    if (exportStartDate.value) parts.push(exportStartDate.value);
    if (exportEndDate.value) parts.push(exportEndDate.value);
    const filename = parts.length > 1 ? `${parts.join('_')}.csv` : 'payments_export.csv';

    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);

    toast.success(t('sessions.exportCsv') + ' ✓');
  } catch {
    toast.error(t('sessions.exportEmpty'));
  } finally {
    exporting.value = false;
  }
};

import { useSmartPolling } from '@/composables/useSmartPolling';

useSmartPolling(() => fetchData(true));
</script>
