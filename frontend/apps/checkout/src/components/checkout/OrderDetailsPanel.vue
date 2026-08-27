<template>
  <div class="flex-1 min-w-[320px] flex flex-col space-y-6">
    <!-- Merchant Branding Hero -->
    <div class="bg-white dark:bg-slate-800 rounded-2xl border border-gray-200 dark:border-slate-700 shadow-sm overflow-hidden">
        <!-- Gradient accent bar -->
        <div class="h-1.5 w-full bg-blue-600 shadow-[0_0_10px_rgba(37,99,235,0.3)]"></div>
        <div class="p-5 flex items-center gap-4">
            <!-- Merchant Logo -->
            <div v-if="merchantLogoUrl" class="w-14 h-14 rounded-xl bg-gray-50 dark:bg-slate-700 border border-gray-100 dark:border-slate-600 flex items-center justify-center overflow-hidden shadow-sm shrink-0">
                <img :src="merchantLogoUrl" :alt="merchantName" class="w-12 h-12 object-contain" />
            </div>
            <div v-else class="w-14 h-14 rounded-xl bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center shadow-sm shrink-0">
                <span class="text-xl font-bold text-white">{{ merchantName.charAt(0).toUpperCase() }}</span>
            </div>
            <!-- Merchant Info -->
            <div class="min-w-0 flex-1">
                <h2 class="text-lg font-bold text-gray-900 dark:text-white truncate">{{ merchantName }}</h2>
                <div class="flex items-center gap-1.5 mt-0.5">
                    <ShieldCheckIcon class="w-3.5 h-3.5 text-emerald-500 dark:text-emerald-400 shrink-0" />
                    <span class="text-xs font-medium text-emerald-600 dark:text-emerald-400">{{ t('order.verifiedMerchant') }}</span>
                </div>
            </div>
        </div>
    </div>

    <!-- Amount Card -->
    <div class="bg-white dark:bg-slate-800 rounded-2xl p-6 border border-gray-200 dark:border-slate-700 relative overflow-hidden group hover:border-gray-300 dark:hover:border-slate-600 transition-colors shadow-sm">
        <div class="flex justify-between items-start mb-1">
            <span class="text-sm font-medium text-gray-500 dark:text-gray-400">{{ t('order.totalAmount') }}</span>
             <div class="p-2 rounded-lg bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400">
                <WalletIcon class="w-5 h-5" />
             </div>
        </div>
        <div class="flex items-baseline gap-2">
             <span class="text-4xl font-bold text-gray-900 dark:text-white tracking-tight leading-none">${{ formattedAmount }}</span>
             <span class="text-xl font-medium text-gray-400 dark:text-gray-500">{{ currency }}</span>
        </div>
    </div>

    <!-- Details List -->
    <div class="space-y-1 px-1">
        <!-- Order ID -->
        <div class="flex items-center justify-between py-4 border-b border-dashed border-gray-200 dark:border-slate-700">
            <span class="text-sm font-medium text-gray-500 dark:text-gray-400">{{ t('order.orderId') }}</span>
            <span class="font-mono text-sm text-gray-700 dark:text-gray-300 tracking-wide">{{ orderId }}</span>
        </div>

        <!-- Network -->
        <div class="flex items-center justify-between py-4 border-b border-dashed border-gray-200 dark:border-slate-700">
            <span class="text-sm font-medium text-gray-500 dark:text-gray-400">{{ t('order.network') }}</span>
            <div class="px-3 py-1.5 rounded-lg bg-gray-50 dark:bg-slate-700 border border-gray-200 dark:border-slate-600 flex items-center gap-2">
                <img v-if="networkIcon" :src="networkIcon" :alt="displayNetwork" class="w-4 h-4" />
                <div v-else class="w-1.5 h-1.5 rounded-full bg-indigo-500 dark:bg-indigo-400"></div>
                <span class="text-xs font-bold text-gray-700 dark:text-gray-200 tracking-wide uppercase">{{ displayNetwork }}</span>
            </div>
        </div>
    </div>

    <!-- Security Badge -->
    <div class="mt-6 bg-blue-50 dark:bg-blue-900/20 border border-blue-100 dark:border-blue-800 rounded-xl p-4 flex gap-4">
        <div class="p-2 bg-blue-100 dark:bg-blue-900/40 rounded-full h-fit mt-1">
            <ShieldCheckIcon class="w-5 h-5 text-blue-600 dark:text-blue-400" />
        </div>
        <div>
            <h4 class="text-sm font-bold text-gray-900 dark:text-white mb-1">{{ t('order.secureCheckout') }}</h4>
            <p class="text-xs text-gray-500 dark:text-gray-400 leading-relaxed">
                {{ t('order.secureDescription', { amount: formattedAmount, currency, network: displayNetwork }) }}
            </p>
        </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { Wallet as WalletIcon, ShieldCheck as ShieldCheckIcon } from 'lucide-vue-next';
import { networkDisplayName } from '../../utils/networkUtils';
import { networkIcons } from '@ironix-pay/ui';

const { t } = useI18n();

const props = defineProps<{
    amount: number;
    currency: string;
    orderId: string;
    merchantName: string;
    merchantLogoUrl?: string | null;
    network: string;
    livemode?: boolean;
}>();

const displayNetwork = computed(() => networkDisplayName(props.network, !props.livemode));
const networkIcon = computed(() => networkIcons[props.network] || null);

const formattedAmount = computed(() => {
    const val = props.amount;
    const fixed = val.toFixed(6);
    const [int, dec = ''] = fixed.split('.');
    const trimmed = (dec || '').replace(/0+$/, '').padEnd(2, '0');
    return `${int}.${trimmed}`;
});
</script>
