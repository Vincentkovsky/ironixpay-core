<template>
  <div class="space-y-6">
    <Card class="animate-fade-in-up">
      <CardHeader>
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <div class="stat-icon-box h-8 w-8 flex items-center justify-center rounded-md">
              <KeyRound class="h-4 w-4 text-brand" />
            </div>
            <CardTitle>{{ t('apiKeys.title') }}</CardTitle>
          </div>
          <Button size="sm" @click="createOpen = true">
            <Plus class="h-3.5 w-3.5 mr-1.5" />
            {{ t('apiKeys.create') }}
          </Button>
        </div>
      </CardHeader>

      <!-- Developer Resources Banner -->
      <div class="mx-6 mb-4 rounded-lg bg-blue-50 dark:bg-blue-950/30 border border-blue-100 dark:border-blue-900 px-4 py-3 flex items-start gap-3">
        <BookOpen class="h-4 w-4 text-blue-600 dark:text-blue-400 mt-0.5 shrink-0" />
        <div class="flex-1 text-sm">
          <span class="text-blue-800 dark:text-blue-300 font-medium">{{ t('apiKeys.docsHint') }}</span>
          <div class="flex items-center gap-4 mt-1.5">
            <a :href="guideUrl" target="_blank" rel="noopener"
               class="inline-flex items-center gap-1 text-xs font-medium text-blue-600 dark:text-blue-400 hover:underline">
              <ExternalLink class="h-3 w-3" />
              {{ t('apiKeys.quickStart') }}
            </a>
            <a href="https://api.ironixpay.com/docs" target="_blank" rel="noopener"
               class="inline-flex items-center gap-1 text-xs font-medium text-blue-600 dark:text-blue-400 hover:underline">
              <ExternalLink class="h-3 w-3" />
              {{ t('apiKeys.apiReference') }}
            </a>
          </div>
        </div>
      </div>

      <CardContent>
        <!-- Loading -->
        <div v-if="loading" class="text-sm text-muted-foreground py-4">{{ t('apiKeys.loading') }}</div>

        <!-- Empty -->
        <div v-else-if="keys.length === 0" class="flex flex-col items-center gap-2 py-12 text-muted-foreground">
          <KeyRound class="h-8 w-8 text-muted-foreground/30" />
          <span>{{ t('apiKeys.noKeys') }}</span>
        </div>

        <!-- Key list -->
        <Table v-else>
          <TableHeader>
            <TableRow>
              <TableHead>{{ t('table.name') }}</TableHead>
              <TableHead>{{ t('table.key') }}</TableHead>
              <TableHead>{{ t('table.created') }}</TableHead>
              <TableHead>{{ t('table.lastUsed') }}</TableHead>
              <TableHead class="w-[80px]"></TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="k in keys" :key="k.id" class="transition-colors">
              <TableCell class="font-medium">{{ k.name }}</TableCell>
              <TableCell>
                <div class="flex items-center gap-2">
                  <code class="text-xs bg-muted px-2 py-1 rounded font-mono">
                    {{ k.prefix }}••••••••
                  </code>
                </div>
              </TableCell>
              <TableCell class="text-sm text-muted-foreground">
                {{ formatDate(k.created_at) }}
              </TableCell>
              <TableCell class="text-sm text-muted-foreground">
                {{ k.last_used_at ? formatDate(k.last_used_at) : t('apiKeys.never') }}
              </TableCell>
              <TableCell>
                <Button variant="ghost" size="sm" class="text-destructive hover:text-destructive" @click="handleRevoke(k.id)">
                  <Trash2 class="h-3.5 w-3.5" />
                </Button>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </CardContent>
    </Card>

    <!-- Create Key Dialog -->
    <Dialog v-model:open="createOpen">
      <DialogContent class="max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t('apiKeys.createTitle') }}</DialogTitle>
        </DialogHeader>
        <form @submit.prevent="handleCreate" class="space-y-4">
          <div class="space-y-1.5">
            <Label>{{ t('apiKeys.keyName') }}</Label>
            <Input v-model="newKeyName" :placeholder="t('apiKeys.keyNamePlaceholder')" />
          </div>
          <div class="flex justify-end gap-2">
            <Button type="button" variant="outline" @click="createOpen = false">
              {{ t('apiKeys.cancel') }}
            </Button>
            <Button type="submit" :disabled="creating">
              {{ creating ? t('apiKeys.creating') : t('apiKeys.create') }}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>

    <!-- Created Key Display Dialog -->
    <Dialog v-model:open="createdOpen">
      <DialogContent class="max-w-lg">
        <DialogHeader>
          <DialogTitle>{{ t('apiKeys.createdTitle') }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-3">
          <p class="text-sm text-orange-600 bg-orange-50 border border-orange-200 rounded-md px-3 py-2">
            ⚠️ {{ t('apiKeys.createdWarning') }}
          </p>
          <div class="flex items-center gap-2">
            <code class="flex-1 text-xs bg-muted px-3 py-2 rounded font-mono break-all select-all">{{ createdKey }}</code>
            <Button variant="outline" size="icon" class="shrink-0" @click="copyKey(createdKey)">
              <Copy class="h-3.5 w-3.5" />
            </Button>
          </div>
          <div class="flex justify-end">
            <Button @click="createdOpen = false">{{ t('apiKeys.done') }}</Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script lang="ts" setup>
import { computed, ref, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { BookOpen, Copy, ExternalLink, KeyRound, Plus, Trash2 } from 'lucide-vue-next';
import { useClipboard } from '@vueuse/core';
import { toast } from 'vue-sonner';
import dayjs from 'dayjs';
import { queryApiKeys, createApiKey, revokeApiKey, type ApiKey } from '@/api/developer';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from '@/components/ui/table';

const { t, locale } = useI18n();
const { copy } = useClipboard();

const guideUrl = computed(() =>
  locale.value === 'zh-CN'
    ? 'https://ironixpay.com/guide/quickstart'
    : 'https://ironixpay.com/en/guide/quickstart'
);
const keys = ref<ApiKey[]>([]);
const loading = ref(true);

// Create key state
const createOpen = ref(false);
const newKeyName = ref('');
const creating = ref(false);
const createdOpen = ref(false);
const createdKey = ref('');

const fetchKeys = async () => {
  loading.value = true;
  try {
    const res = await queryApiKeys();
    keys.value = (res as any).keys || [];
  } catch { /* */ }
  finally { loading.value = false; }
};

const copyKey = (k: string) => {
  if (!k) return;
  copy(k);
  toast.success(t('apiKeys.copied'));
};

const formatDate = (d: string) => dayjs(d).format('YYYY-MM-DD HH:mm');

async function handleCreate() {
  creating.value = true;
  try {
    const res = await createApiKey({
      name: newKeyName.value.trim() || undefined,
    });
    createOpen.value = false;
    newKeyName.value = '';
    createdKey.value = res.key;
    createdOpen.value = true;
    await fetchKeys();
  } catch {
    // interceptor shows backend error
  } finally {
    creating.value = false;
  }
}

async function handleRevoke(keyId: string) {
  if (!confirm(t('apiKeys.revokeConfirm'))) return;
  try {
    await revokeApiKey(keyId);
    toast.success(t('apiKeys.revoked'));
    await fetchKeys();
  } catch {
    // interceptor shows backend error
  }
}

onMounted(fetchKeys);
</script>
