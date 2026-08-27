import axios, {
    AxiosInstance,
    AxiosError,
    InternalAxiosRequestConfig,
} from 'axios';

// DTO Imports from bindings
import { RegisterRequest } from './bindings/RegisterRequest';
import { LoginRequest } from './bindings/LoginRequest';
import { LoginResponse } from './bindings/LoginResponse';
import { MerchantResponse } from './bindings/MerchantResponse';
import { VerifyEmailRequest } from './bindings/VerifyEmailRequest';
import { ResendVerificationRequest } from './bindings/ResendVerificationRequest';
import { Verify2FARequest } from './bindings/Verify2FARequest';
import { Enable2FARequest } from './bindings/Enable2FARequest';
import { TotpSetupResponse } from './bindings/TotpSetupResponse';
import { CreateApiKeyRequest } from './bindings/CreateApiKeyRequest';
import { ApiKeyResponse } from './bindings/ApiKeyResponse';
import { MerchantBalanceResponse } from './bindings/MerchantBalanceResponse';
import { SuccessResponse } from './bindings/SuccessResponse';
import { CreateSessionBody } from './bindings/CreateSessionBody';
import { SessionResponse } from './bindings/SessionResponse';
import { ApiKeyListResponse } from './bindings/ApiKeyListResponse';
import { SessionListResponse } from './bindings/SessionListResponse';
import { UpdateProfileRequest } from './bindings/UpdateProfileRequest';
import { UpdateWalletAddressRequest } from './bindings/UpdateWalletAddressRequest';
import { ChangePasswordRequest } from './bindings/ChangePasswordRequest';

import {
    MerchantStatsResponse,
    WebhookConfigResponse,
    UpdateWebhookConfigRequest,
    WebhookLogResponse,
    BillingLogListResponse,
    SweepHistoryResponse
} from './types';
import { DashboardStats, transformMerchantStats } from './transformers';

// Failed Request Queue Item
interface FailedQueueItem {
    resolve: (token: string) => void;
    reject: (error: any) => void;
}

export interface ApiClientConfig {
    baseURL: string;
    /** Auth routes (/api/auth/*) always hit this URL (production).
     *  If omitted, falls back to baseURL. */
    authBaseURL?: string;
    onAuthError?: () => void; // Callback when refresh fails (e.g. redirect to login)
    getToken: () => string | null;
    setToken: (token: string) => void;
    getHeaders?: () => Record<string, string>;
    /** When provided, called on every request to get the current base URL.
     *  Enables dynamic environment switching without page reload. */
    getBaseURL?: () => string;
}

export class ApiClient {
    private client: AxiosInstance;
    private isRefreshing = false;
    private failedQueue: FailedQueueItem[] = [];
    private config: ApiClientConfig;

    constructor(config: ApiClientConfig) {
        this.config = config;
        this.client = axios.create({
            baseURL: config.baseURL,
            headers: {
                'Content-Type': 'application/json',
            },
            timeout: 10000,
        });

        this.setupInterceptors();
    }

    private setupInterceptors() {
        // Request Interceptor: Inject Token + Route Auth to Production
        this.client.interceptors.request.use(
            (config: InternalAxiosRequestConfig) => {
                const token = this.config.getToken();
                if (token && config.headers) {
                    config.headers.Authorization = `Bearer ${token}`;
                }

                // Dynamic base URL (for environment switching without reload)
                if (this.config.getBaseURL) {
                    config.baseURL = this.config.getBaseURL();
                }

                // Auth routes always go to production (single identity source)
                if (this.config.authBaseURL && config.url?.startsWith('/api/auth/')) {
                    config.baseURL = this.config.authBaseURL;
                }

                // Inject custom headers (e.g. X-Environment)
                if (this.config.getHeaders && config.headers) {
                    const customHeaders = this.config.getHeaders();
                    Object.entries(customHeaders).forEach(([key, value]) => {
                        config.headers[key] = value;
                    });
                }

                return config;
            },
            (error) => Promise.reject(error)
        );

        // Response Interceptor: Handle 401
        this.client.interceptors.response.use(
            (response) => response,
            async (error: AxiosError) => {
                // Extract backend-specific error message FIRST, before any early returns.
                // This ensures consumers get meaningful messages like "Please verify your email"
                // instead of Axios generic "Request failed with status code 401"
                const errorData = error.response?.data as any;
                const backendMessage =
                    errorData?.error?.message ||
                    errorData?.message;
                if (backendMessage) {
                    error.message = backendMessage;
                }

                const originalRequest = error.config as InternalAxiosRequestConfig & {
                    _retry?: boolean;
                };

                if (error.response?.status === 401 && !originalRequest._retry) {
                    // Skip refresh logic for login and 2FA endpoints as 401 is an expected error for invalid credentials
                    if (originalRequest.url?.includes('/login') || originalRequest.url?.includes('/verify-2fa')) {
                        return Promise.reject(error);
                    }

                    if (this.isRefreshing) {
                        // If already refreshing, queue this request
                        return new Promise((resolve, reject) => {
                            this.failedQueue.push({
                                resolve: (token: string) => {
                                    if (originalRequest.headers) {
                                        originalRequest.headers.Authorization = `Bearer ${token}`;
                                    }
                                    resolve(this.client(originalRequest));
                                },
                                reject: (err) => reject(err),
                            });
                        });
                    }

                    originalRequest._retry = true;
                    this.isRefreshing = true;

                    try {
                        // Backend currently does not support refresh tokens.
                        // We must log the user out.
                        if (this.config.onAuthError) {
                            this.config.onAuthError();
                        }
                        return Promise.reject(new Error('Session expired. Please log in again.'));

                    } catch (err) {
                        this.processQueue(err, null);
                        if (this.config.onAuthError) {
                            this.config.onAuthError();
                        }
                        return Promise.reject(err);
                    } finally {
                        this.isRefreshing = false;
                    }
                }

                return Promise.reject(error);
            }
        );
    }

    private processQueue(error: any, token: string | null = null) {
        this.failedQueue.forEach((prom) => {
            if (error) {
                prom.reject(error);
            } else if (token) {
                prom.resolve(token);
            }
        });
        this.failedQueue = [];
    }

    // --- Public API Methods ---

    // Auth & Merchant
    async register(data: RegisterRequest): Promise<MerchantResponse> {
        const res = await this.client.post<MerchantResponse>('/api/auth/register', data);
        return res.data;
    }

    async login(data: LoginRequest): Promise<LoginResponse> {
        const res = await this.client.post<LoginResponse>('/api/auth/login', data);
        return res.data;
    }

    async verifyEmail(data: VerifyEmailRequest): Promise<SuccessResponse> {
        const res = await this.client.post<SuccessResponse>('/api/auth/verify-email', data);
        return res.data;
    }

    async resendVerification(data: ResendVerificationRequest): Promise<SuccessResponse> {
        const res = await this.client.post<SuccessResponse>('/api/auth/resend-verification', data);
        return res.data;
    }

    async verify2fa(data: Verify2FARequest): Promise<LoginResponse> {
        const res = await this.client.post<LoginResponse>('/api/auth/verify-2fa', data);
        return res.data;
    }

    async getProfile(): Promise<MerchantResponse> {
        const res = await this.client.get<MerchantResponse>('/api/internal/merchants/me');
        return res.data;
    }

    async createApiKey(data: CreateApiKeyRequest): Promise<ApiKeyResponse> {
        const res = await this.client.post<ApiKeyResponse>('/api/internal/merchants/api-keys', data);
        return res.data;
    }

    async revokeApiKey(keyId: string): Promise<void> {
        await this.client.delete(`/api/internal/merchants/api-keys/${keyId}`);
    }

    async getApiKeys(): Promise<ApiKeyListResponse> {
        const res = await this.client.get<ApiKeyListResponse>('/api/internal/merchants/api-keys');
        return res.data;
    }

    async getMerchantBalance(): Promise<MerchantBalanceResponse> {
        const res = await this.client.get<MerchantBalanceResponse>('/api/internal/merchants/balance');
        return res.data;
    }

    async setup2FA(): Promise<TotpSetupResponse> {
        const res = await this.client.post<TotpSetupResponse>('/api/internal/merchants/2fa/setup');
        return res.data;
    }

    async enable2FA(data: Enable2FARequest): Promise<SuccessResponse> {
        const res = await this.client.post<SuccessResponse>('/api/internal/merchants/2fa/enable', data);
        return res.data;
    }

    async disable2FA(data: Enable2FARequest): Promise<SuccessResponse> {
        const res = await this.client.post<SuccessResponse>('/api/internal/merchants/2fa/disable', data);
        return res.data;
    }

    async updateProfile(data: UpdateProfileRequest): Promise<MerchantResponse> {
        const res = await this.client.put<MerchantResponse>('/api/internal/merchants/me', data);
        return res.data;
    }

    async changePassword(data: ChangePasswordRequest): Promise<SuccessResponse> {
        const res = await this.client.put<SuccessResponse>('/api/internal/merchants/password', data);
        return res.data;
    }

    async updateWalletConfig(data: UpdateWalletAddressRequest): Promise<SuccessResponse> {
        const res = await this.client.post<SuccessResponse>('/api/internal/merchants/wallets/config', data);
        return res.data;
    }

    // Checkout
    async createSession(data: CreateSessionBody, idempotencyKey?: string): Promise<SessionResponse> {
        const headers: Record<string, string> = {};
        if (idempotencyKey) {
            headers['Idempotency-Key'] = idempotencyKey;
        }
        const res = await this.client.post<SessionResponse>('/v1/checkout/sessions', data, { headers });
        return res.data;
    }

    async listSessions(limit: number = 20): Promise<SessionListResponse> {
        const res = await this.client.get<SessionListResponse>(`/v1/checkout/sessions?limit=${limit}`);
        return res.data;
    }

    async getCheckoutSession(sessionId: string): Promise<SessionResponse> {
        const res = await this.client.get<SessionResponse>(`/v1/checkout/sessions/${sessionId}`);
        return res.data;
    }

    /**
     * Get checkout session for public display (no auth required).
     * Uses the /view endpoint which doesn't require authentication.
     */
    async getCheckoutSessionPublic(sessionId: string): Promise<SessionResponse> {
        const res = await this.client.get<SessionResponse>(`/v1/checkout/sessions/${sessionId}/view`);
        return res.data;
    }


    async getDashboardStats(): Promise<DashboardStats> {
        const res = await this.client.get<MerchantStatsResponse>('/api/internal/merchants/stats');
        return transformMerchantStats(res.data);
    }

    // Webhooks
    async getWebhookConfig(): Promise<WebhookConfigResponse | null> {
        const res = await this.client.get<WebhookConfigResponse | null>('/api/internal/webhooks/config');
        return res.data;
    }

    async updateWebhookConfig(data: UpdateWebhookConfigRequest): Promise<WebhookConfigResponse> {
        const res = await this.client.put<WebhookConfigResponse>('/api/internal/webhooks/config', data);
        return res.data;
    }

    async listWebhookLogs(page: number = 1, pageSize: number = 20): Promise<{ data: WebhookLogResponse[], total: number }> {
        const res = await this.client.get<{ data: WebhookLogResponse[], total: number, page: number, pageSize: number }>(`/api/internal/webhooks/logs?page=${page}&page_size=${pageSize}`);
        return { data: res.data.data, total: res.data.total };
    }

    async resendWebhook(logId: string): Promise<void> {
        await this.client.post(`/api/internal/webhooks/logs/${logId}/resend`);
    }

    // Billing
    async getBillingLogs(page: number = 1, pageSize: number = 20): Promise<BillingLogListResponse> {
        const res = await this.client.get<any>(`/api/internal/billing/logs?page=${page}&page_size=${pageSize}`);
        // Backend returns PaginatedResponse, we map to simple logs object expected by frontend
        return { logs: res.data.data };
    }

    async getSweepHistory(page: number = 1, pageSize: number = 20): Promise<SweepHistoryResponse> {
        const res = await this.client.get<any>(`/api/internal/billing/sweeps?page=${page}&page_size=${pageSize}`);
        return { sweeps: res.data.data };
    }
}


export function createApiClient(config: ApiClientConfig): ApiClient {
    return new ApiClient(config);
}
