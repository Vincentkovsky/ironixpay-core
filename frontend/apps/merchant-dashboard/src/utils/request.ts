import axios from 'axios';
import type { InternalAxiosRequestConfig, AxiosResponse, AxiosRequestConfig } from 'axios';
import { toast } from 'vue-sonner';
import { useEnvironmentStore } from '@/stores';
import { getToken, clearToken } from '@/utils/auth';
import i18n from '@/locale';

const { t } = i18n.global;

// Dedup flag — prevents multiple concurrent 401 responses from spawning duplicate toasts/redirects
let isRedirectingToLogin = false;

// API base URLs per environment
const API_URLS: Record<string, string> = {
    sandbox: import.meta.env.VITE_API_SANDBOX_URL || 'https://sandbox.ironixpay.com',
    live: import.meta.env.VITE_API_LIVE_URL || 'https://api.ironixpay.com',
};

// Auth is always against production — single identity source.
// Sandbox merchants are JIT-created from the production-signed JWT.
export const AUTH_BASE_URL = API_URLS.live;

// Routes that must always hit production (identity + invite operations)
const AUTH_ROUTES = ['/api/auth/', '/api/internal/merchants/accept-invite', '/api/internal/team/'];

function isAuthRoute(url: string | undefined): boolean {
    if (!url) return false;
    return AUTH_ROUTES.some((prefix) => url.startsWith(prefix));
}

const service = axios.create({
    timeout: 10000,
});

// Extend Axios config to support custom options
declare module 'axios' {
    interface AxiosRequestConfig {
        /** Skip the global error toast — caller handles errors itself */
        skipErrorToast?: boolean;
    }
}

// Request interceptor
service.interceptors.request.use(
    (config: InternalAxiosRequestConfig) => {
        const token = getToken();

        if (token) {
            config.headers.set('Authorization', `Bearer ${token}`);
        }

        // Auth routes always go to production (single identity source)
        if (isAuthRoute(config.url)) {
            config.baseURL = AUTH_BASE_URL;
            config.headers.set('X-Environment', 'live');
        } else {
            // Data routes follow the environment toggle
            const env = localStorage.getItem('app_environment') || 'sandbox';
            config.baseURL = API_URLS[env] || API_URLS.sandbox;
            config.headers.set('X-Environment', env);
        }

        return config;
    },
    (error) => Promise.reject(error),
);

// Response interceptor - directly returns response.data
service.interceptors.response.use(
    (response: AxiosResponse) => {
        return response.data;
    },
    (error) => {
        const status = error.response?.status;
        const errorData = error.response?.data;

        // Handle 401 Unauthorized — auto-redirect, no manual action needed
        // Deduplicate: if multiple requests fire 401 simultaneously, only handle once
        if (status === 401) {
            if (!isRedirectingToLogin) {
                isRedirectingToLogin = true;
                clearToken();
                toast.error(t('error.sessionExpired'), {
                    description: t('error.sessionExpiredDesc'),
                    duration: 2000,
                });
                setTimeout(() => { window.location.href = '/login'; }, 800);
            }
            return Promise.reject(error);
        }

        // Handle environment mismatch
        if (errorData?.error?.code === 'environment_mismatch') {
            const envStore = useEnvironmentStore();
            toast.error(t('error.envMismatch'), {
                description: t('error.envMismatchDesc', {
                    expected: errorData.error.expected,
                    current: envStore.currentEnv,
                }),
                duration: 10000,
            });
            return Promise.reject(error);
        }

        // Extract backend-specific error message
        const rawMessage =
            errorData?.error?.message ||
            errorData?.message ||
            error.message ||
            t('error.requestError');

        // Try i18n lookup by error code (e.g. "insufficient_balance" → "error.api.insufficient_balance")
        const errorCode = errorData?.error?.code;
        const i18nKey = errorCode ? `error.api.${errorCode}` : '';
        // NOTE: Don't use te() — it fails with flat dot-notation keys in legacy:false mode.
        // Instead, call t() and check if it resolved (t returns the key itself when not found).
        const translated = i18nKey ? t(i18nKey) : '';
        const message = (translated && translated !== i18nKey)
            ? translated
            : rawMessage;

        // Attach as convenience property for callers
        (error as any)._backendMessage = message;

        // Only show toast if caller didn't opt out
        if (!error.config?.skipErrorToast) {
            toast.error(message);
        }

        return Promise.reject(error);
    },
);

// Type-safe wrapper functions
export const http = {
    get<T>(url: string, config?: AxiosRequestConfig): Promise<T> {
        return service.get(url, config) as Promise<T>;
    },
    post<T>(url: string, data?: unknown, config?: AxiosRequestConfig): Promise<T> {
        return service.post(url, data, config) as Promise<T>;
    },
    put<T>(url: string, data?: unknown, config?: AxiosRequestConfig): Promise<T> {
        return service.put(url, data, config) as Promise<T>;
    },
    patch<T>(url: string, data?: unknown, config?: AxiosRequestConfig): Promise<T> {
        return service.patch(url, data, config) as Promise<T>;
    },
    delete<T>(url: string, config?: AxiosRequestConfig): Promise<T> {
        return service.delete(url, config) as Promise<T>;
    },
};

export default service;
