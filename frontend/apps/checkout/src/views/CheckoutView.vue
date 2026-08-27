<template>
  <PaymentLayout ref="paymentLayout" :loading="loading" :embed-mode="isEmbedMode">
    <template v-if="session">
        <!-- Left Panel: Details (hidden in embed mode for compactness) -->
        <OrderDetailsPanel
            v-if="!isEmbedMode"
            :amount="Number(session.amount)"
            :currency="session.currency"
            :order-id="session.client_reference_id || session.id"
            :merchant-name="session.merchant_name || 'Merchant #' + session.merchant_id.substring(0,6)"
            :merchant-logo-url="session.merchant_logo_url"
            :network="session.network"
            :livemode="session.livemode"
        />

        <!-- Right Panel: Action -->
        <PaymentActionPanel
            :time-left="timeLeft"
            :address="session.pay_address"
            :merchant-name="session.merchant_name || 'Merchant #' + session.merchant_id.substring(0,6)"
            :merchant-logo-url="session.merchant_logo_url"
            :status="session.status.toLowerCase()"
            :network="session.network"
            :livemode="session.livemode"
            :amount-received="Number(session.amount_received)"
            :amount-expected="Number(session.amount)"
            :is-confirming="pendingTxHashes.length > 0"
            :embed-mode="isEmbedMode"
            :currency="session.currency"
            :currency-contract="session.currency_contract"
        >
            <template #overlay>
                <StatusOverlay
                    :show="isPaid"
                    status="paid"
                    :transaction-hash="session.transactions?.[0]?.tx_hash"
                    :network="session.network"
                    :livemode="session.livemode"
                    :redirect-countdown="isEmbedMode ? 0 : redirectCountdown"
                >
                    <!-- Removed custom slot content to let StatusOverlay handle it consistently -->
                </StatusOverlay>

                <StatusOverlay
                    :show="isExpired"
                    status="expired"
                    :redirect-countdown="isEmbedMode ? 0 : redirectCountdown"
                >
                    <button v-if="!redirectCountdown && !isEmbedMode" @click="reload" class="mt-4 bg-slate-900 hover:bg-slate-800 dark:bg-slate-700 dark:hover:bg-slate-600 text-white px-6 py-2 rounded-lg font-medium transition-colors w-full cursor-pointer">
                        {{ t('checkout.startNewOrder') }}
                    </button>
                </StatusOverlay>
            </template>
        </PaymentActionPanel>
    </template>

    <!-- Error State -->
    <div v-else-if="error" class="p-8 text-center w-full flex items-center justify-center">
        <div class="bg-white dark:bg-slate-800 p-8 rounded-2xl border border-gray-200 dark:border-slate-700 shadow-sm max-w-md w-full">
            <h3 class="text-lg font-bold mb-2 text-red-600 dark:text-red-400">{{ t('checkout.errorTitle') }}</h3>
            <p class="text-gray-600 dark:text-gray-300">{{ error }}</p>
        </div>
    </div>
  </PaymentLayout>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, nextTick, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { useRoute } from 'vue-router';

const { t } = useI18n();
import { useCheckoutStore } from '../stores/checkout';
import { storeToRefs } from 'pinia';
import PaymentLayout from '../components/checkout/PaymentLayout.vue';
import OrderDetailsPanel from '../components/checkout/OrderDetailsPanel.vue';
import PaymentActionPanel from '../components/checkout/PaymentActionPanel.vue';
import StatusOverlay from '../components/checkout/StatusOverlay.vue';

const route = useRoute();
const store = useCheckoutStore();
const { session, loading, error, timeLeft, isPaid, isExpired, redirectCountdown, isEmbedMode, pendingTxHashes } = storeToRefs(store);

const sessionId = route.params.sessionId as string;

// Reference to the PaymentLayout component for accessing embedContent
const paymentLayout = ref<InstanceType<typeof PaymentLayout> & { embedContent?: HTMLElement } | null>(null);

// ResizeObserver for embed mode auto-height
const resizeObserver = ref<ResizeObserver | null>(null);

// Debounce and stability controls for resize
const RESIZE_DEBOUNCE_MS = 100;
const HEIGHT_CHANGE_THRESHOLD = 5; // Ignore changes smaller than 5px
let lastReportedHeight = 0;
let resizeTimeoutId: ReturnType<typeof setTimeout> | null = null;

function setupResizeObserver(): void {
    if (!store.isEmbedMode) return;

    // Clean up any existing observer first
    cleanupResizeObserver();

    // Wait for the PaymentLayout to expose its embedContent ref
    nextTick(() => {
        const contentEl = paymentLayout.value?.embedContent;
        if (!contentEl) {
            console.warn('[Embed] Could not find content element for resize observer');
            return;
        }

        resizeObserver.value = new ResizeObserver((entries) => {
            // Skip if already in terminal state
            if (isPaid.value || isExpired.value) {
                cleanupResizeObserver();
                return;
            }

            for (const entry of entries) {
                const newHeight = Math.ceil(entry.contentRect.height) + 32; // 16px padding on each side

                // Ignore small changes to prevent jitter
                if (Math.abs(newHeight - lastReportedHeight) < HEIGHT_CHANGE_THRESHOLD) {
                    continue;
                }

                // Debounce the resize report
                if (resizeTimeoutId) {
                    clearTimeout(resizeTimeoutId);
                }

                resizeTimeoutId = setTimeout(() => {
                    if (newHeight !== lastReportedHeight) {
                        lastReportedHeight = newHeight;
                        store.notifySdkResize(newHeight);
                    }
                    resizeTimeoutId = null;
                }, RESIZE_DEBOUNCE_MS);
            }
        });

        resizeObserver.value.observe(contentEl);
    });
}

function cleanupResizeObserver(): void {
    if (resizeTimeoutId) {
        clearTimeout(resizeTimeoutId);
        resizeTimeoutId = null;
    }
    if (resizeObserver.value) {
        resizeObserver.value.disconnect();
        resizeObserver.value = null;
    }
}

onMounted(() => {
    // Initialize embed mode detection first
    store.initEmbedMode();

    if (sessionId) {
        store.fetchSession(sessionId);
        store.startRealTimeUpdates(sessionId);
    }

    // Set up resize observer after content loads
    nextTick(() => {
        setupResizeObserver();
    });
});

onUnmounted(() => {
    store.stopPolling();
    store.disconnectSSE();
    store.cleanup();
    cleanupResizeObserver();
});

// Report height when session loads (only once, not on every status change)
watch(session, (newSession, oldSession) => {
    if (store.isEmbedMode && newSession && !oldSession) {
        // Initial session load - report height after render
        nextTick(() => {
            const contentEl = paymentLayout.value?.embedContent;
            if (contentEl) {
                const height = contentEl.offsetHeight + 32;
                lastReportedHeight = height;
                store.notifySdkResize(height);
            }
        });
    }
});

// Stop observing when reaching terminal states
watch([isPaid, isExpired], ([paid, expired]) => {
    if (paid || expired) {
        // Send one final resize, then stop observing
        nextTick(() => {
            const contentEl = paymentLayout.value?.embedContent;
            if (contentEl) {
                const height = contentEl.offsetHeight + 32;
                store.notifySdkResize(height);
            }
            cleanupResizeObserver();
        });
    }
});

const reload = () => window.location.reload();
</script>
