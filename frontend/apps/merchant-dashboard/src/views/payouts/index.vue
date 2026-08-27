<template>
  <div class="space-y-6">
    <!-- Pending Approval Banner -->
    <div
      v-if="userStore.pendingApprovalCount > 0 && formModel.status !== 'PendingApproval'"
      class="flex items-center gap-3 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 dark:border-amber-800/40 dark:bg-amber-950/30 animate-fade-in-up"
    >
      <AlertCircle class="h-4 w-4 shrink-0 text-amber-600 dark:text-amber-400" />
      <div class="flex-1 min-w-0">
        <span class="text-sm font-medium text-amber-800 dark:text-amber-200">
          {{ t('payouts.pendingBanner', { count: userStore.pendingApprovalCount }) }}
        </span>
        <span class="text-xs text-amber-600/80 dark:text-amber-400/70 ml-2">
          {{ t('payouts.pendingBannerHint') }}
        </span>
      </div>
      <Button
        size="sm"
        variant="outline"
        class="shrink-0 border-amber-300 text-amber-700 hover:bg-amber-100 dark:border-amber-700 dark:text-amber-300"
        @click="formModel.status = 'PendingApproval'; search()"
      >
        {{ t('payouts.viewPending') }}
      </Button>
    </div>

    <!-- Filters -->
    <Card class="animate-fade-in-up">
      <CardContent class="pt-6">
        <div class="flex flex-col sm:flex-row sm:flex-wrap sm:items-end gap-3 sm:gap-4">
          <div class="space-y-1.5 w-full sm:w-36">
            <Label>{{ t('table.status') }}</Label>
            <Select v-model="formModel.status" @update:modelValue="search">
              <SelectTrigger>
                <SelectValue :placeholder="t('payouts.all')" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">{{ t('payouts.all') }}</SelectItem>
                <SelectItem v-for="opt in statusOptions" :key="opt" :value="opt">
                  <div class="flex items-center gap-2">
                    <span :class="['status-dot', `status-dot--${opt.toLowerCase()}`]" />
                    {{ t(`payouts.status.${opt}`) }}
                  </div>
                </SelectItem>
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
            <Label>{{ t('payouts.search') }}</Label>
            <Input
              v-model="formModel.searchText"
              :placeholder="t('payouts.searchPlaceholder')"
              @keyup.enter="search"
            />
          </div>
          <div class="flex gap-2">
            <Button variant="outline" size="sm" @click="reset">{{ t('payouts.reset') }}</Button>
            <Button variant="outline" size="sm" @click="fetchData">
              <RefreshCw class="h-3.5 w-3.5 mr-1.5" :class="{ 'animate-spin': loading }" />
              {{ t('payouts.refresh') }}
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>

    <!-- Table -->
    <Card class="animate-fade-in-up delay-1">
      <CardContent class="pt-6">
        <div class="overflow-x-auto -mx-4 md:mx-0">
        <Table class="min-w-[500px]">
          <TableHeader>
            <TableRow>
              <TableHead class="w-[100px]">{{ t('table.status') }}</TableHead>
              <TableHead class="hidden sm:table-cell w-[70px]">{{ t('table.network') }}</TableHead>
              <TableHead v-if="hasSubMerchants" class="hidden md:table-cell">{{ t('table.subMerchant') }}</TableHead>
              <TableHead>{{ t('payouts.amount') }}</TableHead>
              <TableHead class="hidden md:table-cell">{{ t('payouts.fee') }}</TableHead>
              <TableHead class="hidden md:table-cell">{{ t('payouts.netAmount') }}</TableHead>
              <TableHead class="hidden lg:table-cell">{{ t('payouts.toAddress') }}</TableHead>
              <TableHead class="hidden lg:table-cell">{{ t('payouts.txHash') }}</TableHead>
              <TableHead>{{ t('table.created') }}</TableHead>
              <TableHead v-if="canApprove" class="w-[100px]">{{ t('table.actions') }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <template v-if="loading">
              <TableRow>
                <TableCell colspan="8" class="text-center py-12 text-muted-foreground">
                  <div class="flex items-center justify-center gap-2">
                    <RefreshCw class="h-4 w-4 animate-spin" />
                    {{ t('payouts.loading') }}
                  </div>
                </TableCell>
              </TableRow>
            </template>
            <template v-else-if="renderData.length === 0">
              <TableRow>
                <TableCell colspan="8" class="text-center py-12 text-muted-foreground">
                  <div class="flex flex-col items-center gap-2">
                    <Inbox class="h-8 w-8 text-muted-foreground/30" />
                    <span>{{ t('payouts.noPayouts') }}</span>
                  </div>
                </TableCell>
              </TableRow>
            </template>
            <template v-else>
              <TableRow
                v-for="row in renderData"
                :key="row.id"
                class="cursor-pointer transition-colors hover:bg-muted/50"
                @click="$router.push({ name: 'PayoutDetail', params: { id: row.id } })"
              >
                <TableCell>
                  <Badge :variant="getVariant(row.status)">
                    {{ t(`payouts.status.${row.status}`) }}
                  </Badge>
                </TableCell>
                <TableCell class="hidden sm:table-cell">
                  <Badge variant="outline" class="text-[10px] px-1.5 py-0">{{ networkDisplayName(row.network, envStore.isSandbox) }}</Badge>
                </TableCell>
                <TableCell v-if="hasSubMerchants" class="hidden md:table-cell">
                  <Badge v-if="row.sub_merchant_code" variant="outline" :class="['text-[10px] px-1.5 py-0', smColorClass(row.sub_merchant_code)]">
                    {{ row.sub_merchant_code }}
                  </Badge>
                  <span v-else class="text-muted-foreground">—</span>
                </TableCell>
                <TableCell>
                  <span class="font-semibold tabular-nums">{{ formatUsdt(row.amount) }}</span>
                  <span class="text-xs text-muted-foreground ml-1">{{ row.currency }}</span>
                </TableCell>
                <TableCell class="hidden md:table-cell text-sm text-muted-foreground tabular-nums">
                  {{ formatUsdt(row.fee) }}
                </TableCell>
                <TableCell class="hidden md:table-cell text-sm tabular-nums">
                  {{ formatUsdt(row.net_amount) }}
                </TableCell>
                <TableCell class="hidden lg:table-cell">
                  <span class="font-mono text-xs text-muted-foreground">
                    {{ row.to_address.slice(0, 6) }}…{{ row.to_address.slice(-4) }}
                  </span>
                </TableCell>
                <TableCell class="hidden lg:table-cell">
                  <a
                    v-if="row.tx_hash"
                    :href="txUrl(row.tx_hash, row.network)"
                    target="_blank"
                    class="font-mono text-xs text-brand hover:underline"
                    @click.stop
                  >
                    {{ row.tx_hash.slice(0, 8) }}…
                  </a>
                  <span v-else class="text-muted-foreground">—</span>
                </TableCell>
                <TableCell class="text-sm text-muted-foreground">
                  {{ formatDateTime(row.created_at) }}
                </TableCell>
                <TableCell v-if="canApprove">
                  <div v-if="row.status === 'PendingApproval'" class="flex gap-1">
                    <Button variant="outline" size="sm" class="h-7 text-xs" @click.stop="openApproval(row.id, 'approve')">
                      {{ t('approval.approve') }}
                    </Button>
                    <Button variant="ghost" size="sm" class="h-7 text-xs text-destructive" @click.stop="openApproval(row.id, 'reject')">
                      {{ t('approval.reject') }}
                    </Button>
                  </div>
                  <span v-else class="text-muted-foreground">—</span>
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
            {{ t('payouts.results', { count: pagination.total }) }}
          </span>
          <div class="flex gap-1">
            <Button
              variant="outline"
              size="sm"
              :disabled="pagination.current <= 1"
              @click="onPageChange(pagination.current - 1)"
            >
              {{ t('payouts.previous') }}
            </Button>
            <Button
              variant="outline"
              size="sm"
              :disabled="pagination.current * pagination.pageSize >= pagination.total"
              @click="onPageChange(pagination.current + 1)"
            >
              {{ t('payouts.next') }}
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  </div>

  <!-- Approval Dialog -->
  <ApprovalDialog
    v-model:open="approvalOpen"
    :action="approvalAction"
    :target-id="approvalTargetId"
    target-type="payout"
    @done="fetchData"
  />
</template>

<script lang="ts" setup>
import { ref, reactive, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { RefreshCw, Inbox, AlertCircle } from 'lucide-vue-next';
import useLoading from '@/hooks/loading';
import { fmtAmt, formatUsdt } from '@/utils/currency';
import { formatDateTime } from '@/utils/date';
import { networkDisplayName } from '@/utils/networkUtils';
import { txUrl } from '@/utils/explorer';
import { useEnvironmentStore } from '@/stores/environment';
import { useUserStore } from '@/stores';
import { http } from '@/utils/request';
import ApprovalDialog from '@/components/ApprovalDialog.vue';
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

interface PayoutRecord {
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
  sub_merchant_code?: string;
}

const { t } = useI18n();
const { loading, setLoading } = useLoading(true);
const envStore = useEnvironmentStore();
const smFilter = useSubMerchantFilter();
const smSelected = smFilter.selected;
const smList = smFilter.subMerchants;
const hasSubMerchants = smFilter.hasSubMerchants;

smFilter.loadSubMerchants();

const formModel = reactive({ searchText: '', status: 'all' });

// Initialize status filter from query params (e.g. from Dashboard "Review Now" button)
import { useRoute } from 'vue-router';
const route = useRoute();
if (route.query.status && typeof route.query.status === 'string') {
  formModel.status = route.query.status;
}
const statusOptions = ['Pending', 'PendingApproval', 'Processing', 'Completed', 'Failed', 'Cancelled', 'ApprovalExpired'];
const renderData = ref<PayoutRecord[]>([]);
const pagination = reactive({ current: 1, pageSize: 20, total: 0 });

const userStore = useUserStore();
const canApprove = computed(() => ['owner', 'admin'].includes(userStore.orgRole || ''));

// Approval dialog state
const approvalOpen = ref(false);
const approvalAction = ref<'approve' | 'reject'>('approve');
const approvalTargetId = ref('');
const openApproval = (id: string, action: 'approve' | 'reject') => {
  approvalTargetId.value = id;
  approvalAction.value = action;
  approvalOpen.value = true;
};

const getVariant = (status: string) => {
  const map: Record<string, 'default' | 'secondary' | 'destructive' | 'outline'> = {
    Completed: 'default',
    Pending: 'secondary',
    PendingApproval: 'outline',
    Processing: 'secondary',
    Failed: 'destructive',
    Cancelled: 'destructive',
    ApprovalExpired: 'outline',
  };
  return map[status] || 'secondary';
};





const fetchData = async (silent = false) => {
  if (!silent) setLoading(true);
  try {
    const params = new URLSearchParams();
    params.set('page', String(pagination.current));
    params.set('page_size', String(pagination.pageSize));
    if (formModel.searchText) params.set('search_text', formModel.searchText);
    if (formModel.status !== 'all') params.set('status', formModel.status);
    // Sub-merchant filter
    const smParams = smFilter.filterParams.value;
    if (smParams.include_sub_merchants) params.set('include_sub_merchants', 'true');
    if (smParams.sub_merchant_code) params.set('sub_merchant_code', smParams.sub_merchant_code);

    const res = await http.get<{ data: PayoutRecord[]; meta: { total: number } }>(
      `/api/internal/merchants/payouts?${params.toString()}`
    );
    renderData.value = res.data;
    pagination.total = res.meta.total;
  } catch {
    // silent
  } finally {
    setLoading(false);
  }
};

const search = () => { pagination.current = 1; fetchData(); };
const reset = () => { formModel.searchText = ''; formModel.status = 'all'; smFilter.resetFilter(); search(); };
const onPageChange = (p: number) => { pagination.current = p; fetchData(); };

import { useSmartPolling } from '@/composables/useSmartPolling';

useSmartPolling(() => fetchData(true));
</script>
