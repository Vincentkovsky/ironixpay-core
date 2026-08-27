import { http } from '@/utils/request';

export interface ExceptionRecord {
    id: string;
    exception_type:
    | 'SessionExpired'
    | 'NoActiveSession'
    | 'SessionAlreadyCompleted'
    | 'DustPayment'
    | 'RiskBlocked'
    | 'UnderpaidExpired'
    | 'WrongToken'
    | 'Unknown';
    amount: string;
    currency: string;
    network: string;
    sender: string;
    tx_hash: string;
    session_id?: string;
    client_ref_id?: string;
    status: 'Pending' | 'Processing' | 'Resolved' | 'Failed';
    resolution?: 'Accepted' | 'Attached' | 'Swept' | 'Transferred' | null;
    created_at: string;
    resolution_tx_hash?: string;
    resolution_to_address?: string;
    available_actions: ('accept' | 'attach' | 'transfer')[];
    sub_merchant_code?: string;
}

export interface ResolutionStats {
    unresolved_count: number;
    unresolved_value: string;
    dust_count_24h: number;
}

export interface PaginatedExceptions {
    data: ExceptionRecord[];
    total: number;
    page: number;
    page_size: number;
    total_pages: number;
}

export function queryResolutionStats(params?: { include_sub_merchants?: boolean; sub_merchant_code?: string }) {
    return http.get<ResolutionStats>('/api/internal/resolution/stats', { params });
}

export interface ExceptionParams {
    page?: number;
    page_size?: number;
    status?: string;
    exception_type?: string;
    search_text?: string;
    include_sub_merchants?: boolean;
    sub_merchant_code?: string;
}

export function queryExceptions(params: ExceptionParams) {
    return http.get<PaginatedExceptions>('/api/internal/resolution/exceptions', { params });
}

export function acceptException(id: string) {
    return http.post<{ success: boolean }>(`/api/internal/resolution/exceptions/${id}/accept`);
}

export function attachException(id: string, data: { session_id: string }) {
    return http.post<{ success: boolean }>(`/api/internal/resolution/exceptions/${id}/attach`, data);
}

export function transferException(id: string, data: { code: string; to_address: string }) {
    return http.post<{ success: boolean; tx_hash: string }>(`/api/internal/resolution/exceptions/${id}/transfer`, data);
}
