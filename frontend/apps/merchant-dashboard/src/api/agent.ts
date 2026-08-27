import { http } from '@/utils/request';

export interface AgentMeResponse {
    is_agent: boolean;
    agent_id?: string;
    referral_code?: string;
    base_rate?: string;
    default_merchant_rate?: string;
    referred_merchant_count: number;
}

export interface AgentOverview {
    is_agent: boolean;
    agent_id?: string;
    referral_code?: string;
    base_rate?: string;
    max_markup?: string;
    default_merchant_rate?: string;
    referred_merchant_count: number;
    total_commission: number;
}

export interface ReferredMerchantCommission {
    merchant_id: string;
    merchant_name: string;
    total_fee_collected: number;
    ironixpay_share: number;
    agent_commission: number;
    transaction_count: number;
}

export interface CommissionReport {
    agent_id: string;
    period_start: string;
    period_end: string;
    total_fee_collected: number;
    total_ironixpay_share: number;
    total_agent_commission: number;
    total_transactions: number;
    merchants: ReferredMerchantCommission[];
}

export interface ReferredMerchantInfo {
    merchant_id: string;
    name: string;
    current_rate: string;
    created_at: string;
}

/** Lightweight check — called on every login (prod only) */
export function fetchAgentMe() {
    return http.get<AgentMeResponse>('/api/internal/agent/me');
}

/** Full overview with all-time commission — called on dashboard mount */
export function fetchAgentOverview() {
    return http.get<AgentOverview>('/api/internal/agent/overview');
}

/** Date-range commission report */
export function fetchAgentCommission(params: { start_date: string; end_date: string }) {
    return http.get<CommissionReport>('/api/internal/agent/commission', { params });
}

/** List merchants referred by this agent */
export function fetchAgentMerchants() {
    return http.get<{ merchants: ReferredMerchantInfo[] }>('/api/internal/agent/merchants');
}

/** Update a referred merchant's fee rate */
export function updateMerchantRate(merchantId: string, feeRate: number) {
    return http.patch<{ merchant_id: string; custom_fee_percentage: string }>(
        `/api/internal/agent/merchants/${merchantId}/rate`,
        { fee_rate: feeRate }
    );
}
