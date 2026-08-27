import { http } from '@/utils/request';

export interface FeeConfigResponse {
    deposit_fee_percentage: string;
    custom_fee_percentage?: string;
    effective_fee_percentage: string;
    outbound_fees: Record<string, string>;
    fee_tier: string;
    fee_source: string;
    first_month_ends_at?: string;
}

export function fetchFeeConfig(): Promise<FeeConfigResponse> {
    return http.get<FeeConfigResponse>('/api/internal/config/fees');
}
