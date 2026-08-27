import { createApiClient } from '@ironix-pay/api-client';
import { getToken, setToken, clearToken } from '@/utils/auth';

const API_SANDBOX = (import.meta.env.VITE_API_SANDBOX_URL as string) || 'https://sandbox.ironixpay.com';
const API_LIVE = (import.meta.env.VITE_API_LIVE_URL as string) || 'https://api.ironixpay.com';

function getApiBaseURL(): string {
    const env = localStorage.getItem('app_environment');
    return env === 'live' ? API_LIVE : API_SANDBOX;
}

// eslint-disable-next-line import/prefer-default-export
export const apiClient = createApiClient({
    baseURL: API_SANDBOX, // initial default, overridden per-request by getBaseURL
    authBaseURL: API_LIVE, // Auth always goes to production (single identity source)
    getBaseURL: getApiBaseURL, // Dynamic: reads env from localStorage on every request
    getToken,
    setToken,
    onAuthError: () => {
        // Handle session expiry
        clearToken();
        if (window.location.pathname !== '/login') {
            window.location.href = '/login';
        }
    },
    getHeaders: () => {
        const env = localStorage.getItem('app_environment') || 'sandbox';
        return {
            'X-Environment': env,
        };
    },
});
