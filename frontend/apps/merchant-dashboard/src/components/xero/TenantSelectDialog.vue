<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import type { XeroTenant } from '@/api/xero'

const props = defineProps<{
    open: boolean
    tenants: XeroTenant[]
    loading: boolean
}>()

const emit = defineEmits<{
    (e: 'update:open', value: boolean): void
    (e: 'select', tenantId: string): void
}>()

const { t } = useI18n()
</script>

<template>
    <Dialog :open="props.open" @update:open="emit('update:open', $event)">
        <DialogContent class="sm:max-w-md">
            <DialogHeader>
                <DialogTitle>{{ t('xero.tenantSelectTitle') }}</DialogTitle>
                <DialogDescription>{{ t('xero.tenantSelectDesc') }}</DialogDescription>
            </DialogHeader>
            <div class="space-y-2 mt-2">
                <button
                    v-for="tenant in props.tenants"
                    :key="tenant.tenant_id"
                    :disabled="props.loading"
                    class="w-full flex items-center justify-between rounded-lg border p-3 text-left hover:bg-muted/50 transition-colors disabled:opacity-50"
                    @click="emit('select', tenant.tenant_id)"
                >
                    <div>
                        <p class="text-sm font-medium">{{ tenant.tenant_name }}</p>
                        <p class="text-xs text-muted-foreground">{{ tenant.tenant_type }}</p>
                    </div>
                    <Button size="sm" variant="outline" :disabled="props.loading">
                        {{ t('xero.tenantSelect') }}
                    </Button>
                </button>
            </div>
        </DialogContent>
    </Dialog>
</template>
