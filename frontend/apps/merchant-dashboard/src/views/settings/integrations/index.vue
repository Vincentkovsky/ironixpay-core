<script setup lang="ts">
import { computed, ref, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { Plug, ArrowRight, Clock } from 'lucide-vue-next'
import { Card, CardContent } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { getXeroCapability, getXeroConnection, type XeroConnection } from '@/api/xero'

const { t } = useI18n()
const router = useRouter()
const loading = ref(true)
const xeroConnection = ref<XeroConnection | null>(null)
const xeroAvailable = ref(false)

onMounted(async () => {
    try {
        const capability = await getXeroCapability()
        xeroAvailable.value = capability.enabled
        if (capability.enabled) {
            const res = await getXeroConnection()
            xeroConnection.value = res
        }
    } catch {
        xeroAvailable.value = false
        xeroConnection.value = null
    } finally {
        loading.value = false
    }
})

const xeroConnected = () => xeroConnection.value?.status === 'active'

interface Integration {
    key: string
    titleKey: string
    descKey: string
    route: string
    available: boolean
    connected: () => boolean
}

const integrations = computed<Integration[]>(() => {
    const list: Integration[] = []
    if (xeroAvailable.value) {
        list.push({
            key: 'xero',
            titleKey: 'xero.title',
            descKey: 'xero.hubDesc',
            route: '/settings/integrations/xero',
            available: true,
            connected: xeroConnected,
        })
    }
    list.push({
        key: 'quickbooks',
        titleKey: 'xero.qbTitle',
        descKey: 'xero.qbDesc',
        route: '',
        available: false,
        connected: () => false,
    })
    return list
})
</script>

<template>
    <div class="space-y-6">
        <!-- Header -->
        <div class="animate-fade-in-up">
            <h1 class="text-xl font-semibold tracking-tight">{{ t('integrations.title') }}</h1>
            <p class="mt-1 text-sm text-muted-foreground">{{ t('integrations.subtitle') }}</p>
        </div>

        <!-- Integration Cards Grid -->
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            <Card
                v-for="(integration, idx) in integrations"
                :key="integration.key"
                class="animate-fade-in-up group relative"
                :class="{ 'opacity-60': !integration.available, 'cursor-pointer hover:border-brand/40 transition-colors': integration.available }"
                :style="{ animationDelay: `${(idx + 1) * 60}ms` }"
                @click="integration.available && router.push(integration.route)"
            >
                <CardContent class="p-5">
                    <div class="flex items-start justify-between">
                        <div class="flex items-center gap-3">
                            <div class="stat-icon-box h-10 w-10 flex items-center justify-center rounded-lg">
                                <Plug class="h-5 w-5 text-brand" />
                            </div>
                            <div>
                                <p class="text-sm font-semibold">{{ t(integration.titleKey) }}</p>
                                <Badge
                                    v-if="!loading && integration.connected()"
                                    variant="outline"
                                    class="mt-1 border-emerald-200 bg-emerald-50 text-emerald-700 text-[10px]"
                                >
                                    {{ t('integrations.connected') }}
                                </Badge>
                                <Badge
                                    v-else-if="!integration.available"
                                    variant="outline"
                                    class="mt-1 text-[10px]"
                                >
                                    <Clock class="h-3 w-3 mr-1" />
                                    {{ t('integrations.comingSoon') }}
                                </Badge>
                            </div>
                        </div>
                        <ArrowRight
                            v-if="integration.available"
                            class="h-4 w-4 text-muted-foreground/40 group-hover:text-brand transition-colors"
                        />
                    </div>
                    <p class="mt-3 text-xs text-muted-foreground leading-relaxed">
                        {{ t(integration.descKey) }}
                    </p>
                </CardContent>
            </Card>
        </div>
    </div>
</template>
