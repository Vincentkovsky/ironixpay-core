import { http } from '@/utils/request';

export interface ApiKey {
    id: string;
    key: string;
    prefix: string;
    name: string;
    created_at: string;
    last_used_at: string | null;
}

export interface ApiKeysResponse {
    keys: ApiKey[];
}

export function queryApiKeys() {
    return http.get<ApiKeysResponse>('/api/internal/merchants/api-keys');
}

export interface CreateApiKeyResponse {
    id: string;
    key: string;
    prefix: string;
    name: string;
    is_test: boolean;
}

export function createApiKey(data: { name?: string; is_test?: boolean; code?: string }) {
    return http.post<CreateApiKeyResponse>('/api/internal/merchants/api-keys', data);
}

export function revokeApiKey(keyId: string) {
    return http.delete(`/api/internal/merchants/api-keys/${keyId}`);
}

// === Webhook Types ===

export interface WebhookConfig {
    url: string;
    secret: string;
    status: 'enabled' | 'disabled';
    description?: string;
    created_at: string;
}

export interface WebhookLog {
    id: string;
    eventType: string;
    createdAt: string;
    targetUrl: string;
    requestPayload: any;
    status: string;
    httpStatus?: number;
    nextRetryAt?: string;
}

export interface WebhookLogParams {
    page?: number;
    pageSize?: number;
    source_id?: string;
}

// === Webhook API ===

/** GET /api/internal/webhooks/config — returns null if not configured */
export function queryWebhookConfig() {
    return http.get<WebhookConfig | null>('/api/internal/webhooks/config');
}

/** PUT /api/internal/webhooks/config — create or update URL/status */
export function updateWebhookConfig(config: { url?: string; status?: string }) {
    return http.put<WebhookConfig>('/api/internal/webhooks/config', config);
}

/** POST /api/internal/webhooks/config/rotate-secret — rotate and return new secret */
export function rotateWebhookSecret() {
    return http.post<{ secret: string }>('/api/internal/webhooks/config/rotate-secret');
}

/** DELETE /api/internal/webhooks/config — remove webhook configuration */
export function deleteWebhookConfig() {
    return http.delete('/api/internal/webhooks/config');
}

export function queryWebhookLogs(params: WebhookLogParams) {
    return http
        .get<any>('/api/internal/webhooks/logs', { params })
        .then((res: any) => {
            let list = [];
            let total = 0;

            if (res.data && Array.isArray(res.data)) {
                list = res.data;
                total = res.meta?.total || 0;
            } else if (res.list && Array.isArray(res.list)) {
                list = res.list;
                total = res.total || 0;
            } else if (Array.isArray(res)) {
                list = res;
                total = res.length;
            }

            const transformedList = list.map((item: any) => ({
                id: item.id,
                eventType: item.event_type,
                createdAt: item.created_at,
                targetUrl: item.target_url,
                requestPayload: item.request_payload,
                status: item.status,
                httpStatus: item.http_status,
                nextRetryAt: item.next_retry_at,
            }));
            return { list: transformedList, total };
        });
}

export function resendWebhook(logId: string) {
    return http.post<{ success: boolean }>(`/api/internal/webhooks/logs/${logId}/resend`);
}
