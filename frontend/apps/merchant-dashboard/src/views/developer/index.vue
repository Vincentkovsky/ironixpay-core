<template>
  <div class="space-y-6">
    <!-- Webhook Config -->
    <Card class="animate-fade-in-up">
      <CardHeader>
        <div class="flex items-center gap-2">
          <div class="stat-icon-box h-8 w-8 flex items-center justify-center rounded-md">
            <Webhook class="h-4 w-4 text-brand" />
          </div>
          <CardTitle>{{ t('developer.webhookConfig') }}</CardTitle>
        </div>
      </CardHeader>

      <!-- Developer Resources Banner -->
      <div class="mx-6 mb-4 rounded-lg bg-blue-50 dark:bg-blue-950/30 border border-blue-100 dark:border-blue-900 px-4 py-3 flex items-start gap-3">
        <BookOpen class="h-4 w-4 text-blue-600 dark:text-blue-400 mt-0.5 shrink-0" />
        <div class="flex-1 text-sm">
          <span class="text-blue-800 dark:text-blue-300 font-medium">{{ t('developer.docsHint') }}</span>
          <div class="flex items-center gap-4 mt-1.5">
            <a :href="webhookGuideUrl" target="_blank" rel="noopener"
               class="inline-flex items-center gap-1 text-xs font-medium text-blue-600 dark:text-blue-400 hover:underline">
              <ExternalLink class="h-3 w-3" />
              {{ t('developer.webhookGuide') }}
            </a>
            <a href="https://api.ironixpay.com/docs" target="_blank" rel="noopener"
               class="inline-flex items-center gap-1 text-xs font-medium text-blue-600 dark:text-blue-400 hover:underline">
              <ExternalLink class="h-3 w-3" />
              {{ t('developer.apiReference') }}
            </a>
          </div>
        </div>
      </div>

      <CardContent>
        <!-- State 1: Empty State -->
        <div v-if="!configLoaded" class="py-8 text-center text-muted-foreground">
          <Loader2 class="h-6 w-6 mx-auto animate-spin" />
        </div>

        <div v-else-if="!hasConfig && !showSetupForm" class="py-12 text-center space-y-4">
          <div class="flex flex-col items-center gap-3">
            <div class="h-12 w-12 rounded-full bg-muted flex items-center justify-center">
              <Webhook class="h-6 w-6 text-muted-foreground/50" />
            </div>
            <div>
              <p class="text-sm font-medium">{{ t('developer.emptyTitle') }}</p>
              <p class="text-xs text-muted-foreground mt-1">{{ t('developer.emptyDescription') }}</p>
            </div>
            <Button id="btn-configure-webhook" size="sm" @click="showSetupForm = true">
              {{ t('developer.configureWebhook') }}
            </Button>
          </div>
        </div>

        <!-- Setup Form (new webhook) -->
        <form v-else-if="!hasConfig && showSetupForm" class="space-y-4" @submit.prevent="createConfig">
          <div class="space-y-2">
            <Label for="wh-setup-url">{{ t('developer.endpointUrl') }}</Label>
            <Input id="wh-setup-url" v-model="setupUrl" :placeholder="t('developer.urlPlaceholder')" />
          </div>
          <div class="flex gap-2">
            <Button id="btn-create-webhook" type="submit" :disabled="saving || !setupUrl">
              <Loader2 v-if="saving" class="h-4 w-4 mr-1.5 animate-spin" />
              {{ t('developer.save') }}
            </Button>
            <Button id="btn-cancel-setup" variant="outline" type="button" @click="showSetupForm = false">
              {{ t('developer.cancel') }}
            </Button>
          </div>
        </form>

        <!-- State 2: Configured -->
        <div v-else class="space-y-5">
          <!-- Endpoint URL -->
          <div class="space-y-2">
            <Label>{{ t('developer.endpointUrl') }}</Label>
            <div v-if="!editingUrl" class="flex items-center gap-2">
              <code class="flex-1 text-sm font-mono bg-muted px-3 py-2 rounded-md truncate">{{ config.url }}</code>
              <Button id="btn-edit-url" variant="outline" size="sm" @click="startEditUrl">
                <Pencil class="h-3.5 w-3.5" />
              </Button>
            </div>
            <form v-else class="flex gap-2" @submit.prevent="saveUrl">
              <Input id="wh-edit-url" v-model="editUrlValue" class="flex-1" />
              <Button id="btn-save-url" size="sm" type="submit" :disabled="saving || !editUrlValue">
                <Loader2 v-if="saving" class="h-4 w-4 mr-1 animate-spin" />
                {{ t('developer.save') }}
              </Button>
              <Button id="btn-cancel-url" variant="outline" size="sm" type="button" @click="editingUrl = false">
                {{ t('developer.cancel') }}
              </Button>
            </form>
          </div>

          <!-- Status -->
          <div class="flex items-center justify-between">
            <div class="space-y-0.5">
              <Label>{{ t('developer.webhookStatus') }}</Label>
              <p class="text-xs text-muted-foreground">{{ t('developer.statusDescription') }}</p>
            </div>
            <Switch
              id="switch-webhook-status"
              :model-value="config.status === 'enabled'"
              @update:model-value="toggleStatus"
            />
          </div>

          <!-- Signing Secret -->
          <div class="space-y-2">
            <Label>{{ t('developer.signingSecret') }}</Label>
            <div class="flex items-center gap-2">
              <code class="flex-1 text-xs font-mono bg-muted px-3 py-2 rounded-md text-muted-foreground select-none">
                {{ config.secret }}
              </code>
              <Button
                id="btn-rotate-secret"
                variant="outline"
                size="sm"
                class="text-destructive hover:text-destructive hover:bg-destructive/10"
                @click="showRotateDialog = true"
              >
                <RefreshCw class="h-3.5 w-3.5 mr-1" />
                {{ t('developer.rotate') }}
              </Button>
            </div>
          </div>

          <!-- Danger Zone -->
          <div class="border-t border-destructive/20 pt-4 mt-6">
            <div class="flex items-center justify-between">
              <div>
                <p class="text-sm font-medium text-destructive">{{ t('developer.dangerZone') }}</p>
                <p class="text-xs text-muted-foreground mt-0.5">{{ t('developer.deleteDescription') }}</p>
              </div>
              <Button
                id="btn-delete-webhook"
                variant="outline"
                size="sm"
                class="text-destructive hover:text-destructive hover:bg-destructive/10 border-destructive/30"
                @click="showDeleteDialog = true"
              >
                <Trash2 class="h-3.5 w-3.5 mr-1" />
                {{ t('developer.deleteWebhook') }}
              </Button>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>

    <!-- Recent Logs -->
    <Card class="animate-fade-in-up delay-1">
      <CardHeader>
        <div class="flex items-center justify-between">
          <CardTitle>{{ t('developer.recentLogs') }}</CardTitle>
          <Button variant="outline" size="sm" @click="fetchLogs">
            <RefreshCw class="h-3.5 w-3.5" />
          </Button>
        </div>
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
            <TableRow v-for="log in logs" :key="log.id">
              <TableCell>
                <div class="flex items-center gap-2">
                  <span :class="['status-dot', log.status === 'Success' ? 'status-dot--success' : 'status-dot--failed']" />
                  <Badge :variant="log.status === 'Success' ? 'default' : 'destructive'">{{ log.status }}</Badge>
                </div>
              </TableCell>
              <TableCell class="hidden md:table-cell text-sm">{{ log.eventType }}</TableCell>
              <TableCell class="text-sm text-muted-foreground">{{ formatDateTime(log.createdAt) }}</TableCell>
              <TableCell class="hidden sm:table-cell"><Badge variant="outline">{{ log.httpStatus || 'N/A' }}</Badge></TableCell>
              <TableCell class="text-right space-x-1">
                <Button variant="ghost" size="sm" @click="selectedLog = log; logDrawerOpen = true">{{ t('sessionDetail.view') }}</Button>
                <Button variant="ghost" size="sm" @click="retryLog(log.id)">{{ t('developer.resend') }}</Button>
              </TableCell>
            </TableRow>
            <TableRow v-if="logsLoaded && logs.length === 0">
              <TableCell colspan="5" class="text-center text-muted-foreground py-12">
                <div class="flex flex-col items-center gap-2">
                  <Terminal class="h-8 w-8 text-muted-foreground/30" />
                  <span>{{ t('developer.noLogs') }}</span>
                </div>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
        </div>
      </CardContent>
    </Card>

    <!-- Rotate Secret Confirmation Dialog -->
    <Dialog v-model:open="showRotateDialog">
      <DialogContent :showCloseButton="false">
        <DialogHeader>
          <DialogTitle class="flex items-center gap-2 text-destructive">
            <ShieldAlert class="h-5 w-5" />
            {{ t('developer.rotateTitle') }}
          </DialogTitle>
          <DialogDescription class="space-y-2 pt-2">
            <p>{{ t('developer.rotateWarning') }}</p>
            <p class="text-xs text-muted-foreground/80">{{ t('developer.rotateReplayHint') }}</p>
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2 sm:gap-0">
          <Button id="btn-cancel-rotate" variant="outline" @click="showRotateDialog = false">
            {{ t('developer.cancel') }}
          </Button>
          <Button
            id="btn-confirm-rotate"
            variant="destructive"
            :disabled="rotating"
            @click="confirmRotate"
          >
            <Loader2 v-if="rotating" class="h-4 w-4 mr-1 animate-spin" />
            {{ t('developer.rotate') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- Secret Reveal Dialog -->
    <Dialog v-model:open="showRevealDialog">
      <DialogContent :showCloseButton="false">
        <DialogHeader>
          <DialogTitle class="flex items-center gap-2">
            <Key class="h-5 w-5 text-brand" />
            {{ t('developer.newSecretTitle') }}
          </DialogTitle>
          <DialogDescription>
            {{ t('developer.newSecretWarning') }}
          </DialogDescription>
        </DialogHeader>
        <div class="space-y-3">
          <div class="relative">
            <code class="block w-full text-xs font-mono bg-muted p-3 rounded-md break-all select-all border">
              {{ revealedSecret }}
            </code>
          </div>
          <Button id="btn-copy-secret" variant="outline" class="w-full" @click="copySecret">
            <Copy class="h-4 w-4 mr-1.5" />
            {{ copied ? t('developer.copied') : t('developer.copySecret') }}
          </Button>
        </div>
        <DialogFooter>
          <Button id="btn-close-reveal" @click="closeRevealDialog">
            {{ t('developer.done') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- Delete Confirmation Dialog -->
    <Dialog v-model:open="showDeleteDialog">
      <DialogContent :showCloseButton="false">
        <DialogHeader>
          <DialogTitle class="flex items-center gap-2 text-destructive">
            <Trash2 class="h-5 w-5" />
            {{ t('developer.deleteTitle') }}
          </DialogTitle>
          <DialogDescription>
            {{ t('developer.deleteWarning') }}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2 sm:gap-0">
          <Button id="btn-cancel-delete" variant="outline" @click="showDeleteDialog = false">
            {{ t('developer.cancel') }}
          </Button>
          <Button
            id="btn-confirm-delete"
            variant="destructive"
            :disabled="deleting"
            @click="confirmDelete"
          >
            <Loader2 v-if="deleting" class="h-4 w-4 mr-1 animate-spin" />
            {{ t('developer.deleteWebhook') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- Webhook Log Detail Drawer -->
    <WebhookLogDrawer v-model:open="logDrawerOpen" :log="selectedLog" @resent="fetchLogs" />
  </div>
</template>

<script lang="ts" setup>
import { ref, reactive, computed, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import {
  BookOpen, RefreshCw, Loader2, Webhook, Terminal, Pencil,
  Trash2, ShieldAlert, Key, Copy, ExternalLink,
} from 'lucide-vue-next';
import { toast } from 'vue-sonner';
import { formatDateTime } from '@/utils/date';
import {
  queryWebhookConfig, updateWebhookConfig, rotateWebhookSecret,
  deleteWebhookConfig, queryWebhookLogs, resendWebhook,
  type WebhookConfig, type WebhookLog,
} from '@/api/developer';
import WebhookLogDrawer from '@/components/WebhookLogDrawer.vue';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';

const { t, locale } = useI18n();

const webhookGuideUrl = computed(() =>
  locale.value === 'zh-CN'
    ? 'https://ironixpay.com/guide/webhooks'
    : 'https://ironixpay.com/en/guide/webhooks'
);

// --- Webhook Config State ---
const config = reactive<Partial<WebhookConfig>>({});
const configLoaded = ref(false);
const hasConfig = computed(() => !!config.url);
const showSetupForm = ref(false);
const setupUrl = ref('');
const saving = ref(false);

// Inline URL editing
const editingUrl = ref(false);
const editUrlValue = ref('');

// Rotate
const showRotateDialog = ref(false);
const rotating = ref(false);

// Secret reveal (shared between create + rotate)
const showRevealDialog = ref(false);
const revealedSecret = ref('');
const copied = ref(false);

// Delete
const showDeleteDialog = ref(false);
const deleting = ref(false);

// --- Logs ---
const logs = ref<WebhookLog[]>([]);
const logsLoaded = ref(false);
const logDrawerOpen = ref(false);
const selectedLog = ref<WebhookLog | null>(null);

// --- Actions ---
const fetchConfig = async () => {
  try {
    const res = await queryWebhookConfig();
    if (res) {
      Object.assign(config, res);
    } else {
      // Clear config
      Object.keys(config).forEach(k => delete (config as any)[k]);
    }
  } catch { /* interceptor */ }
  finally { configLoaded.value = true; }
};

const createConfig = async () => {
  saving.value = true;
  try {
    const res: any = await updateWebhookConfig({ url: setupUrl.value });
    Object.assign(config, res);
    // If secret is not masked, show reveal dialog
    if (res?.secret && !res.secret.includes('***')) {
      revealedSecret.value = res.secret;
      showRevealDialog.value = true;
    }
    showSetupForm.value = false;
    setupUrl.value = '';
    toast.success(t('developer.configSaved'));
  } catch { /* interceptor */ }
  finally { saving.value = false; }
};

const startEditUrl = () => {
  editUrlValue.value = config.url || '';
  editingUrl.value = true;
};

const saveUrl = async () => {
  saving.value = true;
  try {
    const res: any = await updateWebhookConfig({ url: editUrlValue.value });
    Object.assign(config, res);
    editingUrl.value = false;
    toast.success(t('developer.configSaved'));
  } catch { /* interceptor */ }
  finally { saving.value = false; }
};

const toggleStatus = async (checked: boolean) => {
  const newStatus = checked ? 'enabled' : 'disabled';
  try {
    const res: any = await updateWebhookConfig({ status: newStatus });
    Object.assign(config, res);
    toast.success(t('developer.configSaved'));
  } catch { /* interceptor */ }
};

const confirmRotate = async () => {
  rotating.value = true;
  try {
    const res: any = await rotateWebhookSecret();
    showRotateDialog.value = false;
    if (res?.secret) {
      revealedSecret.value = res.secret;
      showRevealDialog.value = true;
    }
    toast.success(t('developer.secretRotated'));
    // Refresh config (will show masked secret)
    await fetchConfig();
  } catch { /* interceptor */ }
  finally { rotating.value = false; }
};

const copySecret = async () => {
  try {
    await navigator.clipboard.writeText(revealedSecret.value);
    copied.value = true;
    toast.success(t('developer.copied'));
    setTimeout(() => { copied.value = false; }, 2000);
  } catch {
    toast.error('Failed to copy');
  }
};

const closeRevealDialog = () => {
  showRevealDialog.value = false;
  revealedSecret.value = '';
  copied.value = false;
};

const confirmDelete = async () => {
  deleting.value = true;
  try {
    await deleteWebhookConfig();
    showDeleteDialog.value = false;
    Object.keys(config).forEach(k => delete (config as any)[k]);
    toast.success(t('developer.webhookDeleted'));
  } catch { /* interceptor */ }
  finally { deleting.value = false; }
};

const fetchLogs = async () => {
  try {
    const res = await queryWebhookLogs({ pageSize: 20 });
    logs.value = res.list;
  } catch { /* */ }
  finally { logsLoaded.value = true; }
};

const retryLog = async (id: string) => {
  try { await resendWebhook(id); toast.success(t('developer.resendTriggered')); fetchLogs(); }
  catch { /* interceptor */ }
};

import { useSmartPolling } from '@/composables/useSmartPolling';

useSmartPolling(fetchLogs);

onMounted(() => { fetchConfig(); });
</script>
