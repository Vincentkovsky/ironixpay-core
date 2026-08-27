import { http } from '@/utils/request';

// ── Types ──

export interface PayoutSettingsResponse {
    requireNewAddressApproval: boolean;
    approvalThreshold: string; // human-readable USDT, e.g. "5000"
    approverRoles: string[];   // e.g. ["owner", "admin"]
    autoWithdrawEnabled: boolean;
    autoWithdrawThreshold: string | null;
    autoWithdrawNetwork: string | null;
    autoWithdrawCurrency: string;
}

export interface UpdatePayoutSettingsRequest {
    requireNewAddressApproval?: boolean;
    approvalThreshold?: string;
    approverRoles?: string[];
    autoWithdrawEnabled?: boolean;
    autoWithdrawThreshold?: string | null;
    autoWithdrawNetwork?: string | null;
    autoWithdrawCurrency?: string | null;
}

export interface ApproveRequest {
    totpCode: string;
}

export interface RejectRequest {
    totpCode: string;
    reason?: string;
}

// ── Settings ──

export function getPayoutSettings() {
    return http.get<PayoutSettingsResponse>('/api/internal/merchants/settings/payout');
}

export function updatePayoutSettings(data: UpdatePayoutSettingsRequest) {
    return http.put<PayoutSettingsResponse>('/api/internal/merchants/settings/payout', data);
}

// ── Payout Approve / Reject ──

export function approvePayout(id: string, totpCode: string) {
    return http.post(`/api/internal/merchants/payouts/${id}/approve`, { totpCode });
}

export function rejectPayout(id: string, totpCode: string, reason?: string) {
    return http.post(`/api/internal/merchants/payouts/${id}/reject`, { totpCode, reason });
}

// ── Withdrawal Approve / Reject ──

export function approveWithdrawal(id: string, totpCode: string) {
    return http.post(`/api/internal/merchants/withdrawals/${id}/approve`, { totpCode });
}

export function rejectWithdrawal(id: string, totpCode: string, reason?: string) {
    return http.post(`/api/internal/merchants/withdrawals/${id}/reject`, { totpCode, reason });
}
