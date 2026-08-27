<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { CheckCircle, XCircle, Loader2, Unplug } from 'lucide-vue-next'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { xeroConnect, disconnectXero, type XeroConnection } from '@/api/xero'

const props = defineProps<{
    connection: XeroConnection | null
    loading: boolean
}>()

const emit = defineEmits<{
    (e: 'refresh'): void
}>()

const { t } = useI18n()
const connecting = ref(false)
const disconnecting = ref(false)

const handleConnect = async (forceReauth = false) => {
    connecting.value = true
    try {
        const res = await xeroConnect(forceReauth)
        window.location.href = res.authorize_url
    } catch {
        toast.error(t('xero.connectFailed'))
        connecting.value = false
    }
}

const handleDisconnect = async () => {
    disconnecting.value = true
    try {
        await disconnectXero()
        toast.success(t('xero.disconnectSuccess'))
        emit('refresh')
    } catch {
        toast.error(t('xero.disconnectFailed'))
    } finally {
        disconnecting.value = false
    }
}

const statusVariant = (status: string) => {
    switch (status) {
        case 'active':
            return 'border-emerald-200 bg-emerald-50 text-emerald-700'
        case 'expired':
        case 'error':
            return 'border-red-200 bg-red-50 text-red-700'
        default:
            return ''
    }
}
</script>

<template>
    <Card class="animate-fade-in-up">
        <CardHeader class="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle class="text-base font-semibold">{{ t('xero.connectionTitle') }}</CardTitle>
            <Badge
                v-if="props.connection"
                variant="outline"
                :class="statusVariant(props.connection.status)"
                class="text-[10px]"
            >
                <CheckCircle v-if="props.connection.status === 'active'" class="h-3 w-3 mr-1" />
                <XCircle v-else class="h-3 w-3 mr-1" />
                {{ t(`xero.status_${props.connection.status}`) }}
            </Badge>
        </CardHeader>
        <CardContent>
            <!-- Not connected -->
            <div v-if="!props.connection" class="space-y-3">
                <p class="text-sm text-muted-foreground">{{ t('xero.connectDesc') }}</p>
                <Button :disabled="connecting" @click="handleConnect(false)">
                    <Loader2 v-if="connecting" class="h-4 w-4 mr-2 animate-spin" />
                    {{ t('xero.connectBtn') }}
                </Button>
            </div>

            <!-- Connected -->
            <div v-else class="space-y-3">
                <div class="grid grid-cols-2 gap-3 text-sm">
                    <div>
                        <p class="text-xs text-muted-foreground">{{ t('xero.tenantName') }}</p>
                        <p class="font-medium">{{ props.connection.xero_tenant_name || '—' }}</p>
                    </div>
                    <div>
                        <p class="text-xs text-muted-foreground">{{ t('xero.connectedAt') }}</p>
                        <p class="font-medium">
                            {{ props.connection.created_at ? new Date(props.connection.created_at).toLocaleDateString() : '—' }}
                        </p>
                    </div>
                </div>

                <div class="flex items-center gap-2 pt-1">
                    <Button variant="outline" size="sm" :disabled="connecting" @click="handleConnect(true)">
                        <Loader2 v-if="connecting" class="h-3.5 w-3.5 mr-1.5 animate-spin" />
                        {{ t('xero.reconnectBtn') }}
                    </Button>
                    <Button variant="ghost" size="sm" class="text-destructive hover:text-destructive" :disabled="disconnecting" @click="handleDisconnect">
                        <Unplug class="h-3.5 w-3.5 mr-1.5" />
                        {{ t('xero.disconnectBtn') }}
                    </Button>
                </div>
            </div>
        </CardContent>
    </Card>
</template>
