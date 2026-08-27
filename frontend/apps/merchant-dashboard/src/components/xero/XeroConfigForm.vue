<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Loader2 } from 'lucide-vue-next'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Button } from '@/components/ui/button'
import { Select, SelectTrigger, SelectValue, SelectContent, SelectItem } from '@/components/ui/select'
import {
    getXeroAccounts,
    getXeroTaxRates,
    updateXeroConnection,
    type XeroAccount,
    type XeroConnection,
    type XeroTaxRate,
} from '@/api/xero'

const props = defineProps<{
    connection: XeroConnection | null
}>()

const emit = defineEmits<{
    (e: 'refresh'): void
}>()

const { t } = useI18n()
const accounts = ref<XeroAccount[]>([])
const taxRates = ref<XeroTaxRate[]>([])
const loadingAccounts = ref(false)
const loadingTaxRates = ref(false)
const saving = ref(false)

const revenueAccountCode = ref('')
const feeAccountCode = ref('')
const payoutAccountCode = ref('')
const taxType = ref('NONE')
const autoSync = ref(false)

watch(
    () => props.connection,
    (conn) => {
        if (conn) {
            revenueAccountCode.value = conn.xero_account_code || ''
            feeAccountCode.value = conn.xero_fee_account_code || ''
            payoutAccountCode.value = conn.xero_payment_account_code || ''
            taxType.value = conn.xero_tax_type || 'NONE'
            autoSync.value = conn.auto_sync_enabled
        }
    },
    { immediate: true },
)

onMounted(async () => {
    if (props.connection?.status === 'active') {
        await fetchConfigOptions()
    }
})

watch(
    () => props.connection?.status,
    (status) => {
        if (status === 'active') fetchConfigOptions()
    },
)

const fetchConfigOptions = async () => {
    await Promise.all([fetchAccounts(), fetchTaxRates()])
}

const fetchAccounts = async () => {
    loadingAccounts.value = true
    try {
        const res = await getXeroAccounts()
        accounts.value = res
    } catch {
        // Silently fail — user can still type values
    } finally {
        loadingAccounts.value = false
    }
}

const fetchTaxRates = async () => {
    loadingTaxRates.value = true
    try {
        const res = await getXeroTaxRates()
        taxRates.value = res
    } catch {
        taxRates.value = []
    } finally {
        loadingTaxRates.value = false
    }
}

const handleSave = async () => {
    const normalizedPayoutAccount =
        payoutAccountCode.value && !payoutAccountCode.value.startsWith('__bank_no_code__')
            ? payoutAccountCode.value
            : ''
    if (autoSync.value && !normalizedPayoutAccount) {
        toast.error(t('xero.payoutAccountRequiredForAutoSync'))
        return
    }

    saving.value = true
    try {
        await updateXeroConnection({
            xero_account_code: revenueAccountCode.value || null,
            xero_fee_account_code: feeAccountCode.value || null,
            xero_payment_account_code: normalizedPayoutAccount || null,
            xero_tax_type: taxType.value || 'NONE',
            auto_sync_enabled: autoSync.value,
        })
        toast.success(t('xero.configSaved'))
        emit('refresh')
    } catch {
        toast.error(t('xero.configSaveFailed'))
    } finally {
        saving.value = false
    }
}

const revenueAccounts = () => accounts.value.filter((a) => a.type === 'REVENUE')
const expenseAccounts = () => accounts.value.filter((a) => a.type === 'EXPENSE' || a.type === 'OVERHEADS')
const taxRateOptions = () => {
    const byTaxType = new Map<string, XeroTaxRate>()
    for (const rate of taxRates.value) {
        if (!byTaxType.has(rate.tax_type)) {
            byTaxType.set(rate.tax_type, rate)
        }
    }
    if (!byTaxType.has('NONE')) {
        byTaxType.set('NONE', {
            tax_type: 'NONE',
            name: 'No Tax',
            display_tax_rate: 0,
            can_apply_to_revenue: true,
            status: 'ACTIVE',
        })
    }

    return Array.from(byTaxType.values()).sort((a, b) => {
        if (a.tax_type === 'NONE') return -1
        if (b.tax_type === 'NONE') return 1
        return a.name.localeCompare(b.name)
    })
}
const formatTaxRate = (rate: number) =>
    Number.isInteger(rate) ? `${rate}` : `${rate.toFixed(2)}`
const bankAccounts = () =>
    accounts.value
        .filter((a) => a.type === 'BANK')
        .map((a) => ({
            ...a,
            selectValue: a.code || `__bank_no_code__${a.account_id}`,
            displayCode: a.code || '—',
            // Some Xero orgs don't expose an editable "Enable payments to this account" toggle
            // for BANK accounts. We only hard-block accounts without code.
            disabled: !a.code,
            disabledReason: [
                !a.code ? t('xero.bankAccountUnavailableNoCode') : '',
            ]
                .filter(Boolean)
                .join('; '),
        }))
</script>

<template>
    <Card class="animate-fade-in-up" style="animation-delay: 60ms">
        <CardHeader class="pb-2">
            <CardTitle class="text-base font-semibold">{{ t('xero.configTitle') }}</CardTitle>
        </CardHeader>
        <CardContent>
            <div v-if="!props.connection" class="text-sm text-muted-foreground">
                {{ t('xero.configConnectFirst') }}
            </div>
            <div v-else class="space-y-4">
                <!-- Revenue Account -->
                <div class="space-y-1.5">
                    <Label class="text-xs">{{ t('xero.revenueAccount') }}</Label>
                    <Select v-model="revenueAccountCode">
                        <SelectTrigger>
                            <SelectValue :placeholder="loadingAccounts ? t('xero.loadingAccounts') : t('xero.selectAccount')" />
                        </SelectTrigger>
                        <SelectContent>
                            <SelectItem v-for="acc in revenueAccounts()" :key="acc.code" :value="acc.code">
                                {{ acc.code }} — {{ acc.name }}
                            </SelectItem>
                        </SelectContent>
                    </Select>
                    <p class="text-[11px] text-muted-foreground">{{ t('xero.revenueAccountHint') }}</p>
                </div>

                <!-- Fee Account -->
                <div class="space-y-1.5">
                    <Label class="text-xs">{{ t('xero.feeAccount') }}</Label>
                    <Select v-model="feeAccountCode">
                        <SelectTrigger>
                            <SelectValue :placeholder="loadingAccounts ? t('xero.loadingAccounts') : t('xero.selectAccount')" />
                        </SelectTrigger>
                        <SelectContent>
                            <SelectItem v-for="acc in expenseAccounts()" :key="acc.code" :value="acc.code">
                                {{ acc.code }} — {{ acc.name }}
                            </SelectItem>
                        </SelectContent>
                    </Select>
                    <p class="text-[11px] text-muted-foreground">{{ t('xero.feeAccountHint') }}</p>
                </div>

                <!-- Tax Code -->
                <div class="space-y-1.5">
                    <Label class="text-xs">{{ t('xero.taxCode') }}</Label>
                    <Select v-model="taxType">
                        <SelectTrigger>
                            <SelectValue :placeholder="loadingTaxRates ? t('xero.loadingTaxRates') : t('xero.selectTaxCode')" />
                        </SelectTrigger>
                        <SelectContent>
                            <SelectItem v-for="tax in taxRateOptions()" :key="tax.tax_type" :value="tax.tax_type">
                                {{ tax.tax_type }} — {{ tax.name }} ({{ formatTaxRate(tax.display_tax_rate) }}%)
                            </SelectItem>
                        </SelectContent>
                    </Select>
                    <p class="text-[11px] text-muted-foreground">{{ t('xero.taxCodeHint') }}</p>
                </div>

                <!-- Payment Account -->
                <div class="space-y-1.5">
                    <Label class="text-xs">{{ t('xero.payoutAccount') }}</Label>
                    <Select v-model="payoutAccountCode">
                        <SelectTrigger>
                            <SelectValue :placeholder="loadingAccounts ? t('xero.loadingAccounts') : t('xero.selectAccount')" />
                        </SelectTrigger>
                        <SelectContent>
                            <SelectItem
                                v-for="acc in bankAccounts()"
                                :key="acc.account_id"
                                :value="acc.selectValue"
                                :disabled="acc.disabled"
                            >
                                <span>
                                    {{ acc.displayCode }} — {{ acc.name }}
                                </span>
                                <span
                                    v-if="acc.disabledReason"
                                    class="text-[11px] text-muted-foreground"
                                >
                                    ({{ acc.disabledReason }})
                                </span>
                            </SelectItem>
                        </SelectContent>
                    </Select>
                    <p class="text-[11px] text-muted-foreground">{{ t('xero.payoutAccountHint') }}</p>
                    <div class="space-y-1 rounded-md border border-dashed bg-muted/30 p-2">
                        <p class="text-[11px] font-medium">{{ t('xero.payoutAccountGuideTitle') }}</p>
                        <p class="text-[11px] text-muted-foreground">{{ t('xero.payoutAccountGuideStep1') }}</p>
                        <p class="text-[11px] text-muted-foreground">{{ t('xero.payoutAccountGuideStep2') }}</p>
                        <p class="text-[11px] text-muted-foreground">{{ t('xero.payoutAccountGuideStep3') }}</p>
                        <p class="text-[11px] text-muted-foreground">{{ t('xero.payoutAccountGuideStep4') }}</p>
                    </div>
                </div>

                <!-- Auto Sync Toggle -->
                <div class="flex items-center justify-between rounded-lg border p-3">
                    <div>
                        <p class="text-sm font-medium">{{ t('xero.autoSync') }}</p>
                        <p class="text-[11px] text-muted-foreground">{{ t('xero.autoSyncHint') }}</p>
                    </div>
                    <Switch v-model="autoSync" />
                </div>

                <!-- Save -->
                <Button :disabled="saving" class="w-full" @click="handleSave">
                    <Loader2 v-if="saving" class="h-4 w-4 mr-2 animate-spin" />
                    {{ t('xero.saveConfig') }}
                </Button>
            </div>
        </CardContent>
    </Card>
</template>
