<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="animate-fade-in-up">
      <h1 class="text-2xl font-bold tracking-tight">{{ t('agent.title') }}</h1>
      <p class="text-sm text-muted-foreground mt-1">{{ t('agent.subtitle') }}</p>
    </div>

    <!-- Loading -->
    <div v-if="loading" class="flex items-center justify-center py-16">
      <div class="flex items-center gap-3 text-muted-foreground">
        <Loader2 class="h-5 w-5 animate-spin" />
        <span class="text-sm">{{ t('agent.loading') }}</span>
      </div>
    </div>

    <template v-else>
      <!-- Agent Info Hero -->
      <div class="agent-hero animate-fade-in-up">
        <div class="agent-hero__content">
          <div class="agent-hero__info">
            <div class="agent-hero__code-group">
              <p class="agent-hero__label">{{ t('agent.referralCode') }}</p>
              <div class="flex items-center gap-2">
                <span class="agent-hero__code">{{ overview?.referral_code }}</span>
                <button
                  class="agent-copy-btn"
                  @click="copy(overview?.referral_code || '')"
                >
                  <Copy class="h-3.5 w-3.5" />
                </button>
              </div>
            </div>
            <div class="agent-hero__link-group">
              <p class="agent-hero__label">{{ t('agent.referralLink') }}</p>
              <div class="flex items-center gap-2">
                <span class="agent-hero__link">{{ referralLink }}</span>
                <button
                  class="agent-copy-btn"
                  @click="copy(referralLink)"
                >
                  <Copy class="h-3.5 w-3.5" />
                </button>
              </div>
            </div>
          </div>
          <div class="agent-hero__rates">
            <div>
              <p class="agent-hero__label">{{ t('agent.baseRate') }}</p>
              <p class="agent-hero__rate">{{ overview?.base_rate || '—' }}</p>
            </div>
            <div>
              <p class="agent-hero__label">{{ t('agent.maxRate') }}</p>
              <p class="agent-hero__rate">{{ overview?.max_markup || '—' }}</p>
            </div>
          </div>
        </div>
        <div class="agent-hero__glow" />
      </div>
      <!-- Quick Guide (collapsible) -->
      <div v-if="showGuide" class="agent-guide animate-fade-in-up">
        <div class="agent-guide__header" @click="guideExpanded = !guideExpanded">
          <div class="flex items-center gap-2">
            <Lightbulb class="h-4 w-4 text-amber-500" />
            <span class="font-medium text-sm">{{ t('agent.guideTitle') }}</span>
          </div>
          <div class="flex items-center gap-1">
            <button class="agent-guide__dismiss" @click.stop="dismissGuide">
              {{ t('agent.guideDismiss') }}
            </button>
            <ChevronDown v-if="!guideExpanded" class="h-4 w-4 text-muted-foreground" />
            <ChevronUp v-else class="h-4 w-4 text-muted-foreground" />
          </div>
        </div>
        <div v-if="guideExpanded" class="agent-guide__body">
          <div class="agent-guide__step">
            <span class="agent-guide__badge">1</span>
            <div>
              <p class="font-medium text-sm">{{ t('agent.guideStep1Title') }}</p>
              <p class="text-xs text-muted-foreground mt-0.5">{{ t('agent.guideStep1Desc') }}</p>
            </div>
          </div>
          <div class="agent-guide__step">
            <span class="agent-guide__badge">2</span>
            <div>
              <p class="font-medium text-sm">{{ t('agent.guideStep2Title') }}</p>
              <p class="text-xs text-muted-foreground mt-0.5">{{ t('agent.guideStep2Desc') }}</p>
            </div>
          </div>
          <div class="agent-guide__step">
            <span class="agent-guide__badge">3</span>
            <div>
              <p class="font-medium text-sm">{{ t('agent.guideStep3Title') }}</p>
              <p class="text-xs text-muted-foreground mt-0.5">{{ t('agent.guideStep3Desc') }}</p>
            </div>
          </div>
        </div>
      </div>

      <!-- Stats -->
      <div class="grid gap-5 sm:grid-cols-2">
        <div class="agent-stat-card agent-stat-card--emerald animate-fade-in-up delay-1">
          <div class="agent-stat-card__header">
            <span class="agent-stat-card__title">{{ t('agent.referredMerchants') }}</span>
            <UsersRound class="h-4 w-4 text-muted-foreground/40" />
          </div>
          <p class="agent-stat-card__value">{{ overview?.referred_merchant_count ?? 0 }}</p>
          <div class="agent-stat-card__glow" />
        </div>
        <div class="agent-stat-card agent-stat-card--amber animate-fade-in-up delay-2">
          <div class="agent-stat-card__header">
            <span class="agent-stat-card__title">{{ t('agent.totalCommission') }}</span>
            <Coins class="h-4 w-4 text-muted-foreground/40" />
          </div>
          <p class="agent-stat-card__value">{{ formatUsdt(overview?.total_commission ?? 0) }} <span class="agent-stat-card__unit">USDT</span></p>
          <div class="agent-stat-card__glow" />
        </div>
      </div>

      <!-- Commission Report -->
      <Card class="animate-fade-in-up delay-3">
        <CardHeader>
          <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
            <CardTitle>{{ t('agent.commissionReport') }}</CardTitle>
            <div class="commission-range">
              <input type="date" v-model="startDate" class="commission-date-input" />
              <span class="text-muted-foreground text-xs">→</span>
              <input type="date" v-model="endDate" class="commission-date-input" />
              <Button size="sm" variant="outline" :disabled="loadingReport" @click="queryCommission">
                <Loader2 v-if="loadingReport" class="h-3.5 w-3.5 animate-spin mr-1" />
                {{ t('agent.query') }}
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <template v-if="report">
            <!-- Commission Summary Row -->
            <div class="commission-summary">
              <div class="commission-kpi">
                <span class="commission-kpi__label">{{ t('agent.feeCollected') }}</span>
                <span class="commission-kpi__value">{{ formatUsdt(report.total_fee_collected) }}</span>
              </div>
              <div class="commission-kpi commission-kpi--highlight">
                <span class="commission-kpi__label">{{ t('agent.yourCommission') }}</span>
                <span class="commission-kpi__value">{{ formatUsdt(report.total_agent_commission) }}</span>
              </div>
              <div class="commission-kpi">
                <span class="commission-kpi__label">{{ t('agent.transactions') }}</span>
                <span class="commission-kpi__value">{{ report.total_transactions }}</span>
              </div>
            </div>

            <!-- Merged per-merchant table: rate + commission + edit -->
            <div v-if="mergedMerchants.length" class="overflow-x-auto -mx-4 md:mx-0 mt-4">
              <Table class="min-w-[500px]">
                <TableHeader>
                  <TableRow>
                    <TableHead>{{ t('agent.colMerchant') }}</TableHead>
                    <TableHead class="text-right">{{ t('agent.currentRate') }}</TableHead>
                    <TableHead class="text-right">{{ t('agent.colFee') }}</TableHead>
                    <TableHead class="text-right">{{ t('agent.colCommission') }}</TableHead>
                    <TableHead class="text-right">{{ t('agent.colTxns') }}</TableHead>
                    <TableHead class="text-right">{{ t('agent.actions') }}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  <TableRow v-for="row in mergedMerchants" :key="row.merchant_id">
                    <TableCell class="font-medium">{{ row.name }}</TableCell>
                    <TableCell class="text-right">
                      <template v-if="editingMerchantId === row.merchant_id">
                        <div class="flex items-center justify-end gap-2">
                          <input
                            v-model="editRateValue"
                            type="number"
                            step="0.01"
                            :min="rateBoundsPercent.min"
                            :max="rateBoundsPercent.max"
                            class="w-20 rounded border px-2 py-1 text-right text-sm"
                          />
                          <span class="text-xs text-muted-foreground">%</span>
                        </div>
                        <p class="text-[11px] text-muted-foreground mt-1 text-right">
                          {{ t('agent.rateRange', { min: rateBoundsPercent.min, max: rateBoundsPercent.max }) }}
                        </p>
                      </template>
                      <span v-else class="tabular-nums">{{ row.current_rate }}</span>
                    </TableCell>
                    <TableCell class="text-right tabular-nums">{{ formatUsdt(row.fee) }}</TableCell>
                    <TableCell class="text-right tabular-nums font-semibold text-emerald-600">{{ formatUsdt(row.commission) }}</TableCell>
                    <TableCell class="text-right tabular-nums">{{ row.txns }}</TableCell>
                    <TableCell class="text-right">
                      <template v-if="editingMerchantId === row.merchant_id">
                        <div class="flex items-center justify-end gap-1">
                          <Button size="sm" variant="default" :disabled="savingRate" @click="saveRate(row.merchant_id)">
                            <Loader2 v-if="savingRate" class="h-3 w-3 animate-spin mr-1" />
                            {{ t('agent.save') }}
                          </Button>
                          <Button size="sm" variant="ghost" @click="cancelEdit">{{ t('agent.cancel') }}</Button>
                        </div>
                      </template>
                      <Button v-else size="sm" variant="outline" @click="startEdit(row)">
                        {{ t('agent.editRate') }}
                      </Button>
                    </TableCell>
                  </TableRow>
                </TableBody>
              </Table>
            </div>
            <div v-else class="text-center text-muted-foreground py-8 text-sm">
              {{ t('agent.noData') }}
            </div>
          </template>
          <div v-else class="text-center text-muted-foreground py-12 text-sm">
            {{ t('agent.noData') }}
          </div>
        </CardContent>
      </Card>
    </template>
  </div>
</template>

<script lang="ts" setup>
import { ref, computed, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { useRouter } from 'vue-router';
import { useUserStore } from '@/stores';
import { fetchAgentOverview, fetchAgentCommission, fetchAgentMerchants, updateMerchantRate } from '@/api/agent';
import type { AgentOverview, CommissionReport, ReferredMerchantInfo } from '@/api/agent';
import dayjs from 'dayjs';
import { Copy, Loader2, UsersRound, Coins, Lightbulb, ChevronDown, ChevronUp } from 'lucide-vue-next';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { toast } from 'vue-sonner';

const { t } = useI18n();
const router = useRouter();
const userStore = useUserStore();

const loading = ref(true);
const overview = ref<AgentOverview | null>(null);
const report = ref<CommissionReport | null>(null);
const loadingReport = ref(false);

const startDate = ref(dayjs().subtract(30, 'day').format('YYYY-MM-DD'));
const endDate = ref(dayjs().add(1, 'day').format('YYYY-MM-DD'));

// Guide
const showGuide = ref(localStorage.getItem('agent_guide_dismissed') !== '1');
const guideExpanded = ref(true);
function dismissGuide() {
  showGuide.value = false;
  localStorage.setItem('agent_guide_dismissed', '1');
}

const referralLink = computed(() => {
  if (!overview.value?.referral_code) return '';
  const origin = window.location.origin;
  return `${origin}/login?ref=${overview.value.referral_code}`;
});

function formatUsdt(sun: number): string {
  if (sun == null) return '—';
  return (sun / 1_000_000).toLocaleString('en-US', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

async function copy(text: string) {
  await navigator.clipboard.writeText(text);
  toast.success(t('agent.copied'));
}

async function loadOverview() {
  loading.value = true;
  try {
    overview.value = await fetchAgentOverview();
  } catch {
    overview.value = null;
  } finally {
    loading.value = false;
  }
}

async function queryCommission() {
  loadingReport.value = true;
  try {
    report.value = await fetchAgentCommission({
      start_date: startDate.value,
      end_date: endDate.value,
    });
  } catch {
    report.value = null;
  } finally {
    loadingReport.value = false;
  }
}

// ── Referred Merchants ──
const merchants = ref<ReferredMerchantInfo[]>([]);
const loadingMerchants = ref(false);
const editingMerchantId = ref<string | null>(null);
const editRateValue = ref('');
const savingRate = ref(false);

const rateBoundsPercent = computed(() => {
  if (!overview.value) return { min: '0', max: '0' };
  // base_rate/max_markup come as "0.1000%" — strip the % and use the number
  const min = overview.value.base_rate?.replace('%', '') || '0';
  const max = overview.value.max_markup?.replace('%', '') || '0';
  return { min, max };
});

async function loadMerchants() {
  loadingMerchants.value = true;
  try {
    const res = await fetchAgentMerchants();
    merchants.value = res.merchants || [];
  } catch {
    merchants.value = [];
  } finally {
    loadingMerchants.value = false;
  }
}

// Merge merchants list with commission data from report
interface MergedMerchantRow {
  merchant_id: string;
  name: string;
  current_rate: string;
  fee: number;
  commission: number;
  txns: number;
}

const mergedMerchants = computed<MergedMerchantRow[]>(() => {
  // Build a lookup from commission report by merchant_id
  const commissionMap = new Map<string, { fee: number; commission: number; txns: number }>();
  if (report.value?.merchants) {
    for (const m of report.value.merchants) {
      commissionMap.set(m.merchant_id, {
        fee: m.total_fee_collected,
        commission: m.agent_commission,
        txns: m.transaction_count,
      });
    }
  }

  return merchants.value.map((m) => {
    const c = commissionMap.get(m.merchant_id);
    return {
      merchant_id: m.merchant_id,
      name: m.name,
      current_rate: m.current_rate,
      fee: c?.fee ?? 0,
      commission: c?.commission ?? 0,
      txns: c?.txns ?? 0,
    };
  });
});

function startEdit(m: MergedMerchantRow) {
  editingMerchantId.value = m.merchant_id;
  // current_rate is like "0.40%" — extract number
  editRateValue.value = m.current_rate.replace('%', '').trim();
}

function cancelEdit() {
  editingMerchantId.value = null;
  editRateValue.value = '';
}

async function saveRate(merchantId: string) {
  const pct = parseFloat(editRateValue.value);
  if (isNaN(pct)) return;
  // Convert percentage to decimal fraction: 0.4% → 0.004
  const feeRate = pct / 100;
  savingRate.value = true;
  try {
    await updateMerchantRate(merchantId, feeRate);
    toast.success(t('agent.rateUpdated'));
    cancelEdit();
    await loadMerchants();
  } catch {
    // error toast handled by interceptor
  } finally {
    savingRate.value = false;
  }
}

onMounted(async () => {
  // Route guard: non-agents redirect to dashboard
  if (!userStore.isAgent) {
    router.replace('/dashboard');
    return;
  }
  await loadOverview();
  await Promise.all([queryCommission(), loadMerchants()]);
});
</script>

<style scoped>
/* ── Agent Guide ── */
.agent-guide {
  border-radius: 0.75rem;
  border: 1px solid oklch(0.85 0.06 85);
  background: oklch(0.99 0.01 85 / 50%);
}
.agent-guide__header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.75rem 1rem;
  cursor: pointer;
  user-select: none;
}
.agent-guide__dismiss {
  font-size: 0.7rem;
  color: oklch(0.55 0 0);
  text-decoration: underline;
  text-underline-offset: 2px;
  margin-right: 0.5rem;
}
.agent-guide__dismiss:hover {
  color: oklch(0.35 0 0);
}
.agent-guide__body {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  padding: 0 1rem 1rem;
}
.agent-guide__step {
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
}
.agent-guide__badge {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 1.5rem;
  height: 1.5rem;
  border-radius: 50%;
  background: oklch(0.627 0.194 149.214);
  color: white;
  font-size: 0.7rem;
  font-weight: 700;
}

/* ── Agent Hero ── */
.agent-hero {
  position: relative;
  overflow: hidden;
  border-radius: 0.75rem;
  border: 1px solid oklch(0.627 0.194 149.214 / 15%);
  padding: 1.5rem 1.75rem;
  background: linear-gradient(135deg, oklch(1 0 0), oklch(0.97 0.012 155));
  box-shadow: 0 1px 3px oklch(0 0 0 / 4%), 0 0 0 1px oklch(0 0 0 / 2%);
}
.agent-hero__content {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}
@media (min-width: 640px) {
  .agent-hero__content {
    flex-direction: row;
    justify-content: space-between;
    align-items: flex-start;
  }
}
.agent-hero__glow {
  position: absolute;
  top: -60%;
  right: -10%;
  width: 220px;
  height: 220px;
  border-radius: 50%;
  background: oklch(0.627 0.194 149.214);
  filter: blur(70px);
  opacity: 0.08;
  pointer-events: none;
}
.agent-hero__info {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  min-width: 0;
}
.agent-hero__label {
  font-size: 0.65rem;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: oklch(0.50 0.015 264);
  margin-bottom: 0.25rem;
}
.agent-hero__code {
  font-size: 1.375rem;
  font-weight: 700;
  letter-spacing: 0.1em;
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  color: oklch(0.18 0.014 265.2);
}
.agent-hero__link {
  font-size: 0.75rem;
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  color: oklch(0.50 0.015 264);
  word-break: break-all;
}
.agent-hero__rates {
  display: flex;
  gap: 2rem;
  flex-shrink: 0;
}
.agent-hero__rate {
  font-size: 1.125rem;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: oklch(0.18 0.014 265.2);
}
.agent-copy-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.75rem;
  height: 1.75rem;
  border-radius: 0.375rem;
  color: oklch(0.50 0.015 264);
  transition: all 0.15s;
  cursor: pointer;
  border: none;
  background: transparent;
}
.agent-copy-btn:hover {
  background: oklch(0.95 0 0);
  color: oklch(0.30 0.015 264);
}

/* ── Stat Cards ── */
.agent-stat-card {
  position: relative;
  overflow: hidden;
  border-radius: 0.75rem;
  border: 1px solid oklch(0.9 0.005 264);
  padding: 1.25rem 1.5rem;
  background: oklch(1 0 0);
  box-shadow: 0 1px 3px oklch(0 0 0 / 4%), 0 0 0 1px oklch(0 0 0 / 2%);
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}
.agent-stat-card:hover {
  box-shadow: 0 4px 12px oklch(0 0 0 / 6%), 0 0 0 1px oklch(0 0 0 / 3%);
  transform: translateY(-2px);
}
.agent-stat-card--emerald { border-left: 3px solid oklch(0.627 0.194 149.214); }
.agent-stat-card--amber { border-left: 3px solid oklch(0.769 0.188 70.08); }
.agent-stat-card__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.625rem;
}
.agent-stat-card__title {
  font-size: 0.75rem;
  font-weight: 500;
  color: oklch(0.50 0.015 264);
}
.agent-stat-card__value {
  font-size: 1.75rem;
  font-weight: 700;
  line-height: 1;
  letter-spacing: -0.02em;
  font-variant-numeric: tabular-nums;
  color: oklch(0.18 0.014 265.2);
}
.agent-stat-card__unit {
  font-size: 0.75rem;
  font-weight: 600;
  color: oklch(0.50 0.015 264);
  letter-spacing: 0.04em;
}
.agent-stat-card__glow {
  position: absolute;
  top: -40%;
  right: -20%;
  width: 160px;
  height: 160px;
  border-radius: 50%;
  filter: blur(50px);
  opacity: 0.1;
  pointer-events: none;
}
.agent-stat-card--emerald .agent-stat-card__glow { background: oklch(0.627 0.194 149.214); }
.agent-stat-card--amber .agent-stat-card__glow { background: oklch(0.769 0.188 70.08); }

/* ── Commission Report ── */
.commission-range {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
}
.commission-date-input {
  padding: 0.375rem 0.625rem;
  border: 1px solid oklch(0.9 0.005 264);
  border-radius: 0.375rem;
  font-size: 0.8rem;
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  background: oklch(1 0 0);
  color: oklch(0.30 0.015 264);
  outline: none;
  transition: border-color 0.15s;
}
.commission-date-input:focus {
  border-color: oklch(0.546 0.245 262.881);
}
.commission-summary {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 0.75rem;
}
@media (min-width: 640px) {
  .commission-summary {
    grid-template-columns: repeat(3, 1fr);
  }
}
.commission-kpi {
  padding: 0.875rem 1rem;
  border-radius: 0.5rem;
  background: oklch(0.98 0.002 264);
  border: 1px solid oklch(0.93 0.005 264);
}
.commission-kpi--highlight {
  border-color: oklch(0.627 0.194 149.214 / 30%);
  background: oklch(0.627 0.194 149.214 / 5%);
}
.commission-kpi__label {
  display: block;
  font-size: 0.625rem;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: oklch(0.50 0.015 264);
  margin-bottom: 0.25rem;
}
.commission-kpi__value {
  font-size: 1.125rem;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: oklch(0.18 0.014 265.2);
}
.commission-kpi--highlight .commission-kpi__value {
  color: oklch(0.5 0.17 149);
}
</style>
