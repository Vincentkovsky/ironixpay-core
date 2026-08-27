<template>
  <div class="space-y-6">
    <!-- Balance Hero -->
    <div class="bill-balance-hero animate-fade-in-up">
      <div class="bill-balance-hero__top">
        <div>
          <p class="bill-balance-label">{{ t('billing.currentBalance') }}</p>
          <div class="bill-balance-tokens">
            <div class="bill-token-group">
              <span class="bill-balance-amount">{{ formatAmount(balance) }}</span>
              <span class="bill-token-badge bill-token-badge--usdt">USDT</span>
            </div>
            <div v-if="usdcBalance > 0" class="bill-token-group">
              <span class="bill-balance-amount bill-balance-amount--secondary">{{ formatAmount(usdcBalance) }}</span>
              <span class="bill-token-badge bill-token-badge--usdc">USDC</span>
            </div>
          </div>
        </div>
        <Button @click="showWithdraw = true">
          <ArrowUpRight class="h-4 w-4 mr-1.5" /> {{ t('billing.withdraw') }}
        </Button>
      </div>

      <!-- Per-chain rows: balance + collection address -->
      <div v-if="chainRows.length >= 1" class="bill-chain-rows">
        <div
          v-for="row in chainRows"
          :key="row.network"
          class="bill-chain-row"
          :class="{ 'bill-chain-row--muted': row.balance <= 0 && row.usdcBalance <= 0 }"
        >
          <div class="bill-chain-row__name">
            <img v-if="networkIcons[row.network]" :src="networkIcons[row.network]" :alt="row.network" class="bill-chain-icon" />
            <span>{{ networkDisplayName(row.network, envStore.isSandbox) }}</span>
          </div>
          <div class="bill-chain-row__balance">
            <span class="bill-chain-bal-pill">{{ formatAmount(row.balance) }} <small>USDT</small></span>
            <span v-if="row.usdcBalance > 0" class="bill-chain-bal-pill bill-chain-bal-pill--usdc">{{ formatAmount(row.usdcBalance) }} <small>USDC</small></span>
          </div>
          <div v-if="row.address" class="bill-chain-row__addr">
            <span class="bill-chain-row__addr-label">{{ t('billing.withdrawAddress') }}:</span>
            <span class="bill-chain-row__addr-text">{{ row.address }}</span>
            <Button variant="ghost" size="sm" class="h-5 w-5 p-0" @click="copyAddr(row.address)">
              <Copy class="h-3 w-3" />
            </Button>
          </div>
          <span v-else class="bill-chain-row__addr bill-chain-row__no-addr">{{ t('billing.noAddress') }}</span>
        </div>
        <div class="bill-chain-row__edit">
          <Button variant="ghost" size="sm" class="h-6 text-xs gap-1" @click="showEditAddress = true">
            <Pencil class="h-3 w-3" /> {{ t('billing.editAddress') || 'Edit' }}
          </Button>
        </div>
      </div>

      <div class="bill-balance-hero__glow" />
    </div>

    <!-- Auto-Withdraw Settings Card -->
    <Card class="animate-fade-in-up delay-1">
      <CardContent class="pt-6 space-y-4">
        <div class="flex items-center justify-between">
          <div class="space-y-0.5">
            <Label class="text-sm font-medium">{{ t('funds.autoWithdraw') }}</Label>
            <p class="text-xs text-muted-foreground">{{ t('funds.autoWithdrawDesc') }}</p>
          </div>
          <Switch
            v-model="autoWithdrawForm.enabled"
            :disabled="!isOwner || savingAuto"
          />
        </div>
        <div v-if="autoWithdrawForm.enabled" class="space-y-3 pt-1">
          <div class="max-w-xs space-y-1">
            <Label class="text-xs text-muted-foreground">{{ t('funds.autoThreshold') }} (USDT)</Label>
            <Input
              v-model="autoWithdrawForm.threshold"
              type="number"
              step="1"
              min="1"
              placeholder="1000"
              :disabled="!isOwner || savingAuto"
              class="tabular-nums"
            />
          </div>
          <p class="text-[11px] text-muted-foreground">{{ t('funds.autoWithdrawHint') }}</p>
        </div>
        <div v-if="isOwner" class="pt-1">
          <Button size="sm" :disabled="savingAuto" @click="saveAutoWithdraw">
            <Loader2 v-if="savingAuto" class="h-3.5 w-3.5 mr-1.5 animate-spin" />
            {{ t('settings.save') }}
          </Button>
        </div>
      </CardContent>
    </Card>

    <!-- Withdrawal History -->
    <Card class="animate-fade-in-up delay-2">
      <CardHeader><CardTitle>{{ t('billing.withdrawalHistory') }}</CardTitle></CardHeader>
      <CardContent>
        <div class="overflow-x-auto -mx-4 md:mx-0">
        <Table class="min-w-[340px]">
          <TableHeader>
            <TableRow>
              <TableHead>{{ t('table.time') }}</TableHead>
              <TableHead class="hidden sm:table-cell w-[70px]">{{ t('table.network') }}</TableHead>
              <TableHead class="text-right">{{ t('table.amount') }}</TableHead>
              <TableHead class="hidden md:table-cell text-right">{{ t('billing.fee') }}</TableHead>
              <TableHead class="hidden md:table-cell text-right">{{ t('billing.youReceive') }}</TableHead>
              <TableHead>{{ t('table.status') }}</TableHead>
              <TableHead class="hidden sm:table-cell">{{ t('billing.txHash') }}</TableHead>
              <TableHead v-if="canApprove" class="w-[100px]">{{ t('table.actions') }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="w in withdrawals" :key="w.id" class="transition-colors">
              <TableCell class="text-sm">{{ formatTime(w.createdAt) }}</TableCell>
              <TableCell class="hidden sm:table-cell">
                <Badge variant="outline" class="text-[10px] px-1.5 py-0">{{ networkDisplayName(w.network, envStore.isSandbox) }}</Badge>
              </TableCell>
              <TableCell class="text-right tabular-nums">{{ formatUsdt(w.amount) }} {{ w.currency || 'USDT' }}</TableCell>
              <TableCell class="hidden md:table-cell text-right tabular-nums text-muted-foreground">{{ formatUsdt(w.fee) }} {{ w.currency || 'USDT' }}</TableCell>
              <TableCell class="hidden md:table-cell text-right tabular-nums font-medium text-green-600">
                {{ formatUsdt(w.netAmount) }} {{ w.currency || 'USDT' }}
              </TableCell>
              <TableCell>
                <div class="flex items-center gap-2">
                  <span :class="['status-dot', `status-dot--${w.status.toLowerCase()}`]" />
                  <Badge :variant="getWdVariant(w.status)">{{ w.status }}</Badge>
                </div>
              </TableCell>
              <TableCell class="hidden sm:table-cell">
                <a v-if="w.txHash" :href="txUrl(w.txHash, w.network)" target="_blank" class="font-mono text-xs text-blue-500 hover:underline">
                  {{ w.txHash.slice(0, 10) }}…
                </a>
                <span v-else class="text-muted-foreground">—</span>
              </TableCell>
              <TableCell v-if="canApprove">
                <div v-if="w.status === 'PendingApproval'" class="flex gap-1">
                  <Button variant="outline" size="sm" class="h-7 text-xs" @click="openWdApproval(w.id, 'approve')">
                    {{ t('approval.approve') }}
                  </Button>
                  <Button variant="ghost" size="sm" class="h-7 text-xs text-destructive" @click="openWdApproval(w.id, 'reject')">
                    {{ t('approval.reject') }}
                  </Button>
                </div>
              </TableCell>
            </TableRow>
            <TableRow v-if="withdrawals.length === 0">
              <TableCell colspan="7" class="text-center text-muted-foreground py-12">
                <div class="flex flex-col items-center gap-2">
                  <Inbox class="h-8 w-8 text-muted-foreground/30" />
                  <span>{{ t('billing.noWithdrawals') }}</span>
                </div>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
        </div>
      </CardContent>
    </Card>

    <!-- Withdraw Dialog -->
    <Dialog v-model:open="showWithdraw">
      <DialogContent class="max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t('billing.withdrawDialog.title') }} {{ wdCurrency }}</DialogTitle>
          <DialogDescription>{{ t('billing.withdrawDialog.description') }}</DialogDescription>
        </DialogHeader>
        <form class="space-y-4" @submit.prevent="submitWithdrawal">
          <div class="space-y-2">
            <Label for="wd-network">{{ t('billing.withdrawDialog.network') }}</Label>
            <div class="flex items-center gap-3">
              <Select v-model="wdNetwork" @update:model-value="onNetworkChange">
                <SelectTrigger id="wd-network" class="flex-1">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="n in availableNetworks" :key="n" :value="n">{{ networkDisplayName(n, envStore.isSandbox) }}</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
          <div class="space-y-2">
            <Label for="wd-currency">{{ t('billing.withdrawDialog.currency') }}</Label>
            <div class="flex items-center gap-3">
              <Select v-model="wdCurrency">
                <SelectTrigger id="wd-currency" class="w-28">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="c in availableCurrencies" :key="c" :value="c">{{ c }}</SelectItem>
                </SelectContent>
              </Select>
              <span class="text-sm text-muted-foreground tabular-nums whitespace-nowrap">
                {{ t('billing.withdrawDialog.available') }}: {{ fmtAmt(currentBalance) }} {{ wdCurrency }}
              </span>
            </div>
          </div>
          <div class="space-y-2">
            <Label for="wd-amount">{{ t('billing.withdrawDialog.amount') }} ({{ wdCurrency }})</Label>
            <Input id="wd-amount" v-model="wdAmount" type="number" step="0.01" placeholder="0.00" />
          </div>
          <!-- Fee Breakdown -->
          <div v-if="wdAmountNum > 0" class="rounded-lg border bg-muted/30 px-4 py-3 space-y-1.5 text-sm">
            <div class="flex justify-between">
              <span class="text-muted-foreground">{{ t('billing.withdrawDialog.fee') }} ({{ wdNetwork }})</span>
              <span class="tabular-nums">-{{ fmtAmt(currentOutboundFee) }} {{ wdCurrency }}</span>
            </div>
            <Separator />
            <div class="flex justify-between font-medium">
              <span>{{ t('billing.withdrawDialog.youReceive') }}</span>
              <span class="tabular-nums" :class="wdNet > 0 ? 'text-green-600' : 'text-red-500'">{{ wdNet > 0 ? fmtAmt(wdNet) : '0.00' }} {{ wdCurrency }}</span>
            </div>
          </div>
          <div class="space-y-2">
            <Label for="wd-totp">{{ t('billing.withdrawDialog.totp') }}</Label>
            <Input id="wd-totp" v-model="wdTotp" maxlength="6" placeholder="000000" class="font-mono tracking-widest text-center" />
          </div>
          <div class="flex justify-end gap-2 pt-2">
            <Button variant="outline" type="button" @click="showWithdraw = false">{{ t('billing.withdrawDialog.cancel') }}</Button>
            <Button type="submit" :disabled="wdLoading || wdNet <= 0">
              <Loader2 v-if="wdLoading" class="h-4 w-4 mr-1.5 animate-spin" />
              {{ t('billing.withdrawDialog.confirm') }}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>

    <!-- Edit Address Dialog -->
    <Dialog v-model:open="showEditAddress">
      <DialogContent class="max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t('billing.editAddress') }}</DialogTitle>
          <DialogDescription>{{ t('billing.editAddressDesc') }}</DialogDescription>
        </DialogHeader>
        <form class="space-y-4" @submit.prevent="submitAddressUpdate">
          <div class="space-y-2">
            <Label for="addr-network">{{ t('billing.network') || 'Network' }}</Label>
            <Select v-model="addrNetwork">
              <SelectTrigger id="addr-network" class="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem v-for="n in availableNetworks" :key="n" :value="n">{{ networkDisplayName(n, envStore.isSandbox) }}</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div class="space-y-2">
            <Label for="addr-new">{{ t('billing.newAddress') }}</Label>
            <Input id="addr-new" v-model="addrNew" :placeholder="addrNetwork === 'TRON' ? 'T…' : '0x…'" class="font-mono text-sm" />
          </div>
          <div class="space-y-2">
            <Label for="addr-totp">{{ t('billing.withdrawDialog.totp') }}</Label>
            <Input id="addr-totp" v-model="addrTotp" maxlength="6" placeholder="000000" class="font-mono tracking-widest text-center" />
          </div>
          <div class="flex justify-end gap-2 pt-2">
            <Button variant="outline" type="button" @click="showEditAddress = false">{{ t('billing.withdrawDialog.cancel') }}</Button>
            <Button type="submit" :disabled="addrLoading">
              <Loader2 v-if="addrLoading" class="h-4 w-4 mr-1.5 animate-spin" />
              {{ t('settings.save') }}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>

    <!-- Withdrawal Approval Dialog -->
    <ApprovalDialog
      v-model:open="wdApprovalOpen"
      :action="wdApprovalAction"
      :target-id="wdApprovalTargetId"
      target-type="withdrawal"
      @done="fetchWithdrawals"
    />
  </div>
</template>

<script lang="ts" setup>
import { ref, reactive, computed, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { resolveErrorMessage } from '@/utils/error-utils';
import { ArrowUpRight, Loader2, Inbox, Pencil, Copy } from 'lucide-vue-next';
import { networkIcons } from '@ironix-pay/ui';
import { toast } from 'vue-sonner';
import { networkDisplayName } from '@/utils/networkUtils';
import { useEnvironmentStore } from '@/stores/environment';
import { useUserStore } from '@/stores';
import { useCanApprove } from '@/composables/useCanApprove';
import { http } from '@/utils/request';
import { formatUsdt, fmtAmt } from '@/utils/currency';
import { formatDateTimeFull } from '@/utils/date';
import { txUrl } from '@/utils/explorer';
import { useFeeConfig } from '@/composables/useFeeConfig';
import { listWithdrawals, requestWithdrawal, type WithdrawalResponse } from '@/api/withdrawal';
import { getPayoutSettings, updatePayoutSettings } from '@/api/payout-settings';
import ApprovalDialog from '@/components/ApprovalDialog.vue';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Separator } from '@/components/ui/separator';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';

const { t } = useI18n();
const userStore = useUserStore();
const envStore = useEnvironmentStore();
const balance = computed(() => userStore.balance || 0);
const usdcBalance = computed(() => userStore.usdcBalance || 0);
const isOwner = computed(() => userStore.orgRole === 'owner');
const { canApprove } = useCanApprove();

// Fee config from backend
const { feeConfig, load: loadFees, getOutboundFee } = useFeeConfig();

const enabledNetworkSet = computed(() => {
  const fees = feeConfig.value?.outbound_fees;
  return fees ? new Set(Object.keys(fees)) : null;
});

const chainRows = computed(() => {
  const addrs = userStore.collectionAddresses;
  const balances = userStore.chainBalances;
  const usdcBalances = userStore.chainUsdcBalances;
  const enabled = enabledNetworkSet.value;
  return Object.keys(addrs)
    .filter((network) => !enabled || enabled.has(network))
    .map((network) => ({
      network,
      balance: balances[network] || 0,
      usdcBalance: usdcBalances[network] || 0,
      address: addrs[network] || '',
    }))
    .sort((a, b) => b.balance - a.balance);
});

const copyAddr = (addr: string) => {
  if (!addr) return;
  navigator.clipboard.writeText(addr);
  toast.success(t('billing.addressCopied') || 'Copied!');
};

// ── Auto-Withdraw Settings ──
const savingAuto = ref(false);
const autoWithdrawForm = reactive({
  enabled: false,
  threshold: '1000',
});

const fetchAutoWithdrawSettings = async () => {
  try {
    const data = await getPayoutSettings();
    autoWithdrawForm.enabled = data.autoWithdrawEnabled ?? false;
    autoWithdrawForm.threshold = data.autoWithdrawThreshold ?? '1000';
  } catch { /* use defaults */ }
};

const saveAutoWithdraw = async () => {
  savingAuto.value = true;
  try {
    await updatePayoutSettings({
      autoWithdrawEnabled: autoWithdrawForm.enabled,
      autoWithdrawThreshold: autoWithdrawForm.enabled ? String(autoWithdrawForm.threshold) : null,
    });
    toast.success(t('payoutSettings.saved'));
  } catch { /* interceptor */ }
  finally { savingAuto.value = false; }
};

// ── Withdrawal ──
const showWithdraw = ref(false);
const wdLoading = ref(false);
const wdAmount = ref('');
const wdTotp = ref('');
const wdNetwork = ref('TRON');
const wdCurrency = ref('USDT');
const withdrawals = ref<WithdrawalResponse[]>([]);

const availableNetworks = computed(() => {
  const allNetworks = Object.keys(userStore.chainBalances).length > 0
    ? Object.keys(userStore.chainBalances)
    : ['TRON'];
  const enabled = enabledNetworkSet.value;
  return enabled ? allNetworks.filter((n) => enabled.has(n)) : allNetworks;
});

const availableCurrencies = computed(() => {
  if (wdNetwork.value === 'TRON') return ['USDT'];
  return ['USDT', 'USDC'];
});

const onNetworkChange = () => {
  if (wdNetwork.value === 'TRON' && wdCurrency.value === 'USDC') {
    wdCurrency.value = 'USDT';
  }
};

const currentBalance = computed(() => {
  if (wdCurrency.value === 'USDC') {
    return userStore.chainUsdcBalances[wdNetwork.value] || 0;
  }
  return userStore.chainBalances[wdNetwork.value] || 0;
});

const currentOutboundFee = computed(() => getOutboundFee(wdNetwork.value));
const wdAmountNum = computed(() => parseFloat(wdAmount.value) || 0);
const wdNet = computed(() => Math.max(0, wdAmountNum.value - currentOutboundFee.value));

const fetchWithdrawals = async () => {
  try { const r = await listWithdrawals(); withdrawals.value = r.data; } catch { /* */ }
};

const submitWithdrawal = async () => {
  if (!wdAmount.value || !wdTotp.value) return;
  wdLoading.value = true;
  try {
    await requestWithdrawal(wdAmount.value.toString(), wdTotp.value, wdNetwork.value, wdCurrency.value);
    toast.success(t('billing.withdrawDialog.submitted'));
    showWithdraw.value = false;
    wdAmount.value = '';
    wdTotp.value = '';
    fetchWithdrawals();
  } catch { /* interceptor shows backend error */ }
  finally { wdLoading.value = false; }
};

const getWdVariant = (s: string) => {
  const m: Record<string, 'default' | 'secondary' | 'destructive' | 'outline'> = {
    Pending: 'secondary', PendingApproval: 'outline', Processing: 'secondary',
    Completed: 'default', Failed: 'destructive', Cancelled: 'destructive', ApprovalExpired: 'outline',
  };
  return m[s] || 'outline';
};

// Withdrawal approval dialog
const wdApprovalOpen = ref(false);
const wdApprovalAction = ref<'approve' | 'reject'>('approve');
const wdApprovalTargetId = ref('');
const openWdApproval = (id: string, action: 'approve' | 'reject') => {
  wdApprovalTargetId.value = id;
  wdApprovalAction.value = action;
  wdApprovalOpen.value = true;
};

// Edit Collection Address
const showEditAddress = ref(false);
const addrNew = ref('');
const addrTotp = ref('');
const addrNetwork = ref('TRON');
const addrLoading = ref(false);

const submitAddressUpdate = async () => {
  if (!addrNew.value || !addrTotp.value) return;
  addrLoading.value = true;
  try {
    await http.post('/api/internal/merchants/wallets/config', {
      network: addrNetwork.value,
      collection_address: addrNew.value,
      code: addrTotp.value,
    }, { skipErrorToast: true });
    toast.success(t('billing.addressUpdated'));
    showEditAddress.value = false;
    addrNew.value = '';
    addrTotp.value = '';
    await userStore.info();
  } catch (e: any) {
    toast.error(resolveErrorMessage(e, t, 'billing.addressUpdateFailed'));
  } finally {
    addrLoading.value = false;
  }
};

const formatTime = (t: string) => formatDateTimeFull(t);
const formatAmount = (v: number) => v.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 6 });

import { useSmartPolling } from '@/composables/useSmartPolling';

useSmartPolling(async () => {
  await Promise.all([fetchWithdrawals(), userStore.info()]);
});

onMounted(() => {
  fetchAutoWithdrawSettings();
  loadFees();
});
</script>

<style scoped>
/* ── Balance Hero ── */
.bill-balance-hero {
  position: relative;
  overflow: hidden;
  border-radius: 0.875rem;
  border: 1px solid oklch(0.546 0.245 262.881 / 12%);
  padding: 1.5rem 1.75rem;
  background: linear-gradient(135deg, oklch(1 0 0), oklch(0.97 0.012 262));
  box-shadow:
    0 1px 3px oklch(0 0 0 / 4%),
    0 0 0 1px oklch(0 0 0 / 2%);
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}
.bill-balance-hero:hover {
  box-shadow:
    0 8px 24px oklch(0.546 0.245 262.881 / 8%),
    0 0 0 1px oklch(0.546 0.245 262.881 / 10%);
  transform: translateY(-1px);
}
.bill-balance-hero__top {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
@media (min-width: 640px) {
  .bill-balance-hero__top {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    gap: 0;
  }
}
.bill-balance-hero__glow {
  position: absolute;
  top: -60%;
  right: -10%;
  width: 220px;
  height: 220px;
  border-radius: 50%;
  background: oklch(0.546 0.245 262.881);
  filter: blur(70px);
  opacity: 0.08;
  pointer-events: none;
  transition: opacity 0.3s ease;
}
.bill-balance-hero:hover .bill-balance-hero__glow {
  opacity: 0.14;
}
.bill-balance-label {
  font-size: 0.7rem;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: oklch(0.50 0.015 264);
  margin-bottom: 0.375rem;
}
.bill-balance-tokens {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem 1.5rem;
  align-items: baseline;
}
.bill-token-group {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
}
.bill-balance-amount {
  font-size: 1.75rem;
  font-weight: 700;
  line-height: 1;
  letter-spacing: -0.025em;
  font-variant-numeric: tabular-nums;
  color: oklch(0.18 0.014 265.2);
}
.bill-balance-amount--secondary {
  font-size: 1.5rem;
}
@media (min-width: 640px) {
  .bill-balance-amount { font-size: 2.25rem; }
  .bill-balance-amount--secondary { font-size: 1.75rem; }
}
.bill-token-badge {
  display: inline-flex;
  align-items: center;
  padding: 0.125rem 0.5rem;
  border-radius: 100px;
  font-size: 0.625rem;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}
.bill-token-badge--usdt {
  background: oklch(0.87 0.12 158 / 18%);
  color: oklch(0.45 0.12 158);
}
.bill-token-badge--usdc {
  background: oklch(0.87 0.14 250 / 18%);
  color: oklch(0.45 0.14 250);
}
.bill-chain-rows {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  gap: 0.375rem;
  margin-top: 1rem;
  padding-top: 0.75rem;
  border-top: 1px solid oklch(0.546 0.245 262.881 / 8%);
}
.bill-chain-row {
  display: grid;
  grid-template-columns: minmax(120px, auto) minmax(100px, auto) 1fr;
  align-items: center;
  gap: 0.75rem;
  padding: 0.5rem 0.75rem;
  border-radius: 0.5rem;
  border: 1px solid oklch(0.546 0.245 262.881 / 10%);
  background: oklch(1 0 0 / 60%);
  transition: all 0.2s ease;
}
.bill-chain-row:hover:not(.bill-chain-row--muted) {
  background: oklch(1 0 0 / 95%);
  border-color: oklch(0.546 0.245 262.881 / 22%);
}
.bill-chain-row:not(.bill-chain-row--muted) {
  border-color: oklch(0.546 0.245 262.881 / 15%);
  background: oklch(1 0 0 / 80%);
  box-shadow: 0 1px 3px oklch(0.546 0.245 262.881 / 4%);
}
.bill-chain-row--muted { opacity: 0.55; }
.bill-chain-row__name {
  display: flex;
  align-items: center;
  gap: 0.375rem;
  font-size: 0.7rem;
  font-weight: 600;
  letter-spacing: 0.03em;
  color: oklch(0.40 0.015 264);
  white-space: nowrap;
}
.bill-chain-icon { width: 16px; height: 16px; border-radius: 50%; flex-shrink: 0; }
.bill-chain-row__balance {
  display: flex;
  align-items: center;
  gap: 0.375rem;
  flex-wrap: wrap;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.bill-chain-bal-pill {
  display: inline-flex;
  align-items: baseline;
  gap: 0.2rem;
  font-size: 0.8rem;
  font-weight: 700;
  color: oklch(0.18 0.014 265.2);
}
.bill-chain-bal-pill small {
  font-size: 0.6rem;
  font-weight: 600;
  color: oklch(0.55 0.015 264);
}
.bill-chain-bal-pill--usdc {
  padding-left: 0.375rem;
  border-left: 1.5px solid oklch(0.546 0.245 262.881 / 12%);
}
.bill-chain-row__addr {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  justify-self: end;
  min-width: 0;
}
.bill-chain-row__addr-text {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 0.6875rem;
  color: oklch(0.50 0.015 264);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.bill-chain-row__addr-label {
  font-size: 0.625rem;
  color: oklch(0.60 0.01 264);
  white-space: nowrap;
  flex-shrink: 0;
}
.bill-chain-row__no-addr {
  font-size: 0.6875rem;
  color: oklch(0.65 0.08 65);
}
.bill-chain-row__edit {
  display: flex;
  justify-content: flex-end;
  margin-top: 0.125rem;
}
@media (max-width: 480px) {
  .bill-chain-row {
    grid-template-columns: 1fr auto;
    gap: 0.25rem 0.5rem;
  }
  .bill-chain-row__addr {
    grid-column: 1 / -1;
    justify-self: start;
  }
}
</style>
