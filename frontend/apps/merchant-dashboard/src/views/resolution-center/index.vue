<template>
  <div class="space-y-6">
    <!-- Stats -->
    <div class="grid gap-5 sm:grid-cols-2">
      <div class="res-hero-card res-hero-card--amber animate-fade-in-up">
        <div class="res-hero-card__content">
          <div class="res-hero-icon res-hero-icon--amber">
            <AlertTriangle class="h-5 w-5" />
          </div>
          <div>
            <p class="res-hero-label">{{ t('resolution.unresolved') }}</p>
            <p class="res-hero-value">{{ resStats.unresolved_count || 0 }}</p>
          </div>
        </div>
        <div class="res-hero-card__glow" />
      </div>
      <div class="res-hero-card res-hero-card--blue animate-fade-in-up delay-1">
        <div class="res-hero-card__content">
          <div class="res-hero-icon res-hero-icon--blue">
            <DollarSign class="h-5 w-5" />
          </div>
          <div>
            <p class="res-hero-label">{{ t('resolution.pendingValue') }}</p>
            <p class="res-hero-value">${{ formatUsdt(resStats.unresolved_value) }}</p>
          </div>
        </div>
        <div class="res-hero-card__glow" />
      </div>
    </div>

    <!-- Filters + Table -->
    <Card class="animate-fade-in-up delay-3">
      <CardHeader>
        <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <CardTitle>{{ t('resolution.exceptions') }}</CardTitle>
          <div class="flex gap-2">
            <div v-if="hasSubMerchants">
              <Select v-model="smSelected" @update:modelValue="search">
                <SelectTrigger class="w-28 sm:w-32 min-w-0 [&>span]:truncate"><SelectValue :placeholder="t('subMerchantFilter.label')" /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="_all">{{ t('subMerchantFilter.all') }}</SelectItem>
                  <SelectItem value="_self">{{ t('subMerchantFilter.self') }}</SelectItem>
                  <SelectItem v-for="sm in smList" :key="sm.sub_merchant_code" :value="sm.sub_merchant_code">
                    {{ sm.display_name || sm.sub_merchant_code }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
            <Select v-model="filterStatus" @update:modelValue="search">
              <SelectTrigger class="w-28 sm:w-32"><SelectValue :placeholder="t('table.status')" /></SelectTrigger>
              <SelectContent>
                <SelectItem value="all">{{ t('sessions.all') }}</SelectItem>
                <SelectItem value="Pending">{{ t('resolution.status.Pending') }}</SelectItem>
                <SelectItem value="Processing">{{ t('resolution.status.Processing') }}</SelectItem>
                <SelectItem value="Resolved">{{ t('resolution.status.Resolved') }}</SelectItem>
                <SelectItem value="Failed">{{ t('resolution.status.Failed') }}</SelectItem>
              </SelectContent>
            </Select>
            <Button variant="outline" size="sm" @click="fetchData">
              <RefreshCw class="h-3.5 w-3.5 mr-1.5" :class="{ 'animate-spin': loading }" /> {{ t('sessions.refresh') }}
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <div class="overflow-x-auto -mx-4 md:mx-0">
        <Table class="min-w-[360px]">
          <TableHeader>
            <TableRow>
              <TableHead>{{ t('table.type') }}</TableHead>
              <TableHead>{{ t('table.amount') }}</TableHead>
              <TableHead class="hidden md:table-cell">{{ t('table.sender') }}</TableHead>
              <TableHead>{{ t('table.status') }}</TableHead>
              <TableHead v-if="hasSubMerchants" class="hidden md:table-cell">{{ t('table.subMerchant') }}</TableHead>
              <TableHead class="hidden lg:table-cell">{{ t('table.time') }}</TableHead>
              <TableHead class="text-right">{{ t('table.actions') }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="row in exceptions" :key="row.id" class="cursor-pointer hover:bg-muted/50 transition-colors" @click="openReview(row)">
              <TableCell>
                <Badge variant="outline" class="text-xs">{{ t(`resolution.type.${row.exception_type}`) }}</Badge>
              </TableCell>
              <TableCell class="font-medium tabular-nums">
                {{ formatUsdt(row.amount) }} {{ row.currency }}
              </TableCell>
              <TableCell class="hidden md:table-cell font-mono text-xs text-muted-foreground">
                {{ row.sender.slice(0, 10) }}…
              </TableCell>
              <TableCell>
                <div class="flex items-center gap-2">
                  <span :class="['status-dot', `status-dot--${row.status.toLowerCase()}`]" />
                  <Badge :variant="getStatusVariant(row.status)">{{ t(`resolution.status.${row.status}`) }}</Badge>
                </div>
              </TableCell>
              <TableCell v-if="hasSubMerchants" class="hidden md:table-cell">
                <Badge v-if="row.sub_merchant_code" variant="outline" :class="['text-[10px] px-1.5 py-0', smColorClass(row.sub_merchant_code)]">
                  {{ row.sub_merchant_code }}
                </Badge>
                <span v-else class="text-muted-foreground">—</span>
              </TableCell>
              <TableCell class="hidden lg:table-cell text-sm text-muted-foreground">
                {{ formatDateTime(row.created_at) }}
              </TableCell>
              <TableCell class="text-right">
                <Button variant="ghost" size="sm">
                  <Eye class="h-3.5 w-3.5" />
                </Button>
              </TableCell>
            </TableRow>
            <TableRow v-if="exceptions.length === 0">
              <TableCell colspan="8" class="text-center text-muted-foreground py-12">
                <div class="flex flex-col items-center gap-2">
                  <Shield class="h-8 w-8 text-muted-foreground/30" />
                  <span>{{ t('resolution.noExceptions') }}</span>
                </div>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
        </div>
      </CardContent>
    </Card>

    <!-- Review Sheet -->
    <Sheet v-model:open="sheetOpen">
      <SheetContent class="w-full sm:max-w-lg overflow-y-auto px-4 sm:px-6">
        <SheetHeader>
          <SheetTitle>{{ t('resolution.reviewTitle') }}</SheetTitle>
          <SheetDescription>{{ t('resolution.reviewDesc') }}</SheetDescription>
        </SheetHeader>

        <template v-if="selectedRow">
          <!-- Compact Header: Amount + Type + Status -->
          <div class="mt-4 rounded-lg bg-muted/50 border px-4 py-3">
            <div class="flex items-center justify-between">
              <p class="text-xl font-bold tabular-nums tracking-tight">{{ formatUsdt(selectedRow.amount) }} <span class="text-sm font-medium text-muted-foreground">{{ selectedRow.currency }}</span></p>
              <Badge :variant="getStatusVariant(selectedRow.status)">{{ t(`resolution.status.${selectedRow.status}`) }}</Badge>
            </div>
            <div class="flex items-center gap-2 mt-1.5">
              <Badge variant="outline" class="text-[11px] px-1.5 py-0">{{ t(`resolution.type.${selectedRow.exception_type}`) }}</Badge>
              <Badge v-if="selectedRow.sub_merchant_code" variant="outline" :class="['text-[10px] px-1.5 py-0', smColorClass(selectedRow.sub_merchant_code)]">
                {{ selectedRow.sub_merchant_code }}
              </Badge>
              <span class="text-[11px] text-muted-foreground">{{ formatDateTime(selectedRow.created_at) }}</span>
            </div>
            <p class="text-[11px] text-muted-foreground mt-1.5 leading-relaxed">{{ t(`resolution.typeHint.${selectedRow.exception_type}`) }}</p>
            <!-- WrongToken: show received vs expected token -->
            <div v-if="selectedRow.exception_type === 'WrongToken'" class="mt-2 flex items-center gap-2 rounded-md bg-amber-500/10 border border-amber-500/20 px-3 py-1.5">
              <AlertTriangle class="h-3.5 w-3.5 text-amber-600 shrink-0" />
              <p class="text-[11px] font-medium text-amber-700 dark:text-amber-400">
                {{ t('resolution.wrongTokenDetail', { received: selectedRow.currency, expected: selectedRow.currency === 'USDC' ? 'USDT' : 'USDC' }) }}
              </p>
            </div>
          </div>

          <!-- Details: single card with dividers -->
          <div class="mt-3 rounded-lg border divide-y">
            <div class="flex items-center justify-between gap-2 px-4 py-2.5">
              <div class="min-w-0 flex-1">
                <p class="text-[11px] font-medium text-muted-foreground">{{ t('resolution.network') }}</p>
              </div>
              <Badge variant="outline" class="text-xs">{{ networkDisplayName(selectedRow.network, envStore.isSandbox) }}</Badge>
            </div>
            <div class="flex items-center justify-between gap-2 px-4 py-2.5">
              <div class="min-w-0 flex-1">
                <p class="text-[11px] font-medium text-muted-foreground">{{ t('resolution.sender') }}</p>
                <code class="text-xs truncate block">{{ truncateAddr(selectedRow.sender) }}</code>
              </div>
              <div class="flex items-center gap-1">
                <a :href="addressUrl(selectedRow.sender, selectedRow.network)" target="_blank" rel="noopener"
                   class="inline-flex items-center justify-center h-7 w-7 rounded-md hover:bg-muted transition-colors"
                   :title="t('resolution.viewOnExplorer')">
                  <ExternalLink class="h-3.5 w-3.5 text-muted-foreground" />
                </a>
                <CopyButton :value="selectedRow.sender" class="shrink-0" />
              </div>
            </div>
            <div v-if="selectedRow.tx_hash" class="flex items-center justify-between gap-2 px-4 py-2.5">
              <div class="min-w-0 flex-1">
                <p class="text-[11px] font-medium text-muted-foreground">{{ t('resolution.txHash') }}</p>
                <code class="text-xs truncate block">{{ truncateAddr(selectedRow.tx_hash, 16, 8) }}</code>
              </div>
              <div class="flex items-center gap-1">
                <a :href="txUrl(selectedRow.tx_hash, selectedRow.network)" target="_blank" rel="noopener"
                   class="inline-flex items-center justify-center h-7 w-7 rounded-md hover:bg-muted transition-colors"
                   :title="t('resolution.viewOnExplorer')">
                  <ExternalLink class="h-3.5 w-3.5 text-muted-foreground" />
                </a>
                <CopyButton :value="selectedRow.tx_hash" class="shrink-0" />
              </div>
            </div>
            <div v-if="selectedRow.session_id" class="flex items-center justify-between gap-2 px-4 py-2.5">
              <div class="min-w-0 flex-1">
                <p class="text-[11px] font-medium text-muted-foreground">{{ t('resolution.relatedSession') }}</p>
                <code class="text-xs truncate block">{{ selectedRow.session_id }}</code>
              </div>
              <div class="flex items-center gap-1">
                <router-link :to="{ name: 'SessionDetail', params: { id: selectedRow.session_id } }"
                   class="inline-flex items-center justify-center h-7 w-7 rounded-md hover:bg-muted transition-colors"
                   :title="t('resolution.goToSession')">
                  <ArrowRight class="h-3.5 w-3.5 text-muted-foreground" />
                </router-link>
                <CopyButton :value="selectedRow.session_id" class="shrink-0" />
              </div>
            </div>
          </div>

          <!-- Actions -->
          <div v-if="selectedRow.available_actions.length > 0" class="mt-4">
            <p class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground mb-2">{{ t('resolution.availableActions') }}</p>
            <div class="space-y-1.5">

              <!-- Accept -->
              <button
                v-if="selectedRow.available_actions.includes('accept')"
                class="sheet-action-btn group"
                :disabled="actionLoading"
                @click="handleAccept"
              >
                <div class="sheet-action-icon bg-emerald-500/10">
                  <Check class="h-3.5 w-3.5 text-emerald-600" />
                </div>
                <div class="flex-1 text-left">
                  <p class="text-sm font-medium leading-tight">{{ t('resolution.action.accept') }}</p>
                  <p class="text-[11px] text-muted-foreground">{{ t('resolution.acceptHint') }}</p>
                </div>
              </button>

              <!-- Attach -->
              <div v-if="selectedRow.available_actions.includes('attach')">
                <button
                  v-if="!showAttachForm"
                  class="sheet-action-btn group"
                  @click="showAttachForm = true"
                >
                  <div class="sheet-action-icon bg-blue-500/10">
                    <Link class="h-3.5 w-3.5 text-blue-600" />
                  </div>
                  <div class="flex-1 text-left">
                    <p class="text-sm font-medium leading-tight">{{ t('resolution.action.attach') }}</p>
                    <p class="text-[11px] text-muted-foreground">{{ t('resolution.attachHint') }}</p>
                  </div>
                </button>
                <div v-else class="rounded-lg border bg-muted/30 p-3 space-y-2">
                  <div class="space-y-1">
                    <Label class="text-xs">{{ t('resolution.sessionId') }}</Label>
                    <Input v-model="attachSessionId" :placeholder="t('resolution.sessionIdPlaceholder')" class="font-mono text-xs h-8" />
                  </div>
                  <div class="flex gap-2 justify-end">
                    <Button size="sm" variant="ghost" @click="showAttachForm = false">{{ t('resolution.cancel') }}</Button>
                    <Button size="sm" :disabled="actionLoading || !attachSessionId.trim()" @click="submitAttach">
                      {{ t('resolution.attach') }}
                    </Button>
                  </div>
                </div>
              </div>

              <!-- Transfer -->
              <div v-if="selectedRow.available_actions.includes('transfer')">
                <button
                  v-if="!showTransferForm"
                  class="sheet-action-btn sheet-action-btn--danger group"
                  @click="showTransferForm = true"
                >
                  <div class="sheet-action-icon bg-red-500/10">
                    <ArrowUpRight class="h-3.5 w-3.5 text-red-600" />
                  </div>
                  <div class="flex-1 text-left">
                    <p class="text-sm font-medium leading-tight">{{ t('resolution.action.transfer') }}</p>
                    <p class="text-[11px] text-muted-foreground">{{ t('resolution.transferHint') }}</p>
                  </div>
                </button>
                <div v-else class="rounded-lg border border-destructive/20 bg-destructive/5 p-3 space-y-2">
                  <div class="flex items-center gap-1.5 text-[11px] text-destructive font-medium">
                    <AlertTriangle class="h-3 w-3" />
                    {{ t('resolution.transferHint') }}
                  </div>
                  <div class="space-y-1">
                    <Label class="text-xs">{{ t('resolution.toAddress') }}</Label>
                    <Input v-model="transferAddress" :placeholder="t('resolution.toAddressPlaceholder')" class="font-mono text-xs h-8" />
                  </div>
                  <div class="space-y-1">
                    <Label class="text-xs">{{ t('resolution.totpCode') }}</Label>
                    <Input v-model="transferCode" type="text" inputmode="numeric" maxlength="6" placeholder="000000" class="font-mono tracking-[0.3em] h-8" />
                  </div>
                  <!-- Fee Breakdown -->
                  <div class="rounded-md border border-border/60 bg-muted/30 px-3 py-2 space-y-1">
                    <div class="flex items-center justify-between text-xs">
                      <span class="text-muted-foreground">{{ t('resolution.feeBreakdown.gross') }}</span>
                      <span class="font-medium tabular-nums">{{ formatUsdt(selectedRow!.amount) }} {{ selectedRow!.currency }}</span>
                    </div>
                    <div class="flex items-center justify-between text-xs">
                      <span class="text-muted-foreground">{{ t('resolution.feeBreakdown.fee') }}</span>
                      <span class="tabular-nums" :class="refundFee === '0.00' ? 'text-emerald-600' : 'text-amber-600'">
                        -{{ refundFee }} {{ selectedRow!.currency }}
                      </span>
                    </div>
                    <div class="border-t border-border/40 pt-1 flex items-center justify-between text-xs">
                      <span class="font-medium">{{ t('resolution.feeBreakdown.net') }}</span>
                      <span class="font-semibold tabular-nums">{{ refundNet }} {{ selectedRow!.currency }}</span>
                    </div>
                    <p v-if="isAmlException" class="text-[10px] text-emerald-600 mt-0.5">{{ t('resolution.feeBreakdown.amlNote') }}</p>
                  </div>
                  <div class="flex gap-2 justify-end">
                    <Button size="sm" variant="ghost" @click="showTransferForm = false">{{ t('resolution.cancel') }}</Button>
                    <Button size="sm" variant="destructive" :disabled="actionLoading || !transferAddress.trim()" @click="submitTransfer">
                      {{ t('resolution.transfer') }}
                    </Button>
                  </div>
                </div>
              </div>

            </div>
          </div>

          <div v-else class="mt-4 space-y-3">
            <div class="text-sm text-muted-foreground text-center py-3 rounded-lg border border-dashed">
              {{ t('resolution.noActions') }}
            </div>
            <!-- Resolution details: tx hash + to_address -->
            <div v-if="selectedRow.resolution_tx_hash" class="rounded-lg border bg-muted/30 p-3 space-y-2 text-sm">
              <div class="flex items-center justify-between">
                <span class="text-muted-foreground text-xs">{{ t('resolution.resolutionTxHash') }}</span>
                <div class="flex items-center gap-1.5">
                  <span class="font-mono text-xs">{{ selectedRow.resolution_tx_hash.slice(0, 12) }}…{{ selectedRow.resolution_tx_hash.slice(-6) }}</span>
                  <a :href="txUrl(selectedRow.resolution_tx_hash, selectedRow.network)" target="_blank" rel="noopener" class="text-muted-foreground hover:text-foreground">
                    <ExternalLink class="h-3 w-3" />
                  </a>
                  <CopyButton :value="selectedRow.resolution_tx_hash" show-toast />
                </div>
              </div>
              <div v-if="selectedRow.resolution_to_address" class="flex items-center justify-between">
                <span class="text-muted-foreground text-xs">{{ t('resolution.resolutionToAddress') }}</span>
                <div class="flex items-center gap-1.5">
                  <span class="font-mono text-xs">{{ selectedRow.resolution_to_address.slice(0, 8) }}…{{ selectedRow.resolution_to_address.slice(-6) }}</span>
                  <CopyButton :value="selectedRow.resolution_to_address" show-toast />
                </div>
              </div>
            </div>
          </div>
        </template>
      </SheetContent>
    </Sheet>
  </div>
</template>

<script lang="ts" setup>
import { ref, reactive, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import {
  RefreshCw, AlertTriangle, DollarSign, Shield,
  Eye, Check, Link, ArrowUpRight, ExternalLink, ArrowRight,
} from 'lucide-vue-next';
import { toast } from 'vue-sonner';
import { networkDisplayName } from '@/utils/networkUtils';
import { useEnvironmentStore } from '@/stores/environment';
import useLoading from '@/hooks/loading';
import { formatUsdt } from '@/utils/currency';
import { formatDateTime } from '@/utils/date';
import { txUrl, addressUrl } from '@/utils/explorer';
import {
  queryResolutionStats, queryExceptions, acceptException, attachException, transferException,
  type ExceptionRecord, type ResolutionStats,
} from '@/api/resolution';
import CopyButton from '@/components/CopyButton.vue';
import { useSubMerchantFilter, smColorClass } from '@/composables/useSubMerchantFilter';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/components/ui/sheet';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';

const { t } = useI18n();
const { loading, setLoading } = useLoading();
const envStore = useEnvironmentStore();
const smFilter = useSubMerchantFilter();
const smSelected = smFilter.selected;
const smList = smFilter.subMerchants;
const hasSubMerchants = smFilter.hasSubMerchants;

smFilter.loadSubMerchants();

const resStats = reactive<Partial<ResolutionStats>>({});
const exceptions = ref<ExceptionRecord[]>([]);
const filterStatus = ref('all');

// Sheet state
const sheetOpen = ref(false);
const selectedRow = ref<ExceptionRecord | null>(null);
const actionLoading = ref(false);

// Inline form state (inside Sheet)
const showAttachForm = ref(false);
const attachSessionId = ref('');
const showTransferForm = ref(false);
const transferAddress = ref('');
const transferCode = ref('');

const openReview = (row: ExceptionRecord) => {
  selectedRow.value = row;
  showAttachForm.value = false;
  showTransferForm.value = false;
  attachSessionId.value = '';
  transferAddress.value = '';
  transferCode.value = '';
  sheetOpen.value = true;
};

// Fee preview computations (mirrors backend FeeConfig)
// Backend returns amounts in human-readable units (e.g., 10.0 = 10 USDT)
const FLOOR_REFUND = 1.5;      // 1.5 USDT
const FEE_PERCENTAGE = 0.001;   // 0.1%

const isAmlException = computed(() =>
  selectedRow.value?.exception_type === 'RiskBlocked'
);

const refundFeeRaw = computed(() => {
  if (!selectedRow.value) return 0;
  if (isAmlException.value) return 0;
  const amt = Number(selectedRow.value.amount);
  const pctFee = amt * FEE_PERCENTAGE;
  const fee = Math.max(FLOOR_REFUND, pctFee);
  return Math.min(fee, amt); // fee never exceeds amount
});

const refundFee = computed(() => formatUsdt(refundFeeRaw.value));
const refundNet = computed(() =>
  formatUsdt(Number(selectedRow.value?.amount ?? 0) - refundFeeRaw.value)
);

const truncateAddr = (addr: string, head = 10, tail = 6) =>
  addr.length > head + tail + 3 ? `${addr.slice(0, head)}…${addr.slice(-tail)}` : addr;

const getStatusVariant = (s: string) => {
  const m: Record<string, 'default' | 'secondary' | 'destructive' | 'outline'> = {
    Pending: 'secondary', Processing: 'secondary', Resolved: 'default', Failed: 'destructive',
  };
  return m[s] || 'outline';
};

const fetchStats = async () => {
  try { Object.assign(resStats, await queryResolutionStats(smFilter.filterParams.value)); } catch { /* */ }
};

const fetchData = async (silent = false) => {
  if (!silent) setLoading(true);
  try {
    const res = await queryExceptions({
      page: 1, page_size: 50,
      ...(filterStatus.value !== 'all' && { status: filterStatus.value.toLowerCase() }),
      ...smFilter.filterParams.value,
    });
    exceptions.value = res.data;
  } catch { /* */ } finally {
    setLoading(false);
  }
};

const search = () => fetchData();

const refreshAfterAction = () => {
  sheetOpen.value = false;
  fetchData();
  fetchStats();
};

const handleAccept = async () => {
  if (!selectedRow.value) return;
  actionLoading.value = true;
  try {
    await acceptException(selectedRow.value.id);
    toast.success(t('resolution.accepted'));
    refreshAfterAction();
  } catch { /* interceptor shows backend error */ }
  finally { actionLoading.value = false; }
};

async function submitAttach() {
  if (!selectedRow.value || !attachSessionId.value.trim()) return;
  actionLoading.value = true;
  try {
    await attachException(selectedRow.value.id, { session_id: attachSessionId.value.trim() });
    toast.success(t('resolution.attached'));
    refreshAfterAction();
  } catch { /* interceptor shows backend error */ }
  finally { actionLoading.value = false; }
}

async function submitTransfer() {
  if (!selectedRow.value || !transferAddress.value.trim()) return;
  actionLoading.value = true;
  try {
    await transferException(selectedRow.value.id, {
      to_address: transferAddress.value.trim(),
      code: transferCode.value || '',
    });
    toast.success(t('resolution.transferred'));
    refreshAfterAction();
  } catch { /* interceptor shows backend error */ }
  finally { actionLoading.value = false; }
}

import { useSmartPolling } from '@/composables/useSmartPolling';

useSmartPolling(async () => {
  await Promise.all([fetchStats(), fetchData(true)]);
});
</script>

<style scoped>
/* ── Hero stat cards ── */
.res-hero-card {
  position: relative;
  overflow: hidden;
  border-radius: 0.75rem;
  border: 1px solid oklch(0.9 0.005 264);
  padding: 1.25rem 1.5rem;
  background: oklch(1 0 0);
  box-shadow: 0 1px 3px oklch(0 0 0 / 4%), 0 0 0 1px oklch(0 0 0 / 2%);
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}
.res-hero-card:hover {
  box-shadow: 0 4px 12px oklch(0 0 0 / 6%), 0 0 0 1px oklch(0 0 0 / 3%);
  transform: translateY(-2px);
}
.res-hero-card__content {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: 1rem;
}
.res-hero-card__glow {
  position: absolute;
  top: -40%;
  right: -20%;
  width: 160px;
  height: 160px;
  border-radius: 50%;
  filter: blur(50px);
  opacity: 0.12;
  pointer-events: none;
  transition: opacity 0.3s ease;
}
.res-hero-card:hover .res-hero-card__glow {
  opacity: 0.2;
}
/* Amber variant */
.res-hero-card--amber { border-left: 3px solid oklch(0.769 0.188 70.08); }
.res-hero-card--amber .res-hero-card__glow { background: oklch(0.769 0.188 70.08); }
/* Blue variant */
.res-hero-card--blue { border-left: 3px solid oklch(0.546 0.245 262.881); }
.res-hero-card--blue .res-hero-card__glow { background: oklch(0.546 0.245 262.881); }

.res-hero-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 2.75rem;
  height: 2.75rem;
  border-radius: 0.625rem;
  flex-shrink: 0;
}
.res-hero-icon--amber {
  background: linear-gradient(135deg, oklch(0.769 0.188 70.08 / 12%), oklch(0.769 0.188 70.08 / 6%));
  color: oklch(0.65 0.17 60);
}
.res-hero-icon--blue {
  background: linear-gradient(135deg, oklch(0.546 0.245 262.881 / 12%), oklch(0.546 0.245 262.881 / 6%));
  color: oklch(0.546 0.245 262.881);
}
.res-hero-label {
  font-size: 0.7rem;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: oklch(0.50 0.015 264);
  margin-bottom: 0.125rem;
}
.res-hero-value {
  font-size: 1.75rem;
  font-weight: 700;
  line-height: 1;
  letter-spacing: -0.02em;
  font-variant-numeric: tabular-nums;
  color: oklch(0.18 0.014 265.2);
}

/* ── Sheet action buttons ── */
.sheet-action-btn {
  display: flex;
  align-items: center;
  gap: 0.625rem;
  width: 100%;
  padding: 0.625rem 0.75rem;
  border-radius: 0.5rem;
  border: 1px solid hsl(var(--border));
  background: hsl(var(--card));
  box-shadow: 0 1px 2px 0 rgb(0 0 0 / 0.05);
  cursor: pointer;
  font: inherit;
  color: inherit;
  transition: all 0.15s ease;
}
.sheet-action-btn:hover {
  background: hsl(var(--muted) / 0.6);
  box-shadow: 0 2px 4px 0 rgb(0 0 0 / 0.08);
  transform: translateY(-0.5px);
}
.sheet-action-btn:active {
  transform: translateY(0) scale(0.99);
  box-shadow: 0 0 0 0 transparent;
}
.sheet-action-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  transform: none;
  box-shadow: none;
}
.sheet-action-btn--danger {
  border-color: hsl(var(--destructive) / 0.15);
}
.sheet-action-btn--danger:hover {
  background: hsl(var(--destructive) / 0.06);
  border-color: hsl(var(--destructive) / 0.25);
}
.sheet-action-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 1.75rem;
  height: 1.75rem;
  border-radius: 0.375rem;
  flex-shrink: 0;
}
</style>
