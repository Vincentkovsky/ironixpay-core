export interface MerchantStatsResponse {
    total_volume_usdt: string;
    today_volume_usdt: string;
    total_transactions: number;
    total_transactions_today: number;
}

export interface WebhookConfigResponse {
    url: string;
    secret: string;
    status: string;
    description?: string;
}

export interface UpdateWebhookConfigRequest {
    url: string;
    rotate_secret?: boolean;
    status?: string;
}

export interface WebhookLogResponse {
    id: string;
    event_type: string;
    created_at: string;
    target_url: string;
    request_payload: any;
    status: 'Pending' | 'Success' | 'Failed' | 'Retrying';
    http_status?: number;
    next_retry_at?: string;
}

export interface WebhookLogListResponse {
    logs: WebhookLogResponse[];
}

export interface BillingLogResponse {
    id: string;
    environment: string;
    merchantId: string;
    sessionId?: string;
    externalRefId?: string;
    type: 'PaymentCredit' | 'Withdrawal' | 'Refund';
    previousBalance: string;
    amountChange: string;
    balanceAfter: string;
    description?: string;
    createdAt: string;
}

export interface BillingLogListResponse {
    logs: BillingLogResponse[];
}

export interface SweepTransactionResponse {
    id: string;
    network: string;
    merchant_id: string;
    session_id?: string;
    tx_hash?: string;
    amount: string;
    from_address: string;
    to_address: string;
    status: 'Pending' | 'Broadcasted' | 'Confirmed' | 'Failed'; // Approximate enum
    error_message?: string;
    created_at: string;
    confirmed_at?: string;
}

export interface SweepHistoryResponse {
    sweeps: SweepTransactionResponse[];
}
