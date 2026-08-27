import { http } from '@/utils/request';
import type { SessionStatus } from '@ironix-pay/api-client';

export interface PricingInfo {
    currency: string;
    amount: string;
    exchange_rate: string;
}

export interface SessionRecord {
    id: string;
    amount: number;
    status: SessionStatus;
    network: string;
    createdTime: string;
    currency: string;
    amountReceived: number;
    clientReferenceId?: string;
    payAddress: string;
    url: string;
    pricing?: PricingInfo | null;
    sub_merchant_code?: string;
}

export interface SessionParams {
    current: number;
    pageSize: number;
    id?: string;
    status?: string;
    network?: string;
    startDate?: string;
    endDate?: string;
    include_sub_merchants?: boolean;
    sub_merchant_code?: string;
}

export interface SessionListRes {
    list: SessionRecord[];
    total: number;
}

export function querySessionList(params: SessionParams) {
    const backendParams: Record<string, string | number | boolean | undefined> = {
        page: params.current,
        page_size: params.pageSize,
        search_text: params.id || undefined,
        status: params.status || undefined,
        network: params.network || undefined,
        created_after: params.startDate || undefined,
        created_before: params.endDate || undefined,
        include_sub_merchants: params.include_sub_merchants || undefined,
        sub_merchant_code: params.sub_merchant_code || undefined,
    };

    // Build query string, skipping null/empty values
    const searchParams = new URLSearchParams();
    Object.entries(backendParams).forEach(([k, v]) => {
        if (v != null && v !== '') searchParams.append(k, String(v));
    });

    return http
        .get<any>(`/v1/checkout/sessions?${searchParams.toString()}`)
        .then((res: any) => {
            const rawData = res.data || [];
            const total = res.meta ? res.meta.total : 0;

            const list = rawData.map((item: any) => ({
                id: item.id,
                amount: parseFloat(item.amount),
                status: item.status,
                network: item.network || 'TRON',
                createdTime: item.created_at,
                currency: item.currency || 'USDT',
                amountReceived: parseFloat(item.amount_received),
                clientReferenceId: item.client_reference_id,
                payAddress: item.pay_address,
                url: item.url,
                expiresAt: item.expires_at,
                transactions: item.transactions || [],
                pricing: item.pricing || null,
                sub_merchant_code: item.sub_merchant_code || undefined,
            }));

            return { list, total };
        });
}

export interface TransactionRecord {
    txHash: string;
    network: string;
    amount: number;
    status: string;
    time: string;
}

export interface SessionDetail extends SessionRecord {
    merchantName?: string;
    network: string;
    amountExpected: number;
    expiresAt: string;
    feeAmount?: number;
    netAmount?: number;
    transactions: TransactionRecord[];
}

export function querySessionDetail(id: string) {
    return http.get<any>(`/v1/checkout/sessions/${id}`).then((data: any) => {
        const detail: SessionDetail = {
            id: data.id,
            merchantName: data.merchant_name,
            amount: parseFloat(data.amount),
            amountExpected: parseFloat(data.amount),
            amountReceived: parseFloat(data.amount_received),
            status: data.status,
            network: data.network,
            currency: data.currency,
            payAddress: data.pay_address,
            clientReferenceId: data.client_reference_id,
            url: data.url,
            createdTime: data.created_at,
            expiresAt: data.expires_at,
            feeAmount: data.fee_amount ? parseFloat(data.fee_amount) : undefined,
            netAmount: data.net_amount ? parseFloat(data.net_amount) : undefined,
            pricing: data.pricing || null,
            transactions: (data.transactions || []).map((tx: any) => ({
                txHash: tx.tx_hash,
                network: tx.network || data.network,
                amount: parseFloat(tx.amount),
                status: tx.status,
                time: tx.created_at,
            })),
        };
        return { data: detail };
    });
}
