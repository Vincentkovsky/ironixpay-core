import { http } from '@/utils/request';
import type { BillingLogResponse } from '@ironix-pay/api-client';

export type { BillingLogResponse };

export interface BillingLogsParams {
    page?: number;
    pageSize?: number;
    /** Filter by network (e.g. "BSC", "POLYGON") */
    network?: string;
    include_sub_merchants?: boolean;
    sub_merchant_code?: string;
}

export interface PaginatedBillingLogs {
    data: BillingLogResponse[];
    total: number;
    page: number;
    pageSize: number;
    totalPages: number;
    meta?: {
        total: number;
        page: number;
        page_size: number;
        total_pages: number;
    };
}

export function queryBillingLogs(params: BillingLogsParams = {}) {
    const queryParams: Record<string, any> = {
        page: params.page || 1,
        page_size: params.pageSize || 20,
    };
    if (params.network) {
        queryParams.network = params.network;
    }
    if (params.include_sub_merchants) {
        queryParams.include_sub_merchants = true;
    }
    if (params.sub_merchant_code) {
        queryParams.sub_merchant_code = params.sub_merchant_code;
    }
    return http.get<PaginatedBillingLogs>('/api/internal/billing/logs', {
        params: queryParams,
    });
}

export function queryBillingLogDetail(id: string) {
    return http.get<BillingLogResponse>(`/api/internal/billing/logs/${id}`);
}
