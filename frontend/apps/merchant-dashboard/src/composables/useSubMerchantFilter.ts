import { ref, computed } from 'vue';
import { listSubMerchants, type SubMerchantItem } from '@/api/sub-merchant';

// ─── Module-level cache (shared across all pages) ────────────
const _subMerchants = ref<SubMerchantItem[]>([]);
const _loaded = ref(false);
const _hasSubMerchants = computed(() => _subMerchants.value.length > 0);

async function _loadSubMerchants() {
    if (_loaded.value) return;
    try {
        const res = await listSubMerchants(1, 100);
        _subMerchants.value = res.items.filter((sm) => sm.status === 'active');
        _loaded.value = true;
    } catch {
        // Fail silently — filter simply won't appear
    }
}

/**
 * Shared composable for sub-merchant filtering across list pages.
 *
 * The sub-merchant list is cached at module level (one API call shared by
 * all pages). Each page gets its own `selected` filter value.
 */
export function useSubMerchantFilter() {
    const selected = ref<string>('_all');

    /** Query params to merge into API calls. */
    const filterParams = computed(() => {
        if (selected.value === '_all') {
            return { include_sub_merchants: true };
        }
        if (selected.value === '_self') {
            return {}; // backend default: only parent records
        }
        // Specific sub-merchant code
        return { include_sub_merchants: true, sub_merchant_code: selected.value };
    });

    /** Reset filter to "All". */
    function resetFilter() {
        selected.value = '_all';
    }

    return {
        subMerchants: _subMerchants,
        selected,
        hasSubMerchants: _hasSubMerchants,
        filterParams,
        loadSubMerchants: _loadSubMerchants,
        resetFilter,
    };
}

// ─── Badge color helper ─────────────────────────────────────

const SM_COLORS = [
    'bg-blue-50 text-blue-700 border-blue-200',
    'bg-emerald-50 text-emerald-700 border-emerald-200',
    'bg-violet-50 text-violet-700 border-violet-200',
    'bg-amber-50 text-amber-700 border-amber-200',
    'bg-rose-50 text-rose-700 border-rose-200',
    'bg-cyan-50 text-cyan-700 border-cyan-200',
];

/**
 * Returns a deterministic Tailwind class string for a given sub-merchant code.
 * Same code always maps to the same color.
 */
export function smColorClass(code: string): string {
    let hash = 0;
    for (const ch of code) hash = ((hash << 5) - hash + ch.charCodeAt(0)) | 0;
    const idx = Math.abs(hash) % SM_COLORS.length;
    return SM_COLORS[idx]!;
}
