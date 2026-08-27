<template>
  <div v-if="show" class="absolute inset-0 z-10 bg-white dark:bg-slate-800 flex flex-col items-center justify-center p-8 text-center animate-in fade-in duration-300 rounded-3xl overflow-hidden border border-gray-200 dark:border-slate-700">

    <!-- Blue Countdown Bar -->
    <div v-if="status === 'paid' || status === 'overpaid'" class="absolute top-0 left-0 right-0 h-1.5 bg-blue-600 shadow-[0_0_15px_rgba(37,99,235,0.4)] animate-progress-strip"></div>
    <div v-else class="absolute top-0 left-0 right-0 h-1.5" :class="status === 'expired' ? 'bg-red-500 dark:bg-red-600' : 'bg-gray-200 dark:bg-slate-700'"></div>

    <!-- Icon Circle -->
    <div class="h-24 w-24 rounded-full flex items-center justify-center mb-8 shadow-lg ring-4 ring-inset"
         :class="statusColorClass">
       <component :is="iconComponent" class="w-12 h-12" />
    </div>

    <h2 class="text-3xl font-bold text-gray-900 dark:text-white mb-3 tracking-tight">{{ title }}</h2>
    <p class="text-gray-500 dark:text-gray-400 text-base max-w-[320px] leading-relaxed mb-4">{{ description }}</p>

    <!-- Transaction Hash (Only for Paid) -->
    <div v-if="(status === 'paid' || status === 'overpaid') && transactionHash"
         class="mt-6 w-full max-w-[340px] bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl p-3 flex items-center justify-between group transition-colors hover:border-gray-300 dark:hover:border-slate-600">
        <span class="text-gray-400 dark:text-gray-500 text-sm font-medium pl-1">{{ t('status.transactionHash') }}</span>
        <div class="flex items-center gap-2">
            <a :href="explorerUrl" target="_blank" rel="noopener noreferrer"
               class="font-mono text-sm text-blue-600 dark:text-blue-400 hover:text-blue-500 dark:hover:text-blue-300 transition-colors flex items-center gap-1">
                {{ truncatedHash }}
                <span class="text-xs">›</span>
            </a>
        </div>
    </div>

    <!-- Redirect Countdown -->
    <div v-if="redirectCountdown && redirectCountdown > 0"
         class="mt-6 text-sm text-gray-400 dark:text-gray-500 flex items-center gap-2 animate-pulse">
        <Loader2 class="w-4 h-4 animate-spin" />
        <span>{{ t('status.redirectingIn', { count: redirectCountdown }) }}</span>
    </div>

    <!-- Content Slot -->
    <div class="mt-8 w-full">
        <slot></slot>
    </div>

  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { Check, XCircle, Loader2, AlertTriangle } from 'lucide-vue-next';

const { t } = useI18n();

const props = defineProps<{
  status: 'paid' | 'expired' | 'detected' | 'underpaid' | 'overpaid';
  show: boolean;
  transactionHash?: string | null;
  network?: string;
  livemode?: boolean;
  redirectCountdown?: number;
}>();

const iconComponent = computed(() => {
  switch (props.status) {
    case 'paid':
    case 'overpaid': return Check;
    case 'expired': return XCircle;
    case 'detected': return Loader2; // Should spin
    case 'underpaid': return AlertTriangle;
    default: return Loader2;
  }
});

const title = computed(() => {
  switch (props.status) {
    case 'paid': return t('status.paid.title');
    case 'overpaid': return t('status.overpaid.title');
    case 'expired': return t('status.expired.title');
    case 'detected': return t('status.detected.title');
    case 'underpaid': return t('status.underpaid.title');
    default: return t('status.processing');
  }
});

const description = computed(() => {
    switch (props.status) {
    case 'paid': return t('status.paid.description');
    case 'overpaid': return t('status.overpaid.description');
    case 'expired': return t('status.expired.description');
    case 'detected': return t('status.detected.description');
    case 'underpaid': return t('status.underpaid.description');
    default: return '';
  }
});

const statusColorClass = computed(() => {
  switch (props.status) {
    case 'paid':
    case 'overpaid': return 'bg-green-50 dark:bg-green-900/20 ring-green-200 dark:ring-green-900 text-green-500 dark:text-green-400';
    case 'expired': return 'bg-red-50 dark:bg-red-900/20 ring-red-200 dark:ring-red-900 text-red-500 dark:text-red-400';
    case 'detected': return 'bg-blue-50 dark:bg-blue-900/20 ring-blue-200 dark:ring-blue-900 text-blue-500 dark:text-blue-400 animate-pulse';
    case 'underpaid': return 'bg-amber-50 dark:bg-amber-900/20 ring-amber-200 dark:ring-amber-900 text-amber-500 dark:text-amber-400';
    default: return 'bg-gray-100 dark:bg-slate-700';
  }
});

const truncatedHash = computed(() => {
    if (!props.transactionHash) return '';
    if (props.transactionHash.length <= 15) return props.transactionHash;
    return `${props.transactionHash.slice(0, 6)}...${props.transactionHash.slice(-4)}`;
});

const EXPLORERS: Record<string, { mainnet: string; testnet: string }> = {
    TRON: {
        mainnet: 'https://tronscan.org/#/transaction/',
        testnet: 'https://nile.tronscan.org/#/transaction/',
    },
    BSC: {
        mainnet: 'https://bscscan.com/tx/',
        testnet: 'https://testnet.bscscan.com/tx/',
    },
    ETHEREUM: {
        mainnet: 'https://etherscan.io/tx/',
        testnet: 'https://sepolia.etherscan.io/tx/',
    },
    POLYGON: {
        mainnet: 'https://polygonscan.com/tx/',
        testnet: 'https://amoy.polygonscan.com/tx/',
    },
    ARBITRUM: {
        mainnet: 'https://arbiscan.io/tx/',
        testnet: 'https://sepolia.arbiscan.io/tx/',
    },
    BASE: {
        mainnet: 'https://basescan.org/tx/',
        testnet: 'https://sepolia.basescan.org/tx/',
    },
    OPTIMISM: {
        mainnet: 'https://optimistic.etherscan.io/tx/',
        testnet: 'https://sepolia-optimism.etherscan.io/tx/',
    },
};

const explorerUrl = computed(() => {
    if (!props.transactionHash) return '#';
    const network = props.network || 'TRON';
    const isTestnet = props.livemode === false;
    const explorer = EXPLORERS[network] || EXPLORERS.TRON;
    return (isTestnet ? explorer.testnet : explorer.mainnet) + props.transactionHash;
});
</script>
