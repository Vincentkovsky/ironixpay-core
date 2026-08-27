import { http } from '@/utils/request';

export interface SubMerchantItem {
    id: string;
    parent_org_id: string;
    sub_merchant_code: string;
    display_name: string;
    child_org_id: string;
    status: 'active' | 'suspended';
    created_at: string;
    updated_at: string;
}

export interface SubMerchantListResponse {
    items: SubMerchantItem[];
    total: number;
    page: number;
    page_size: number;
}

export function listSubMerchants(page = 1, pageSize = 20) {
    return http.get<SubMerchantListResponse>('/api/internal/sub-merchants', {
        params: { page, page_size: pageSize },
    });
}

export function createSubMerchant(data: { sub_merchant_code: string; display_name: string }) {
    return http.post<SubMerchantItem>('/api/internal/sub-merchants', data);
}

export function updateSubMerchant(
    code: string,
    data: { display_name?: string; status?: 'active' | 'suspended' },
) {
    return http.patch<SubMerchantItem>(`/api/internal/sub-merchants/${code}`, data);
}

// ─── Stats ────────────────────────────────────────────────

export interface SubMerchantStatsEntry {
    sub_merchant_code: string;
    display_name: string;
    status: 'active' | 'suspended';
    total_volume: string;
    today_volume: string;
    total_transactions: number;
    today_transactions: number;
}

export interface SubMerchantStatsSummary {
    total_volume: string;
    today_volume: string;
    total_transactions: number;
    today_transactions: number;
}

export interface SubMerchantStatsResponse {
    summary: SubMerchantStatsSummary;
    sub_merchants: SubMerchantStatsEntry[];
}

export function getSubMerchantStats() {
    return http.get<SubMerchantStatsResponse>('/api/internal/sub-merchants/stats');
}
