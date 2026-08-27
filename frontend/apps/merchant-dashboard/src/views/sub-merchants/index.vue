<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 animate-fade-in-up">
      <div>
        <h1 class="text-2xl font-bold tracking-tight">{{ t('subMerchant.title') }}</h1>
        <p class="text-sm text-muted-foreground mt-1">{{ t('subMerchant.subtitle') }}</p>
      </div>
      <Button id="btn-create-sub-merchant" @click="showCreateDialog = true">
        <Plus class="h-4 w-4 mr-1.5" />
        {{ t('subMerchant.create') }}
      </Button>
    </div>

    <!-- Stats Summary Cards -->
    <div v-if="stats" class="grid grid-cols-2 md:grid-cols-4 gap-4 animate-fade-in-up">
      <Card v-for="card in statsCards" :key="card.label">
        <CardContent class="pt-5 pb-4 px-5">
          <p class="text-xs text-muted-foreground font-medium mb-1">{{ card.label }}</p>
          <p class="text-xl font-bold tabular-nums tracking-tight">{{ card.value }}</p>
        </CardContent>
      </Card>
    </div>

    <!-- Loading -->
    <div v-if="loading" class="flex items-center justify-center py-16">
      <div class="flex items-center gap-3 text-muted-foreground">
        <Loader2 class="h-5 w-5 animate-spin" />
        <span class="text-sm">{{ t('subMerchant.loading') }}</span>
      </div>
    </div>

    <!-- Table -->
    <Card v-else class="animate-fade-in-up delay-1">
      <CardContent class="pt-6">
        <div v-if="items.length" class="overflow-x-auto -mx-4 md:mx-0">
          <Table class="min-w-[600px]">
            <TableHeader>
              <TableRow>
                <TableHead>{{ t('subMerchant.colCode') }}</TableHead>
                <TableHead>{{ t('subMerchant.colName') }}</TableHead>
                <TableHead>{{ t('subMerchant.colStatus') }}</TableHead>
                <TableHead class="text-right">{{ t('subMerchant.colVolume') }}</TableHead>
                <TableHead class="text-right">{{ t('subMerchant.colTxCount') }}</TableHead>
                <TableHead class="text-right">{{ t('subMerchant.colActions') }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="item in items" :key="item.id">
                <TableCell>
                  <span class="sm-code">{{ item.sub_merchant_code }}</span>
                </TableCell>
                <TableCell class="font-medium">{{ item.display_name }}</TableCell>
                <TableCell>
                  <Badge :class="statusClass(item.status)">
                    {{ t(`subMerchant.status_${item.status}`) }}
                  </Badge>
                </TableCell>
                <TableCell class="text-right tabular-nums text-sm">
                  {{ getStatValue(item.sub_merchant_code, 'total_volume') }}
                </TableCell>
                <TableCell class="text-right tabular-nums text-sm">
                  {{ getStatValue(item.sub_merchant_code, 'total_transactions') }}
                </TableCell>
                <TableCell class="text-right">
                  <div class="flex items-center justify-end gap-1.5">
                    <Button
                      v-if="item.status === 'active'"
                      size="sm"
                      variant="outline"
                      :disabled="togglingId === item.sub_merchant_code"
                      @click="confirmToggle(item)"
                    >
                      <Loader2 v-if="togglingId === item.sub_merchant_code" class="h-3 w-3 animate-spin mr-1" />
                      <Pause v-else class="h-3 w-3 mr-1" />
                      {{ t('subMerchant.suspend') }}
                    </Button>
                    <Button
                      v-else
                      size="sm"
                      variant="outline"
                      :disabled="togglingId === item.sub_merchant_code"
                      @click="confirmToggle(item)"
                    >
                      <Loader2 v-if="togglingId === item.sub_merchant_code" class="h-3 w-3 animate-spin mr-1" />
                      <Play v-else class="h-3 w-3 mr-1" />
                      {{ t('subMerchant.reactivate') }}
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>

          <!-- Pagination -->
          <div class="flex items-center justify-between mt-4 px-1">
            <p class="text-xs text-muted-foreground">
              {{ t('subMerchant.totalCount', { count: total }) }}
            </p>
            <div class="flex items-center gap-2">
              <Button
                size="sm"
                variant="outline"
                :disabled="page <= 1"
                @click="page--; loadData()"
              >
                <ChevronLeft class="h-4 w-4" />
              </Button>
              <span class="text-sm tabular-nums text-muted-foreground">
                {{ page }} / {{ totalPages }}
              </span>
              <Button
                size="sm"
                variant="outline"
                :disabled="page >= totalPages"
                @click="page++; loadData()"
              >
                <ChevronRight class="h-4 w-4" />
              </Button>
            </div>
          </div>
        </div>

        <!-- Empty state -->
        <div v-else class="sm-empty">
          <Store class="h-10 w-10 text-muted-foreground/30 mb-3" />
          <p class="text-sm text-muted-foreground">{{ t('subMerchant.empty') }}</p>
          <Button variant="outline" size="sm" class="mt-3" @click="showCreateDialog = true">
            <Plus class="h-3.5 w-3.5 mr-1" />
            {{ t('subMerchant.createFirst') }}
          </Button>
        </div>
      </CardContent>
    </Card>

    <!-- Create Dialog -->
    <Dialog v-model:open="showCreateDialog">
      <DialogContent class="sm:max-w-[425px]">
        <DialogHeader>
          <DialogTitle>{{ t('subMerchant.createTitle') }}</DialogTitle>
          <DialogDescription>{{ t('subMerchant.createDesc') }}</DialogDescription>
        </DialogHeader>
        <div class="grid gap-4 py-4">
          <div class="grid gap-2">
            <Label for="sm-code">{{ t('subMerchant.codeLabel') }}</Label>
            <Input
              id="sm-code"
              v-model="createForm.code"
              :placeholder="t('subMerchant.codePlaceholder')"
              maxlength="100"
            />
            <p class="text-xs text-muted-foreground">{{ t('subMerchant.codeHint') }}</p>
          </div>
          <div class="grid gap-2">
            <Label for="sm-name">{{ t('subMerchant.nameLabel') }}</Label>
            <Input
              id="sm-name"
              v-model="createForm.displayName"
              :placeholder="t('subMerchant.namePlaceholder')"
              maxlength="200"
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" @click="showCreateDialog = false">
            {{ t('subMerchant.cancel') }}
          </Button>
          <Button
            id="btn-confirm-create"
            :disabled="creating || !createForm.code.trim() || !createForm.displayName.trim()"
            @click="handleCreate"
          >
            <Loader2 v-if="creating" class="h-4 w-4 animate-spin mr-1.5" />
            {{ t('subMerchant.confirm') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- Status Toggle Confirmation Dialog -->
    <Dialog v-model:open="showToggleDialog">
      <DialogContent class="sm:max-w-[400px]">
        <DialogHeader>
          <DialogTitle>
            {{ toggleTarget?.status === 'active' ? t('subMerchant.suspendTitle') : t('subMerchant.reactivateTitle') }}
          </DialogTitle>
          <DialogDescription>
            {{ toggleTarget?.status === 'active'
              ? t('subMerchant.suspendDesc', { name: toggleTarget?.display_name })
              : t('subMerchant.reactivateDesc', { name: toggleTarget?.display_name })
            }}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" @click="showToggleDialog = false">
            {{ t('subMerchant.cancel') }}
          </Button>
          <Button
            :variant="toggleTarget?.status === 'active' ? 'destructive' : 'default'"
            :disabled="togglingId !== null"
            @click="executeToggle"
          >
            <Loader2 v-if="togglingId !== null" class="h-4 w-4 animate-spin mr-1.5" />
            {{ toggleTarget?.status === 'active' ? t('subMerchant.suspendConfirm') : t('subMerchant.reactivateConfirm') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script lang="ts" setup>
import { ref, computed, onMounted, reactive } from 'vue';
import { useI18n } from 'vue-i18n';
import {
  Plus, Loader2, Store, Pause, Play,
  ChevronLeft, ChevronRight,
} from 'lucide-vue-next';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import {
  Dialog, DialogContent, DialogDescription,
  DialogFooter, DialogHeader, DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { toast } from 'vue-sonner';
import {
  listSubMerchants, createSubMerchant, updateSubMerchant,
  getSubMerchantStats,
} from '@/api/sub-merchant';
import type { SubMerchantItem, SubMerchantStatsResponse, SubMerchantStatsEntry } from '@/api/sub-merchant';

const { t } = useI18n();

// ── State ──
const loading = ref(true);
const items = ref<SubMerchantItem[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 20;
const stats = ref<SubMerchantStatsResponse | null>(null);

// Build code → stats map for O(1) lookup
const statsMap = computed(() => {
  const map = new Map<string, SubMerchantStatsEntry>();
  if (stats.value) {
    for (const entry of stats.value.sub_merchants) {
      map.set(entry.sub_merchant_code, entry);
    }
  }
  return map;
});

function getStatValue(code: string, field: keyof SubMerchantStatsEntry): string {
  const entry = statsMap.value.get(code);
  if (!entry) return '—';
  const val = entry[field];
  if (field === 'total_volume' || field === 'today_volume') {
    return `${val} USDT`;
  }
  return String(val);
}

const statsCards = computed(() => {
  if (!stats.value) return [];
  const s = stats.value.summary;
  return [
    { label: t('subMerchant.statsTotalVolume'), value: `${s.total_volume} USDT` },
    { label: t('subMerchant.statsTodayVolume'), value: `${s.today_volume} USDT` },
    { label: t('subMerchant.statsTotalTx'), value: String(s.total_transactions) },
    { label: t('subMerchant.statsTodayTx'), value: String(s.today_transactions) },
  ];
});

const totalPages = computed(() => Math.max(1, Math.ceil(total.value / pageSize)));

// Create dialog
const showCreateDialog = ref(false);
const creating = ref(false);
const createForm = reactive({ code: '', displayName: '' });

// Status toggle
const togglingId = ref<string | null>(null);
const showToggleDialog = ref(false);
const toggleTarget = ref<SubMerchantItem | null>(null);

// ── Helpers ──
function statusClass(status: string) {
  return status === 'active'
    ? 'sm-badge--active'
    : 'sm-badge--suspended';
}

// ── Data Loading ──
async function loadData() {
  loading.value = items.value.length === 0; // only show full loader on first load
  try {
    const [listRes, statsRes] = await Promise.all([
      listSubMerchants(page.value, pageSize),
      getSubMerchantStats(),
    ]);
    items.value = listRes.items;
    total.value = listRes.total;
    stats.value = statsRes;
  } catch {
    // error toast handled by interceptor
  } finally {
    loading.value = false;
  }
}

// ── Create ──
async function handleCreate() {
  if (!createForm.code.trim() || !createForm.displayName.trim()) return;
  creating.value = true;
  try {
    await createSubMerchant({
      sub_merchant_code: createForm.code.trim(),
      display_name: createForm.displayName.trim(),
    });
    toast.success(t('subMerchant.createSuccess'));
    showCreateDialog.value = false;
    createForm.code = '';
    createForm.displayName = '';
    page.value = 1;
    await loadData();
  } catch {
    // error toast handled by interceptor
  } finally {
    creating.value = false;
  }
}

// ── Status Toggle (with confirmation) ──
function confirmToggle(item: SubMerchantItem) {
  toggleTarget.value = item;
  showToggleDialog.value = true;
}

async function executeToggle() {
  if (!toggleTarget.value) return;
  const item = toggleTarget.value;
  const newStatus = item.status === 'active' ? 'suspended' : 'active';
  togglingId.value = item.sub_merchant_code;
  try {
    await updateSubMerchant(item.sub_merchant_code, { status: newStatus });
    toast.success(
      newStatus === 'suspended'
        ? t('subMerchant.suspendSuccess')
        : t('subMerchant.reactivateSuccess'),
    );
    showToggleDialog.value = false;
    toggleTarget.value = null;
    await loadData();
  } catch {
    // error toast handled by interceptor
  } finally {
    togglingId.value = null;
  }
}

onMounted(() => loadData());
</script>

<style scoped>
.sm-code {
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 0.8rem;
  font-weight: 600;
  padding: 0.125rem 0.5rem;
  border-radius: 0.25rem;
  background: oklch(0.97 0.005 264);
  border: 1px solid oklch(0.92 0.005 264);
  color: oklch(0.35 0.015 264);
}

.sm-badge--active {
  background: oklch(0.627 0.194 149.214 / 10%) !important;
  color: oklch(0.45 0.15 149) !important;
  border-color: oklch(0.627 0.194 149.214 / 20%) !important;
}

.sm-badge--suspended {
  background: oklch(0.65 0.18 25 / 10%) !important;
  color: oklch(0.5 0.15 25) !important;
  border-color: oklch(0.65 0.18 25 / 20%) !important;
}

.sm-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 3rem 1rem;
}
</style>
