import { MerchantStatsResponse } from './types';

export interface DashboardStats {
    totalVolume: number;
    todayVolume: number;
    totalTransactions: number;
    todayTransactions: number;
    // We keep the original strings in case we need high precision display later
    original: MerchantStatsResponse;
}

/**
 * Transforms backend stats (strings) to frontend numbers for charts/display.
 * Note regarding precision:
 * JavaScript numbers are floats. Large crypto integers (in smallest units) might lose precision.
 * However, these stats are likely "USDT major units" (formatted in backend) or we are converting for
 * high-level dashboard display where strict atomic precision is less critical than graph readability.
 * If exact calculation is needed, use BigNumber/Decimal library on the original strings.
 */
export function transformMerchantStats(dto: MerchantStatsResponse): DashboardStats {
    return {
        totalVolume: parseFloat(dto.total_volume_usdt || '0'),
        todayVolume: parseFloat(dto.today_volume_usdt || '0'),
        totalTransactions: dto.total_transactions,
        todayTransactions: dto.total_transactions_today,
        original: dto
    };
}
