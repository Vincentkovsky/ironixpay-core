import { ref } from 'vue';
import { fetchFeeConfig, type FeeConfigResponse } from '@/api/config';

const cached = ref<FeeConfigResponse | null>(null);
const loading = ref(false);

/**
 * Composable for per-network fee configuration.
 *
 * Fetches once from backend and caches in module-level ref.
 * All components sharing this composable see the same cached data.
 */
export function useFeeConfig() {
    const load = async () => {
        if (cached.value || loading.value) return;
        loading.value = true;
        try {
            cached.value = await fetchFeeConfig();
        } catch {
            // Fallback: empty — callers will use getOutboundFee default
        } finally {
            loading.value = false;
        }
    };

    /**
     * Get outbound fee for a network in USDT (float).
     * Falls back to 1.5 if not loaded or network not found.
     */
    const getOutboundFee = (network: string): number => {
        const fees = cached.value?.outbound_fees;
        if (!fees) return 1.5;
        const val = fees[network];
        return val ? parseFloat(val) : 1.5;
    };

    /** Clear cache so next load() re-fetches (e.g. after environment switch) */
    const invalidate = () => {
        cached.value = null;
    };

    return {
        feeConfig: cached,
        loading,
        load,
        invalidate,
        getOutboundFee,
    };
}
