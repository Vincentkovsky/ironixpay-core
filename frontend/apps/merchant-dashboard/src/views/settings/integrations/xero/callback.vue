<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Loader2, CheckCircle, XCircle } from 'lucide-vue-next'
import { xeroCallback } from '@/api/xero'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()

const status = ref<'loading' | 'success' | 'tenant-select' | 'error'>('loading')
const errorMsg = ref('')

onMounted(async () => {
    const code = route.query.code as string
    const state = route.query.state as string

    if (!code) {
        status.value = 'error'
        errorMsg.value = t('xero.callbackNoCode')
        return
    }
    if (!state) {
        status.value = 'error'
        errorMsg.value = t('xero.callbackNoState')
        return
    }

    try {
        const res = await xeroCallback(code, state)

        if (res.tenants.length > 1 && !res.connection_id) {
            status.value = 'tenant-select'
            sessionStorage.setItem('xero_pending_tenants', JSON.stringify(res.tenants))
            router.replace({
                path: '/settings/integrations/xero',
                query: { select_tenant: '1' },
            })
        } else {
            status.value = 'success'
            toast.success(t('xero.connectSuccess'))
            setTimeout(() => {
                router.replace('/settings/integrations/xero')
            }, 1500)
        }
    } catch (e: any) {
        status.value = 'error'
        const apiError = e?.response?.data?.error
        errorMsg.value = apiError?.message || apiError || t('xero.connectFailed')
    }
})
</script>

<template>
    <div class="flex min-h-[60vh] items-center justify-center">
        <div class="text-center space-y-4 animate-fade-in-up">
            <!-- Loading -->
            <template v-if="status === 'loading'">
                <Loader2 class="h-8 w-8 animate-spin text-brand mx-auto" />
                <p class="text-sm text-muted-foreground">{{ t('xero.callbackProcessing') }}</p>
            </template>

            <!-- Success -->
            <template v-if="status === 'success'">
                <CheckCircle class="h-8 w-8 text-emerald-500 mx-auto" />
                <p class="text-sm font-medium">{{ t('xero.connectSuccess') }}</p>
                <p class="text-xs text-muted-foreground">{{ t('xero.callbackRedirecting') }}</p>
            </template>

            <!-- Error -->
            <template v-if="status === 'error'">
                <XCircle class="h-8 w-8 text-destructive mx-auto" />
                <p class="text-sm font-medium">{{ t('xero.connectFailed') }}</p>
                <p class="text-xs text-muted-foreground">{{ errorMsg }}</p>
            </template>
        </div>
    </div>
</template>
