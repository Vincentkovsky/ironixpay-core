import { http } from '@/utils/request';

export interface WithdrawalResponse {
    id: string;
    merchantId: string;
    environment: string;
    amount: string;
    fee: string;
    netAmount: string;
    toAddress: string;
    network: string;
    status: 'Pending' | 'PendingApproval' | 'Processing' | 'Completed' | 'Failed' | 'Cancelled' | 'ApprovalExpired';
    txHash: string | null;
    errorReason: string | null;
    currency: string;
    requestedBy: string | null;
    reviewedBy: string | null;
    reviewedAt: string | null;
    createdAt: string;
    completedAt: string | null;
}

export interface PaginatedWithdrawals {
    data: WithdrawalResponse[];
    total: number;
    page: number;
    pageSize: number;
    totalPages: number;
}

export function requestWithdrawal(amount: string, totpCode: string, network: string = 'TRON', currency: string = 'USDT') {
    return http.post<WithdrawalResponse>('/api/internal/merchants/withdrawals', {
        amount,
        totpCode,
        network,
        currency,
    });
}

export function listWithdrawals(page = 1, pageSize = 20) {
    return http.get<PaginatedWithdrawals>('/api/internal/merchants/withdrawals', {
        params: { page, page_size: pageSize },
    });
}

export function fetchPendingApprovalCount(): Promise<number> {
    return http
        .get<{ pending_approvals: number }>('/api/internal/merchants/notifications/pending-count')
        .then((res: any) => res.pending_approvals ?? 0)
        .catch(() => 0);
}
