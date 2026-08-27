<template>
  <div :class="[
    'flex-1 bg-white dark:bg-slate-800 relative overflow-hidden flex flex-col',
    embedMode ? 'rounded-none border-0 shadow-none max-w-none' : 'max-w-[480px] rounded-3xl border border-gray-200 dark:border-slate-700 shadow-sm'
  ]">
       <!-- Blue Top Border (hidden in embed) -->
       <div v-if="!embedMode" class="h-1.5 w-full bg-blue-600 shadow-[0_0_10px_rgba(37,99,235,0.3)]"></div>

        <div :class="['flex-1 flex flex-col items-center', embedMode ? 'p-4 pb-2' : 'p-8']">

             <!-- Compact Embed Header -->
             <div v-if="embedMode" class="w-full mb-4 pb-4 border-b border-gray-100 dark:border-slate-700">
                <!-- Row 1: Amount + Timer -->
                <div class="flex items-center justify-between mb-3">
                    <div class="flex items-baseline gap-1.5">
                        <span class="text-3xl font-bold text-gray-900 dark:text-white tracking-tight">{{ formattedExpected }}</span>
                        <span class="text-base font-medium text-gray-400 dark:text-gray-500">{{ currency }}</span>
                    </div>
                    <div class="px-3 py-1.5 rounded-full bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 flex items-center gap-1.5">
                        <ClockIcon class="w-3 h-3 text-gray-400 dark:text-gray-500" />
                        <span class="font-mono text-xs font-medium" :class="timerColorClass">{{ formattedTime }}</span>
                    </div>
                </div>
                <!-- Row 2: Merchant + Network -->
                <div class="flex items-center justify-between">
                    <div class="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400">
                        <div v-if="merchantLogoUrl" class="w-6 h-6 rounded-md bg-gray-50 dark:bg-slate-700 border border-gray-100 dark:border-slate-600 flex items-center justify-center overflow-hidden shrink-0">
                            <img :src="merchantLogoUrl" :alt="merchantName" class="w-5 h-5 object-contain" />
                        </div>
                        <div v-else class="w-6 h-6 rounded-md bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center shrink-0">
                            <span class="text-[10px] font-bold text-white">{{ merchantName.charAt(0).toUpperCase() }}</span>
                        </div>
                        <span class="font-semibold text-gray-700 dark:text-gray-300">{{ merchantName }}</span>
                    </div>
                    <div class="px-2.5 py-1 rounded-md bg-indigo-50 dark:bg-indigo-900/30 border border-indigo-100 dark:border-indigo-800 flex items-center gap-1.5">
                        <img v-if="networkIcon" :src="networkIcon" :alt="displayNetwork" class="w-3.5 h-3.5" />
                        <div v-else class="w-1.5 h-1.5 rounded-full bg-indigo-500 dark:bg-indigo-400"></div>
                        <span class="text-[10px] font-bold text-indigo-600 dark:text-indigo-400 tracking-wide uppercase">{{ displayNetwork }}</span>
                    </div>
                </div>
             </div>

             <!-- Timer Pill (normal mode only) -->
            <div v-if="!embedMode" class="mb-8 px-4 py-1.5 rounded-full bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 flex items-center gap-2">
                <ClockIcon class="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
                <span class="font-mono text-xs text-gray-600 dark:text-gray-400 tracking-wide">{{ t('payment.expiresIn') }} <span :class="timerColorClass">{{ formattedTime }}</span></span>
            </div>

            <!-- QR Container -->
            <!-- Note: Background matches card background in dark mode for seamless look -->
            <div :class="['relative bg-white dark:bg-slate-800 group transition-transform hover:scale-[1.02] duration-300', embedMode ? 'p-2 rounded-xl shadow-md border border-gray-100 dark:border-slate-700 mb-4' : 'p-4 rounded-[2rem] shadow-lg border border-gray-100 dark:border-slate-700 mb-8']">
                 <qrcode-vue
                     :value="address"
                     :size="embedMode ? 160 : 200"
                     level="H"
                     :background="qrBackground"
                     :foreground="qrForeground"
                 />

            </div>

            <!-- Open in Wallet (mobile only) -->
            <button
                v-if="isMobile && isActive && !isSolana"
                @click="openInWallet"
                class="w-full mb-6 py-3 px-4 rounded-xl bg-blue-600 hover:bg-blue-500
                       text-white font-semibold text-sm flex items-center justify-center gap-2
                       transition-all active:scale-[0.98] shadow-lg shadow-blue-600/20 cursor-pointer"
            >
                <ExternalLinkIcon class="w-4 h-4" />
                {{ walletButtonLabel }}
            </button>

            <!-- Wallet Address -->
            <div class="w-full" :class="embedMode ? 'mb-2' : 'mb-4'">
                <div class="text-[10px] text-gray-500 dark:text-gray-400 uppercase font-bold tracking-[0.2em] mb-2 text-center">{{ t('payment.walletAddress') }}</div>
                <div class="flex items-center gap-2 bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl p-2.5 pl-4 group hover:border-gray-300 dark:hover:border-slate-600 transition-colors">
                    <span class="font-mono text-sm text-gray-700 dark:text-gray-300 truncate select-all tracking-wide">{{ truncatedAddress }}</span>
                    <button @click="copyAddress" :aria-label="t('payment.walletAddress')" class="p-2 ml-auto rounded-lg bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-600 hover:bg-gray-50 dark:hover:bg-slate-700 text-gray-400 dark:text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 transition-all cursor-pointer">
                        <CopyIcon v-if="!copied" class="w-4 h-4" />
                        <CheckIcon v-else class="w-4 h-4 text-green-500" />
                    </button>
                </div>
            </div>

            <!-- Amount to Send -->
            <div class="w-full" :class="embedMode ? 'mb-4' : 'mb-8'">
                <div class="text-[10px] text-gray-500 dark:text-gray-400 uppercase font-bold tracking-[0.2em] mb-2 text-center">{{ t('payment.amountToSend') }}</div>
                <div class="flex items-center gap-2 bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl p-2.5 pl-4 group hover:border-gray-300 dark:hover:border-slate-600 transition-colors">
                    <span class="font-mono text-sm text-gray-700 dark:text-gray-300 tracking-wide">{{ formattedExpected }} <span class="text-gray-400 dark:text-gray-500">{{ currency }}</span></span>
                    <button @click="copyAmount" :aria-label="t('payment.amountToSend')" class="p-2 ml-auto rounded-lg bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-600 hover:bg-gray-50 dark:hover:bg-slate-700 text-gray-400 dark:text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 transition-all cursor-pointer">
                        <CopyIcon v-if="!amountCopied" class="w-4 h-4" />
                        <CheckIcon v-else class="w-4 h-4 text-green-500" />
                    </button>
                </div>
            </div>

            <!-- ⚠️ Chain Warning -->
            <div class="w-full mb-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-xl px-4 py-3 flex items-start gap-2.5">
                <AlertTriangle class="w-4 h-4 text-red-500 dark:text-red-400 mt-0.5 shrink-0" />
                <p class="text-xs text-red-600 dark:text-red-400 leading-relaxed">
                    {{ t('payment.chainWarning', { network: displayNetwork, currency }) }}
                </p>
            </div>

            <!-- Token Contract Info -->
            <div v-if="currencyContract" class="w-full mb-4 flex items-center justify-center gap-1 text-[11px] text-gray-400 dark:text-gray-500">
                <span>{{ currency }} {{ t('payment.contract') }}:</span>
                <a
                    v-if="explorerUrl"
                    :href="explorerUrl"
                    target="_blank"
                    rel="noopener noreferrer"
                    class="font-mono hover:text-blue-500 dark:hover:text-blue-400 transition-colors"
                >{{ truncatedContract }}</a>
                <span v-else class="font-mono">{{ truncatedContract }}</span>
                <a v-if="explorerUrl" :href="explorerUrl" target="_blank" rel="noopener noreferrer" class="hover:text-blue-500 dark:hover:text-blue-400 transition-colors" :aria-label="t('payment.viewOnExplorer')">
                    <ExternalLinkIcon class="w-3 h-3" />
                </a>
            </div>

            <!-- Partial Payment Alert (Inline) -->
            <div v-if="status === 'underpaid'" class="w-full mb-6 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-xl p-4 flex gap-3 items-start animate-in fade-in slide-in-from-bottom-2 duration-500">
                <div class="mt-0.5 min-w-fit">
                    <AlertTriangle class="w-5 h-5 text-amber-500 dark:text-amber-400" />
                </div>
                <div>
                    <h4 class="text-sm font-bold text-amber-600 dark:text-amber-400 mb-1">{{ t('payment.partialDetected') }}</h4>
                    <p class="text-xs text-gray-600 dark:text-gray-400 leading-relaxed">
                        {{ t('payment.received') }} <span class="text-gray-900 dark:text-white font-mono font-medium">{{ formattedReceived }} {{ currency }}</span>.
                        {{ t('payment.stillNeed') }} <span class="text-red-500 dark:text-red-400 font-mono font-bold">{{ formattedRemaining }} {{ currency }}</span>.
                    </p>
                </div>
            </div>

            <!-- Status Text / Detected State -->
            <div class="mt-auto w-full pb-4">
                <!-- Payment Detected State -->
                <div v-if="isDetected" class="w-full bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-xl p-4 flex gap-3 items-center animate-in fade-in slide-in-from-bottom-2 duration-500">
                     <div class="h-5 w-5 relative flex items-center justify-center">
                        <Loader2 class="h-5 w-5 text-blue-600 dark:text-blue-400 animate-spin" />
                     </div>
                     <div>
                        <h4 class="text-sm font-bold text-blue-600 dark:text-blue-400 mb-0.5">{{ t('payment.paymentDetected') }}</h4>
                        <p class="text-xs text-gray-500 dark:text-gray-400">{{ t('payment.confirmingOnChain') }}</p>
                     </div>
                </div>

                <!-- Default Waiting State -->
                <div v-else class="flex items-center gap-2.5 opacity-60">
                     <span class="text-sm font-medium text-gray-400 dark:text-gray-500">{{ t('payment.waitingForTransaction') }}</span>
                </div>
            </div>
        </div>

        <!-- Status Overlays (Slot) -->
        <slot name="overlay"></slot>

        <!-- Footer -->
        <div :class="['px-6 border-t border-gray-100 dark:border-slate-700 flex justify-between items-center text-[11px] text-gray-400 dark:text-gray-500', embedMode ? 'py-2' : 'py-4']">
            <div class="flex items-center gap-1.5">
                <div class="w-3.5 h-3.5 bg-[#2563eb] rounded flex items-center justify-center">
                    <span class="text-[6px] font-bold text-white leading-none">IX</span>
                </div>
                <span>{{ t('payment.poweredBy') }} <span class="font-semibold text-blue-600 dark:text-blue-400">IronixPay</span></span>
            </div>
            <div class="flex gap-4">
                <a :href="locale === 'zh-CN' ? 'https://ironixpay.com/terms' : 'https://ironixpay.com/en/terms'" target="_blank" rel="noopener" class="hover:text-gray-600 dark:hover:text-gray-400 transition-colors">{{ t('payment.terms') }}</a>
                <a :href="locale === 'zh-CN' ? 'https://ironixpay.com/privacy' : 'https://ironixpay.com/en/privacy'" target="_blank" rel="noopener" class="hover:text-gray-600 dark:hover:text-gray-400 transition-colors">{{ t('payment.privacy') }}</a>
            </div>
        </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { Clock as ClockIcon, Copy as CopyIcon, Check as CheckIcon, AlertTriangle, Loader2, ExternalLink as ExternalLinkIcon } from 'lucide-vue-next';

const { t, locale } = useI18n();
import QrcodeVue from 'qrcode.vue';
import { networkDisplayName, isEvmNetwork, contractExplorerUrl } from '../../utils/networkUtils';
import { networkIcons } from '@ironix-pay/ui';

const props = defineProps<{
    timeLeft: number;
    address: string;
    merchantName: string;
    merchantLogoUrl?: string | null;
    status: string;
    network: string;
    embedMode?: boolean;
    amountReceived?: number;
    amountExpected?: number;
    currency?: string;
    /** USDT contract address from the checkout session (environment-aware) */
    currencyContract?: string;
    /** True when there are pending transactions being confirmed on blockchain */
    isConfirming?: boolean;
    /** Session livemode flag (true = production, false = sandbox) */
    livemode?: boolean;
}>();

const copied = ref(false);
const amountCopied = ref(false);
const isMobile = ref(false);
const currency = computed(() => props.currency || 'USDT');

// QR Code Colors — read dark class from DOM directly (avoids Pinia type inference issues)
const qrBackground = computed(() =>
  document.documentElement.classList.contains('dark') ? '#1e293b' : '#ffffff');
const qrForeground = computed(() =>
  document.documentElement.classList.contains('dark') ? '#ffffff' : '#000000');


const isDetected = computed(() => {
    // Show "Payment Detected" banner ONLY when there's a transaction being confirmed
    // (i.e., pendingTxHashes.length > 0 - passed as isConfirming prop)
    // NOT when there are already confirmed transactions in the session
    return props.isConfirming &&
           props.status !== 'paid' &&
           props.status !== 'overpaid' &&
           props.status !== 'expired';
});

const displayNetwork = computed(() => networkDisplayName(props.network, !props.livemode));
const networkIcon = computed(() => networkIcons[props.network] || null);

/** True if the session is on an EVM-compatible chain */
const isEvmChain = computed(() => isEvmNetwork(props.network));
const isSolana = computed(() => props.network === 'SOLANA');

/** Truncated contract address: 0x8AC7...580d */
const truncatedContract = computed(() => {
    const c = props.currencyContract;
    if (!c || c.length <= 14) return c || '';
    return `${c.slice(0, 6)}…${c.slice(-4)}`;
});

/** Block explorer URL for the token contract */
const explorerUrl = computed(() => contractExplorerUrl(props.network, props.currencyContract || ''));

const walletButtonLabel = computed(() => {
    if (isEvmChain.value) return t('payment.openInMetaMask');
    return t('payment.openInTronLink');
});

const formattedReceived = computed(() => {
    return (props.amountReceived || 0).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 6 });
});

const formattedRemaining = computed(() => {
    const remaining = (props.amountExpected || 0) - (props.amountReceived || 0);
    return Math.max(0, remaining).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 6 });
});

// Truncate address with middle ellipsis: 0x71C7656E...B5f6d8976F
const truncatedAddress = computed(() => {
    if (!props.address || props.address.length <= 20) return props.address;
    const start = props.address.slice(0, 10);
    const end = props.address.slice(-10);
    return `${start}...${end}`;
});

const formattedTime = computed(() => {
    const h = Math.floor(props.timeLeft / 3600);
    const m = Math.floor((props.timeLeft % 3600) / 60);
    const s = props.timeLeft % 60;
    return `${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
});

const timerColorClass = computed(() => {
    if (props.timeLeft < 60) return 'text-red-500 dark:text-red-400';
    if (props.timeLeft < 300) return 'text-amber-500 dark:text-amber-400';
    return 'text-gray-900 dark:text-gray-200';
});

const copyAddress = async () => {
  try {
    await navigator.clipboard.writeText(props.address);
    copied.value = true;
    setTimeout(() => copied.value = false, 2000);
  } catch (err) {
    console.error('Copy failed', err);
  }
};

const copyAmount = async () => {
  try {
    await navigator.clipboard.writeText(formattedExpected.value);
    amountCopied.value = true;
    setTimeout(() => amountCopied.value = false, 2000);
  } catch (err) {
    console.error('Copy failed', err);
  }
};

// USDT contract address is now provided via props.currencyContract
// No more hardcoded map needed — the session stores the correct contract per environment.

const formattedExpected = computed(() => {
    return (props.amountExpected || 0).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 6, useGrouping: false });
});

const isActive = computed(() => {
    return props.status !== 'paid' && props.status !== 'overpaid' && props.status !== 'expired';
});

const openInWallet = () => {
    // Use session's currency_contract directly (already env-aware)
    const usdtContract = props.currencyContract || '';
    if (!usdtContract) return;
    const amount = props.amountExpected || 0;

    if (isEvmChain.value) {
        // EVM (MetaMask / Trust Wallet) deep link
        // Use EIP-681 payment request format
        const metamaskDownload = 'https://metamask.io/download/';
        // Convert standard units to smallest EVM token unit (6 decimals for USDT/USDC)
        const microAmount = BigInt(Math.round(amount * 1_000_000));
        // USDT/USDC on EVM chains use 6 decimals, no further scaling needed for transfer()
        const eip681 = `ethereum:${usdtContract}/transfer?address=${props.address}&uint256=${microAmount}`;
        window.location.href = eip681;
        setTimeout(() => {
            if (!document.hidden) {
                if (confirm(t('payment.walletNotDetected'))) {
                    window.location.href = metamaskDownload;
                }
            }
        }, 2000);
    } else {
        // TRON (TronLink) deep link
        const deepLink = `tronlink://transfer?toAddress=${props.address}&token=${usdtContract}&amount=${amount}`;
        const downloadLink = 'https://www.tronlink.org/';
        window.location.href = deepLink;
        setTimeout(() => {
            if (!document.hidden) {
                if (confirm(t('payment.tronlinkNotDetected'))) {
                    window.location.href = downloadLink;
                }
            }
        }, 2000);
    }
};

onMounted(() => {
    isMobile.value = /Android|iPhone|iPad|iPod/i.test(navigator.userAgent);
});
</script>
