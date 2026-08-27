<template>
  <div class="space-y-6">
    <!-- Balance Hero -->
    <div class="dash-balance-hero animate-fade-in-up">
      <div class="dash-balance-hero__content">
        <div>
          <p class="dash-balance-label">{{ t('dashboard.availableBalance') }}</p>
          <div class="dash-balance-tokens">
            <div class="dash-token-group">
              <span class="dash-balance-amount">{{ formatAmount(userStore.balance || 0) }}</span>
              <span class="dash-token-badge dash-token-badge--usdt">USDT</span>
            </div>
            <div v-if="(userStore.usdcBalance || 0) > 0" class="dash-token-group">
              <span class="dash-balance-amount">{{ formatAmount(userStore.usdcBalance || 0) }}</span>
              <span class="dash-token-badge dash-token-badge--usdc">USDC</span>
            </div>
          </div>
        </div>
        <Button class="dash-withdraw-btn" @click="$router.push({ name: 'Funds' })">
          <ArrowUpRight class="h-4 w-4 mr-1.5" />
          {{ t('billing.withdraw') }}
        </Button>
      </div>
      <div class="dash-balance-hero__glow" />
    </div>

    <!-- Pending Approval Alert -->
    <div
      v-if="userStore.pendingApprovalCount > 0"
      class="dash-pending-alert animate-fade-in-up"
    >
      <div class="flex items-center gap-3">
        <div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-amber-100 dark:bg-amber-900/30">
          <AlertCircle class="h-4 w-4 text-amber-600 dark:text-amber-400" />
        </div>
        <div class="flex-1 min-w-0">
          <p class="text-sm font-semibold text-amber-800 dark:text-amber-200">
            {{ t('dashboard.pendingApprovals', { count: userStore.pendingApprovalCount }) }}
          </p>
          <p class="text-xs text-amber-600/80 dark:text-amber-400/70 mt-0.5">
            {{ t('dashboard.pendingApprovalsHint') }}
          </p>
        </div>
        <Button
          size="sm"
          variant="outline"
          class="shrink-0 border-amber-300 text-amber-700 hover:bg-amber-50 dark:border-amber-700 dark:text-amber-300 dark:hover:bg-amber-900/20"
          @click="$router.push({ name: 'Payouts', query: { status: 'PendingApproval' } })"
        >
          {{ t('dashboard.reviewNow') }}
          <ArrowRight class="h-3.5 w-3.5 ml-1" />
        </Button>
      </div>
    </div>

    <!-- ═══ Getting Started (Empty State) ═══ -->
    <Card v-if="showOnboarding" class="animate-fade-in-up border-dashed border-blue-200 dark:border-blue-800 bg-gradient-to-br from-blue-50/50 to-indigo-50/30 dark:from-blue-950/20 dark:to-indigo-950/10">
      <CardHeader>
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <div class="flex h-8 w-8 items-center justify-center rounded-lg bg-blue-100 dark:bg-blue-900/40">
              <Rocket class="h-4 w-4 text-blue-600 dark:text-blue-400" />
            </div>
            <CardTitle class="text-base">{{ t('dashboard.onboarding.title') }}</CardTitle>
          </div>
          <Button variant="ghost" size="sm" class="text-xs text-muted-foreground" @click="dismissOnboarding">
            {{ t('dashboard.onboarding.dismiss') }}
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <div class="grid gap-3 sm:grid-cols-3">
          <!-- Step 1: API Key -->
          <div class="onboarding-step group" @click="$router.push({ name: 'ApiKeys' })">
            <div class="onboarding-step__num">1</div>
            <div>
              <p class="text-sm font-medium">{{ t('dashboard.onboarding.step1Title') }}</p>
              <p class="text-xs text-muted-foreground mt-0.5">{{ t('dashboard.onboarding.step1Desc') }}</p>
            </div>
            <ArrowRight class="h-3.5 w-3.5 text-muted-foreground/50 ml-auto shrink-0 group-hover:text-blue-500 transition-colors" />
          </div>
          <!-- Step 2: Webhook -->
          <div class="onboarding-step group" @click="$router.push({ name: 'Developer' })">
            <div class="onboarding-step__num">2</div>
            <div>
              <p class="text-sm font-medium">{{ t('dashboard.onboarding.step2Title') }}</p>
              <p class="text-xs text-muted-foreground mt-0.5">{{ t('dashboard.onboarding.step2Desc') }}</p>
            </div>
            <ArrowRight class="h-3.5 w-3.5 text-muted-foreground/50 ml-auto shrink-0 group-hover:text-blue-500 transition-colors" />
          </div>
          <!-- Step 3: First Payment -->
          <a :href="quickstartUrl" target="_blank" rel="noopener" class="onboarding-step group">
            <div class="onboarding-step__num">3</div>
            <div>
              <p class="text-sm font-medium">{{ t('dashboard.onboarding.step3Title') }}</p>
              <p class="text-xs text-muted-foreground mt-0.5">{{ t('dashboard.onboarding.step3Desc') }}</p>
            </div>
            <ExternalLink class="h-3.5 w-3.5 text-muted-foreground/50 ml-auto shrink-0 group-hover:text-blue-500 transition-colors" />
          </a>
        </div>
      </CardContent>
    </Card>

    <!-- ═══ Analytics Section ═══ -->

    <!-- Time Range Selector -->
    <div class="flex items-center justify-between animate-fade-in-up delay-1">
      <div class="flex items-center gap-3">
        <h2 class="text-lg font-semibold text-foreground/90">{{ t('dashboard.analyticsTitle') }}</h2>
        <Select v-if="hasSubMerchants" v-model="smSelected" @update:modelValue="fetchAnalytics" class="w-40">
          <SelectTrigger class="h-8 text-xs [&>span]:truncate">
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
      <div class="analytics-range-group">
        <button
          v-for="r in ranges"
          :key="r.key"
          :class="['analytics-range-btn', { 'analytics-range-btn--active': activeRange === r.key }]"
          @click="setRange(r.key)"
        >
          {{ r.label }}
        </button>
      </div>
    </div>

    <!-- KPI Cards -->
    <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4 animate-fade-in-up delay-1">
      <div class="kpi-card kpi-card--volume">
        <div class="kpi-card__icon"><TrendingUp class="h-4 w-4" /></div>
        <p class="kpi-card__label">{{ t('dashboard.grossVolume') }}</p>
        <p class="kpi-card__value">${{ formatMicro(totalGrossVolume) }}</p>
      </div>
      <div class="kpi-card kpi-card--revenue">
        <div class="kpi-card__icon"><DollarSign class="h-4 w-4" /></div>
        <p class="kpi-card__label">{{ t('dashboard.netRevenue') }}</p>
        <p class="kpi-card__value">${{ formatMicro(totalNetRevenue) }}</p>
      </div>
      <div class="kpi-card kpi-card--count">
        <div class="kpi-card__icon"><Hash class="h-4 w-4" /></div>
        <p class="kpi-card__label">{{ t('dashboard.txCount') }}</p>
        <p class="kpi-card__value">{{ totalTxCount }}</p>
      </div>
      <div class="kpi-card kpi-card--rate">
        <div class="kpi-card__icon"><Percent class="h-4 w-4" /></div>
        <p class="kpi-card__label">{{ t('dashboard.conversionRate') }}</p>
        <p class="kpi-card__value">{{ (analytics.conversion_rate * 100).toFixed(1) }}%</p>
      </div>
    </div>

    <!-- Charts Row -->
    <div class="grid gap-5 lg:grid-cols-3 animate-fade-in-up delay-2">
      <!-- Revenue Trend (2/3) -->
      <Card class="lg:col-span-2">
        <CardHeader>
          <CardTitle class="text-sm font-medium">{{ t('dashboard.revenueTrend') }}</CardTitle>
        </CardHeader>
        <CardContent>
          <div v-if="analytics.time_series.length === 0" class="flex items-center justify-center h-[260px] text-muted-foreground text-sm">
            {{ t('dashboard.noData') }}
          </div>
          <v-chart v-else :option="trendOption" :style="{ height: '260px' }" autoresize />
        </CardContent>
      </Card>

      <!-- Network Distribution (1/3) -->
      <Card>
        <CardHeader>
          <CardTitle class="text-sm font-medium">{{ t('dashboard.networkDist') }}</CardTitle>
        </CardHeader>
        <CardContent>
          <div v-if="analytics.network_distribution.length === 0" class="flex items-center justify-center h-[260px] text-muted-foreground text-sm">
            {{ t('dashboard.noData') }}
          </div>
          <v-chart v-else :option="networkOption" :style="{ height: '260px' }" autoresize />
        </CardContent>
      </Card>
    </div>

    <!-- Status Breakdown (horizontal bar) -->
    <Card class="animate-fade-in-up delay-3" v-if="analytics.status_breakdown.length > 0">
      <CardHeader>
        <CardTitle class="text-sm font-medium">{{ t('dashboard.statusBreakdown') }}</CardTitle>
      </CardHeader>
      <CardContent>
        <div class="status-breakdown">
          <div
            v-for="s in analytics.status_breakdown"
            :key="s.label"
            class="status-breakdown__item"
          >
            <div class="flex items-center gap-2 min-w-[100px]">
              <span :class="['status-dot', `status-dot--${s.label.toLowerCase()}`]" />
              <span class="text-sm text-muted-foreground">{{ s.label }}</span>
            </div>
            <div class="status-breakdown__bar-track">
              <div
                class="status-breakdown__bar-fill"
                :class="`status-bar--${s.label.toLowerCase()}`"
                :style="{ width: statusPct(s.value) + '%' }"
              />
            </div>
            <span class="text-sm font-semibold tabular-nums min-w-[40px] text-right">{{ s.value }}</span>
          </div>
        </div>
      </CardContent>
    </Card>

    <!-- Recent Transactions -->
    <Card class="animate-fade-in-up delay-3">
      <CardHeader>
        <div class="flex items-center justify-between">
          <CardTitle>{{ t('dashboard.recentTransactions') }}</CardTitle>
          <Button variant="ghost" size="sm" class="text-muted-foreground hover:text-foreground" @click="$router.push({ name: 'Sessions' })">
            {{ t('dashboard.viewAll') }}
            <ArrowRight class="h-3.5 w-3.5 ml-1" />
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <div class="overflow-x-auto -mx-4 md:mx-0">
        <Table class="min-w-[400px]">
          <TableHeader>
            <TableRow>
              <TableHead>{{ t('table.status') }}</TableHead>
              <TableHead>{{ t('table.amount') }}</TableHead>
              <TableHead class="hidden md:table-cell">{{ t('table.refId') }}</TableHead>
              <TableHead class="text-right">{{ t('table.time') }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow
              v-for="row in recentSessions"
              :key="row.id"
              class="group transition-colors"
            >
              <TableCell>
                <div class="flex items-center gap-2">
                  <span :class="['status-dot', `status-dot--${row.status.toLowerCase()}`]" />
                  <Badge :variant="getStatusVariant(row.status)">
                    {{ row.status }}
                  </Badge>
                </div>
              </TableCell>
              <TableCell class="font-medium tabular-nums">
                <span :class="['Paid','Overpaid'].includes(row.status) ? 'text-emerald-600' : 'text-muted-foreground'">{{ ['Paid','Overpaid'].includes(row.status) ? '+' : '' }} {{ fmtAmt(row.amount) }} {{ row.currency || 'USDT' }}</span>
              </TableCell>
              <TableCell class="hidden md:table-cell text-muted-foreground font-mono text-xs">
                {{ row.clientReferenceId || row.id.slice(0, 8) }}
              </TableCell>
              <TableCell class="text-right text-xs text-muted-foreground">
                {{ formatTime(row.createdTime) }}
              </TableCell>
            </TableRow>
            <TableRow v-if="recentSessions.length === 0">
              <TableCell colspan="4" class="text-center text-muted-foreground py-12">
                <div class="flex flex-col items-center gap-2">
                  <Inbox class="h-8 w-8 text-muted-foreground/30" />
                  <span>{{ t('dashboard.noRecentTransactions') }}</span>
                </div>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
        </div>
      </CardContent>
    </Card>
  </div>
</template>

<script lang="ts" setup>
import { ref, reactive, computed, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { useUserStore } from '@/stores';
import { queryDashboardStats, type DashboardStats } from '@/api/dashboard';
import { queryAnalytics, type AnalyticsResponse } from '@/api/analytics';
import { querySessionList, type SessionRecord } from '@/api/session';
import { fmtAmt } from '@/utils/currency';
import { useSmartPolling } from '@/composables/useSmartPolling';
import dayjs from 'dayjs';
import relativeTime from 'dayjs/plugin/relativeTime';
import utc from 'dayjs/plugin/utc';
import VChart from 'vue-echarts';
import { use } from 'echarts/core';
import { BarChart, PieChart } from 'echarts/charts';
import {
  GridComponent,
  TooltipComponent,
  LegendComponent,
} from 'echarts/components';
import { CanvasRenderer } from 'echarts/renderers';
import {
  TrendingUp, Hash, ArrowRight, ArrowUpRight, Inbox, AlertCircle,
  DollarSign, Percent, Rocket, ExternalLink,
} from 'lucide-vue-next';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Card, CardContent, CardHeader, CardTitle,
} from '@/components/ui/card';
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from '@/components/ui/table';
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select';
import { useSubMerchantFilter } from '@/composables/useSubMerchantFilter';

// Register ECharts modules (tree-shakeable)
use([BarChart, PieChart, GridComponent, TooltipComponent, LegendComponent, CanvasRenderer]);

dayjs.extend(relativeTime);
dayjs.extend(utc);

const { t, locale } = useI18n();
const userStore = useUserStore();
const stats = reactive<Partial<DashboardStats>>({});
const recentSessions = ref<SessionRecord[]>([]);

// ── Sub-merchant filter ──
const smFilter = useSubMerchantFilter();
const smSelected = smFilter.selected;
const smList = smFilter.subMerchants;
const hasSubMerchants = smFilter.hasSubMerchants;
smFilter.loadSubMerchants();

// ── Onboarding ──
const onboardingDismissed = ref(localStorage.getItem('ironix_onboarding_dismissed') === '1');
const showOnboarding = computed(() =>
  !onboardingDismissed.value && recentSessions.value.length === 0 && totalTxCount.value === 0
);
const dismissOnboarding = () => {
  onboardingDismissed.value = true;
  localStorage.setItem('ironix_onboarding_dismissed', '1');
};
const quickstartUrl = computed(() =>
  locale.value === 'zh-CN'
    ? 'https://ironixpay.com/guide/quickstart'
    : 'https://ironixpay.com/en/guide/quickstart'
);

// ── Analytics state ──
const analytics = reactive<AnalyticsResponse>({
  kpis: [],
  time_series: [],
  network_distribution: [],
  status_breakdown: [],
  conversion_rate: 0,
});

type RangeKey = 'today' | '7d' | '30d' | 'all';
const activeRange = ref<RangeKey>('30d');

const ranges = computed(() => [
  { key: 'today' as RangeKey, label: t('dashboard.rangeToday') },
  { key: '7d' as RangeKey, label: t('dashboard.range7d') },
  { key: '30d' as RangeKey, label: t('dashboard.range30d') },
  { key: 'all' as RangeKey, label: t('dashboard.rangeAll') },
]);

function getDateRange(key: RangeKey): { start_date?: string; end_date?: string } {
  const now = dayjs().utc();
  const end = now.endOf('day').toISOString();
  switch (key) {
    case 'today': return { start_date: now.startOf('day').toISOString(), end_date: end };
    case '7d':    return { start_date: now.subtract(7, 'day').startOf('day').toISOString(), end_date: end };
    case '30d':   return { start_date: now.subtract(30, 'day').startOf('day').toISOString(), end_date: end };
    case 'all':   return {};
  }
}

function setRange(key: RangeKey) {
  activeRange.value = key;
}

// ── Computed KPI aggregations ──
const totalGrossVolume = computed(() => analytics.kpis.reduce((s, k) => s + k.gross_volume, 0));
const totalNetRevenue = computed(() => analytics.kpis.reduce((s, k) => s + k.net_revenue, 0));
const totalTxCount = computed(() => analytics.kpis.reduce((s, k) => s + k.tx_count, 0));

// ── Chart options ──
const trendOption = computed(() => {
  // Group time series by date, merging currencies
  const dateMap = new Map<string, { volume: number; count: number }>();
  for (const pt of analytics.time_series) {
    const existing = dateMap.get(pt.date) || { volume: 0, count: 0 };
    existing.volume += pt.volume;
    existing.count += pt.count;
    dateMap.set(pt.date, existing);
  }
  const dates = [...dateMap.keys()].sort();
  const dataPoints = dates.map(d => [
    d, // ISO date string, ECharts time axis parses this automatically
    Number((dateMap.get(d)!.volume / 1_000_000).toFixed(2)),
  ]);

  return {
    grid: { left: 50, right: 20, top: 20, bottom: 30 },
    tooltip: {
      trigger: 'axis',
      backgroundColor: 'rgba(255,255,255,0.95)',
      borderColor: '#e5e7eb',
      textStyle: { color: '#374151', fontSize: 12 },
      formatter: (params: any) => {
        const p = params[0];
        const date = dayjs(p.value[0]).format('YYYY-MM-DD');
        return `<b>${date}</b><br/>Volume: $${Number(p.value[1]).toLocaleString()}`;
      },
    },
    xAxis: {
      type: 'time',
      axisLine: { lineStyle: { color: '#e5e7eb' } },
      axisLabel: {
        color: '#9ca3af',
        fontSize: 11,
        formatter: (value: number) => dayjs(value).format('MM/DD'),
      },
    },
    yAxis: {
      type: 'value',
      axisLabel: {
        color: '#9ca3af',
        fontSize: 11,
        formatter: (v: number) => v >= 1000 ? `$${(v / 1000).toFixed(0)}k` : `$${v}`,
      },
      splitLine: { lineStyle: { color: '#f3f4f6' } },
    },
    series: [{
      type: 'bar',
      data: dataPoints,
      itemStyle: {
        color: {
          type: 'linear',
          x: 0, y: 0, x2: 0, y2: 1,
          colorStops: [
            { offset: 0, color: '#6366f1' },
            { offset: 1, color: '#a5b4fc' },
          ],
        },
        borderRadius: [4, 4, 0, 0],
      },
      barMaxWidth: 32,
    }],
  };
});

const networkColors = ['#6366f1', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#ec4899', '#06b6d4'];

const networkOption = computed(() => ({
  tooltip: {
    trigger: 'item',
    backgroundColor: 'rgba(255,255,255,0.95)',
    borderColor: '#e5e7eb',
    textStyle: { color: '#374151', fontSize: 12 },
    formatter: (p: any) => `<b>${p.name}</b><br/>${p.value} txns (${p.percent}%)`,
  },
  legend: {
    orient: 'vertical',
    left: 'left',
    top: 'center',
    itemWidth: 10,
    itemHeight: 10,
    itemGap: 8,
    textStyle: { fontSize: 11, color: '#6b7280' },
  },
  series: [{
    type: 'pie',
    radius: ['40%', '70%'],
    center: ['62%', '50%'],
    avoidLabelOverlap: true,
    label: { show: false },
    emphasis: {
      label: { show: false },
      itemStyle: { shadowBlur: 10, shadowOffsetX: 0, shadowColor: 'rgba(0,0,0,0.15)' },
    },
    data: analytics.network_distribution.map((d, i) => ({
      name: d.label,
      value: d.value,
      itemStyle: { color: networkColors[i % networkColors.length] },
    })),
  }],
}));

// ── Status breakdown helpers ──
const statusTotal = computed(() => analytics.status_breakdown.reduce((s, b) => s + b.value, 0));
const statusPct = (v: number) => statusTotal.value > 0 ? (v / statusTotal.value) * 100 : 0;

// ── Data fetching ──
const fetchStats = async () => {
  try {
    const data = await queryDashboardStats();
    Object.assign(stats, data);
  } catch (err) {
    console.error(err);
  }
};

const fetchAnalytics = async () => {
  try {
    const range = getDateRange(activeRange.value);
    const data = await queryAnalytics({ ...range, ...smFilter.filterParams.value });
    Object.assign(analytics, data);
  } catch (err) {
    console.error('Analytics fetch error:', err);
  }
};

const fetchRecentSessions = async () => {
  try {
    const { list } = await querySessionList({ current: 1, pageSize: 5 });
    recentSessions.value = list;
  } catch (err) {
    console.error(err);
  }
};

// Refetch analytics when range changes
watch(activeRange, () => fetchAnalytics());



useSmartPolling(async () => {
  await Promise.all([fetchStats(), fetchAnalytics(), fetchRecentSessions()]);
});

// ── Helpers ──
const formatAmount = (val: number | string) =>
  Number(val).toLocaleString('en-US', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 6,
  });

/** Convert i64 microunits to display string */
const formatMicro = (micro: number) =>
  (micro / 1_000_000).toLocaleString('en-US', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });

const formatTime = (time: string) => dayjs(time).fromNow();

const getStatusVariant = (status: string) => {
  const map: Record<string, 'default' | 'secondary' | 'destructive' | 'outline'> = {
    Paid: 'default',
    Pending: 'secondary',
    Expired: 'outline',
    Underpaid: 'destructive',
    Overpaid: 'default',
  };
  return map[status] || 'secondary';
};
</script>

<style scoped>
/* ── Balance Hero ── */
.dash-balance-hero {
  position: relative;
  overflow: hidden;
  border-radius: 0.75rem;
  border: 1px solid oklch(0.546 0.245 262.881 / 15%);
  padding: 1.5rem 1.75rem;
  background: linear-gradient(135deg, oklch(1 0 0), oklch(0.97 0.012 262));
  box-shadow: 0 1px 3px oklch(0 0 0 / 4%), 0 0 0 1px oklch(0 0 0 / 2%);
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}
.dash-balance-hero:hover {
  box-shadow: 0 6px 20px oklch(0.546 0.245 262.881 / 8%), 0 0 0 1px oklch(0.546 0.245 262.881 / 10%);
  transform: translateY(-1px);
}
.dash-balance-hero__content {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
@media (min-width: 640px) {
  .dash-balance-hero__content {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    gap: 0;
  }
}
.dash-balance-hero__glow {
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
.dash-balance-hero:hover .dash-balance-hero__glow {
  opacity: 0.14;
}
.dash-balance-label {
  font-size: 0.7rem;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: oklch(0.50 0.015 264);
  margin-bottom: 0.375rem;
}

/* ── Token Groups ── */
.dash-balance-tokens {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem 1.5rem;
  align-items: baseline;
}
.dash-token-group {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
}
.dash-balance-amount {
  font-size: 1.75rem;
  font-weight: 700;
  line-height: 1;
  letter-spacing: -0.025em;
  font-variant-numeric: tabular-nums;
  color: oklch(0.18 0.014 265.2);
}
.dash-balance-amount--secondary {
  font-size: 1.5rem;
}
@media (min-width: 640px) {
  .dash-balance-amount {
    font-size: 2.25rem;
  }
  .dash-balance-amount--secondary {
    font-size: 1.75rem;
  }
}
.dash-token-badge {
  display: inline-flex;
  align-items: center;
  padding: 0.125rem 0.5rem;
  border-radius: 100px;
  font-size: 0.625rem;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}
.dash-token-badge--usdt {
  background: oklch(0.87 0.12 158 / 18%);
  color: oklch(0.45 0.12 158);
}
.dash-token-badge--usdc {
  background: oklch(0.87 0.14 250 / 18%);
  color: oklch(0.45 0.14 250);
}
.dash-withdraw-btn {
  flex-shrink: 0;
}

/* ── Analytics Range Selector ── */
.analytics-range-group {
  display: flex;
  gap: 2px;
  border-radius: 0.5rem;
  background: oklch(0.95 0.005 264);
  padding: 2px;
  border: 1px solid oklch(0.9 0.005 264);
}
.analytics-range-btn {
  padding: 0.25rem 0.75rem;
  border-radius: 0.375rem;
  font-size: 0.75rem;
  font-weight: 500;
  color: oklch(0.50 0.015 264);
  transition: all 0.15s ease;
  cursor: pointer;
  border: none;
  background: transparent;
}
.analytics-range-btn:hover {
  color: oklch(0.30 0.02 264);
}
.analytics-range-btn--active {
  background: oklch(1 0 0);
  color: oklch(0.18 0.014 265.2);
  box-shadow: 0 1px 2px oklch(0 0 0 / 6%);
  font-weight: 600;
}

/* ── KPI Cards ── */
.kpi-card {
  position: relative;
  border-radius: 0.75rem;
  border: 1px solid oklch(0.9 0.005 264);
  padding: 1.125rem 1.25rem;
  background: oklch(1 0 0);
  transition: all 0.2s ease;
  overflow: hidden;
}
.kpi-card:hover {
  box-shadow: 0 4px 12px oklch(0 0 0 / 6%);
  transform: translateY(-1px);
}
.kpi-card__icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 2rem;
  height: 2rem;
  border-radius: 0.5rem;
  margin-bottom: 0.75rem;
}
.kpi-card--volume .kpi-card__icon {
  background: oklch(0.627 0.194 149.214 / 12%);
  color: oklch(0.5 0.17 149);
}
.kpi-card--revenue .kpi-card__icon {
  background: oklch(0.546 0.245 262.881 / 12%);
  color: oklch(0.546 0.245 262.881);
}
.kpi-card--count .kpi-card__icon {
  background: oklch(0.769 0.188 70.08 / 12%);
  color: oklch(0.65 0.17 60);
}
.kpi-card--rate .kpi-card__icon {
  background: oklch(0.6 0.24 10 / 12%);
  color: oklch(0.55 0.22 10);
}
.kpi-card__label {
  font-size: 0.65rem;
  font-weight: 600;
  letter-spacing: 0.05em;
  text-transform: uppercase;
  color: oklch(0.50 0.015 264);
  margin-bottom: 0.25rem;
}
.kpi-card__value {
  font-size: 1.5rem;
  font-weight: 700;
  line-height: 1;
  letter-spacing: -0.02em;
  font-variant-numeric: tabular-nums;
  color: oklch(0.18 0.014 265.2);
}

/* ── Status Breakdown ── */
.status-breakdown {
  display: flex;
  flex-direction: column;
  gap: 0.625rem;
}
.status-breakdown__item {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}
.status-breakdown__bar-track {
  flex: 1;
  height: 8px;
  border-radius: 4px;
  background: oklch(0.95 0.005 264);
  overflow: hidden;
}
.status-breakdown__bar-fill {
  height: 100%;
  border-radius: 4px;
  transition: width 0.5s ease;
}
.status-bar--paid { background: oklch(0.627 0.194 149.214); }
.status-bar--overpaid { background: oklch(0.546 0.245 262.881); }
.status-bar--expired { background: oklch(0.65 0.15 55); }
.status-bar--underpaid { background: oklch(0.7 0.2 25); }
.status-bar--blocked { background: oklch(0.6 0.24 10); }

/* ── Pending Approval Alert ── */
.dash-pending-alert {
  border-radius: 0.75rem;
  border: 1px solid oklch(0.769 0.188 70.08 / 30%);
  padding: 0.875rem 1.25rem;
  background: oklch(0.98 0.015 80);
}
:root.dark .dash-pending-alert {
  background: oklch(0.25 0.02 60 / 40%);
  border-color: oklch(0.6 0.15 60 / 25%);
}

/* ── Onboarding Steps ── */
.onboarding-step {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.875rem 1rem;
  border-radius: 0.5rem;
  border: 1px solid oklch(0.9 0.005 264);
  background: oklch(1 0 0 / 80%);
  cursor: pointer;
  transition: all 0.2s ease;
  text-decoration: none;
  color: inherit;
}
.onboarding-step:hover {
  border-color: oklch(0.546 0.245 262.881 / 30%);
  box-shadow: 0 2px 8px oklch(0.546 0.245 262.881 / 8%);
  transform: translateY(-1px);
}
.onboarding-step__num {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 1.5rem;
  height: 1.5rem;
  border-radius: 50%;
  background: oklch(0.546 0.245 262.881 / 10%);
  color: oklch(0.546 0.245 262.881);
  font-size: 0.7rem;
  font-weight: 700;
  flex-shrink: 0;
}
:root.dark .onboarding-step {
  background: oklch(0.2 0.005 264 / 60%);
  border-color: oklch(0.35 0.01 264);
}
:root.dark .onboarding-step:hover {
  border-color: oklch(0.546 0.245 262.881 / 40%);
}
</style>
