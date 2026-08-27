<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { RefreshCw, RotateCcw } from 'lucide-vue-next'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Table, TableHeader, TableRow, TableHead, TableBody, TableCell } from '@/components/ui/table'
import { getXeroSyncLogs, retryXeroSync, type XeroSyncLog } from '@/api/xero'

const props = defineProps<{
    connected: boolean
}>()

const { t } = useI18n()
const logs = ref<XeroSyncLog[]>([])
const loading = ref(false)
const page = ref(1)
const totalPages = ref(1)
const retryingId = ref<string | null>(null)
const perPage = 10

onMounted(() => {
    if (props.connected) fetchLogs()
})

watch(
    () => props.connected,
    (val) => {
        if (val) fetchLogs()
    },
)

const fetchLogs = async () => {
    loading.value = true
    try {
        const res = await getXeroSyncLogs(page.value, perPage)
        logs.value = res.data
        totalPages.value = res.meta.total_pages
    } catch {
        // ignore
    } finally {
        loading.value = false
    }
}

const handleRetry = async (logId: string) => {
    retryingId.value = logId
    try {
        await retryXeroSync(logId)
        toast.success(t('xero.retryQueued'))
        await fetchLogs()
    } catch {
        toast.error(t('xero.retryFailed'))
    } finally {
        retryingId.value = null
    }
}

const prevPage = () => {
    if (page.value > 1) {
        page.value--
        fetchLogs()
    }
}

const nextPage = () => {
    if (page.value < totalPages.value) {
        page.value++
        fetchLogs()
    }
}

const statusClass = (status: string) => {
    switch (status) {
        case 'synced':
            return 'border-emerald-200 bg-emerald-50 text-emerald-700'
        case 'failed':
            return 'border-red-200 bg-red-50 text-red-700'
        case 'pending':
            return 'border-amber-200 bg-amber-50 text-amber-700'
        default:
            return ''
    }
}

const formatDate = (d: string) => {
    return new Date(d).toLocaleString(undefined, {
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
    })
}
</script>

<template>
    <Card class="animate-fade-in-up" style="animation-delay: 120ms">
        <CardHeader class="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle class="text-base font-semibold">{{ t('xero.syncLogsTitle') }}</CardTitle>
            <Button
                v-if="props.connected"
                variant="ghost"
                size="sm"
                :disabled="loading"
                @click="fetchLogs"
            >
                <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': loading }" />
            </Button>
        </CardHeader>
        <CardContent>
            <div v-if="!props.connected" class="text-sm text-muted-foreground">
                {{ t('xero.configConnectFirst') }}
            </div>

            <div v-else-if="logs.length === 0 && !loading" class="py-6 text-center text-sm text-muted-foreground">
                {{ t('xero.noSyncLogs') }}
            </div>

            <div v-else class="space-y-3">
                <Table>
                    <TableHeader>
                        <TableRow>
                            <TableHead class="text-xs">{{ t('xero.session') }}</TableHead>
                            <TableHead class="text-xs">{{ t('xero.invoice') }}</TableHead>
                            <TableHead class="text-xs">{{ t('xero.logStatus') }}</TableHead>
                            <TableHead class="text-xs">{{ t('xero.attempts') }}</TableHead>
                            <TableHead class="text-xs">{{ t('xero.logDate') }}</TableHead>
                            <TableHead class="text-xs w-[60px]"></TableHead>
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        <TableRow v-for="log in logs" :key="log.id">
                            <TableCell class="text-xs text-muted-foreground font-mono">
                                {{ log.session_id.slice(0, 12) }}…
                            </TableCell>
                            <TableCell class="text-xs text-muted-foreground font-mono">
                                {{ log.xero_invoice_id || '—' }}
                            </TableCell>
                            <TableCell>
                                <Badge variant="outline" :class="statusClass(log.status)" class="text-[10px]">
                                    {{ t(`xero.syncStatus_${log.status}`) }}
                                </Badge>
                            </TableCell>
                            <TableCell class="text-xs text-muted-foreground">
                                {{ log.attempt_count }}
                            </TableCell>
                            <TableCell class="text-xs text-muted-foreground">
                                {{ formatDate(log.created_at) }}
                            </TableCell>
                            <TableCell>
                                <Button
                                    v-if="log.status === 'failed'"
                                    variant="ghost"
                                    size="sm"
                                    :disabled="retryingId === log.id"
                                    @click="handleRetry(log.id)"
                                >
                                    <RotateCcw class="h-3 w-3" :class="{ 'animate-spin': retryingId === log.id }" />
                                </Button>
                            </TableCell>
                        </TableRow>
                    </TableBody>
                </Table>

                <!-- Pagination -->
                <div v-if="totalPages > 1" class="flex items-center justify-between pt-1">
                    <p class="text-xs text-muted-foreground">
                        {{ t('xero.logPage', { current: page, total: totalPages }) }}
                    </p>
                    <div class="flex gap-1">
                        <Button variant="outline" size="sm" :disabled="page <= 1" @click="prevPage">
                            {{ t('xero.logPrev') }}
                        </Button>
                        <Button variant="outline" size="sm" :disabled="page >= totalPages" @click="nextPage">
                            {{ t('xero.logNext') }}
                        </Button>
                    </div>
                </div>
            </div>
        </CardContent>
    </Card>
</template>
