<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { ArrowLeft } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import XeroConnectionCard from '@/components/xero/XeroConnectionCard.vue'
import XeroConfigForm from '@/components/xero/XeroConfigForm.vue'
import XeroSyncLogs from '@/components/xero/XeroSyncLogs.vue'
import TenantSelectDialog from '@/components/xero/TenantSelectDialog.vue'
import {
    getXeroCapability,
    getXeroConnection,
    xeroSelectTenant,
    type XeroConnection,
    type XeroTenant,
} from '@/api/xero'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const loading = ref(true)
const connection = ref<XeroConnection | null>(null)

// Tenant selection state (from callback redirect)
const showTenantDialog = ref(false)
const pendingTenants = ref<XeroTenant[]>([])
const selectingTenant = ref(false)

const isConnected = computed(() => connection.value?.status === 'active')

onMounted(async () => {
    try {
        const capability = await getXeroCapability()
        if (!capability.enabled) {
            toast.error(t('xero.notAvailable'))
            await router.replace('/settings/integrations')
            return
        }
    } catch {
        toast.error(t('xero.notAvailable'))
        await router.replace('/settings/integrations')
        return
    }

    await fetchConnection()

    // If redirected from callback with tenant selection
    if (route.query.select_tenant === '1') {
        const stored = sessionStorage.getItem('xero_pending_tenants')
        if (stored) {
            pendingTenants.value = JSON.parse(stored)
            sessionStorage.removeItem('xero_pending_tenants')
            showTenantDialog.value = true
        }
    }
})

const fetchConnection = async () => {
    loading.value = true
    try {
        const res = await getXeroConnection()
        connection.value = res
    } catch {
        connection.value = null
    } finally {
        loading.value = false
    }
}

const handleTenantSelect = async (tenantId: string) => {
    selectingTenant.value = true
    try {
        await xeroSelectTenant(tenantId)
        toast.success(t('xero.tenantSelected'))
        showTenantDialog.value = false
        await fetchConnection()
    } catch {
        toast.error(t('xero.tenantSelectFailed'))
    } finally {
        selectingTenant.value = false
    }
}
</script>

<template>
    <div class="space-y-6">
        <!-- Header -->
        <div class="animate-fade-in-up flex items-center gap-3">
            <Button variant="ghost" size="sm" as-child>
                <router-link to="/settings/integrations">
                    <ArrowLeft class="h-4 w-4" />
                </router-link>
            </Button>
            <div>
                <h1 class="text-xl font-semibold tracking-tight">{{ t('xero.title') }}</h1>
                <p class="text-sm text-muted-foreground">{{ t('xero.subtitle') }}</p>
            </div>
        </div>

        <!-- Cards -->
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
            <XeroConnectionCard
                :connection="connection"
                :loading="loading"
                @refresh="fetchConnection"
            />
            <XeroConfigForm
                :connection="connection"
                @refresh="fetchConnection"
            />
        </div>

        <!-- Sync Logs (full width) -->
        <XeroSyncLogs :connected="isConnected" />

        <!-- Tenant Select Dialog -->
        <TenantSelectDialog
            v-model:open="showTenantDialog"
            :tenants="pendingTenants"
            :loading="selectingTenant"
            @select="handleTenantSelect"
        />
    </div>
</template>
