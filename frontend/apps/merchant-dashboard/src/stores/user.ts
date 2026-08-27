import { defineStore } from 'pinia';
import { apiClient } from '@/api/client';
import type { LoginRequest, MerchantResponse } from '@ironix-pay/api-client';
import { setToken, clearToken, getToken } from '@/utils/auth';
import { fetchAgentMe } from '@/api/agent';
import { fetchPendingApprovalCount } from '@/api/withdrawal';
import type { AgentMeResponse } from '@/api/agent';

export type OrgRole = 'owner' | 'admin' | 'developer' | 'finance' | 'viewer';

export interface UserState {
    name?: string;
    avatar?: string;
    email?: string;
    phone?: string;
    accountId?: string;
    /** Current user's ID (from users table) */
    userId?: string;
    /** Current organization ID (= merchant ID) */
    orgId?: string;
    /** Current organization display name */
    orgName?: string;
    /** Current user's role in the organization */
    orgRole?: OrgRole;
    /** Aggregated USDT balance across all chains (human-readable) */
    balance?: number;
    /** Per-chain USDT balances for current environment: { TRON: 100.5, BSC: 50.2 } */
    chainBalances: Record<string, number>;
    /** Aggregated USDC balance across all chains (human-readable) */
    usdcBalance?: number;
    /** Per-chain USDC balances for current environment: { BSC: 10.5, ETH: 5.2 } */
    chainUsdcBalances: Record<string, number>;
    /** Legacy single address (primary chain for current env) — backward compat */
    collectionAddress?: string;
    /**
     * Per-chain withdrawal/payout destination addresses: { TRON: "T...", BSC: "0x..." }
     * NOTE: Named "collection" historically — these are where the merchant COLLECTS payouts,
     * NOT the HD-derived addresses where customers pay.
     */
    collectionAddresses: Record<string, string | null>;
    is_2fa_enabled?: boolean;
    role: '' | '*' | 'admin' | 'user';
    /** Whether the current merchant is an agent (production only) */
    isAgent: boolean;
    /** Cached lightweight agent info from /agent/me */
    agentInfo: AgentMeResponse | null;
    /** Count of PendingApproval withdrawals + payouts */
    pendingApprovalCount: number;
}

/**
 * Extract per-chain addresses for the current environment from MerchantResponse.
 * Returns a flat map: { TRON: "Txxx", BSC: "0xxx" }
 */
function extractAddresses(
    res: MerchantResponse,
    isSandbox: boolean,
): Record<string, string | null> {
    const envKey = isSandbox ? 'sandbox' : 'production';
    const addrs: Record<string, string | null> = {};

    if (res.collection_addresses) {
        for (const [network, envMap] of Object.entries(res.collection_addresses)) {
            addrs[network] = envMap[envKey] ?? null;
        }
    }

    // Backward compat: if collection_addresses is empty, fall back to legacy fields
    if (Object.keys(addrs).length === 0) {
        const legacy = isSandbox
            ? (res.collection_address_sandbox ?? res.collection_address)
            : res.collection_address;
        if (legacy) {
            addrs['TRON'] = legacy;
        }
    }

    return addrs;
}

/** Pick the "primary" address for display — prefer TRON, then first available */
function primaryAddress(addrs: Record<string, string | null>): string | undefined {
    if (addrs['TRON']) return addrs['TRON']!;
    const first = Object.values(addrs).find(v => v);
    return first ?? undefined;
}

/**
 * Extract per-chain balances for the current environment from MerchantResponse.
 * Returns { TRON: 100.5, BSC: 50.2 } (human-readable USDT)
 */
function extractChainBalances(
    res: MerchantResponse,
    isSandbox: boolean,
): Record<string, number> {
    const envKey = isSandbox ? 'sandbox' : 'production';
    const balances: Record<string, number> = {};

    if (res.chain_balances) {
        for (const [network, envMap] of Object.entries(res.chain_balances)) {
            const raw = envMap[envKey];
            if (raw !== undefined) {
                balances[network] = Number(raw);
            }
        }
    }

    return balances;
}

/**
 * Extract per-chain USDC balances for the current environment from MerchantResponse.
 * Returns { BSC: 10.5, ETH: 5.2 } (human-readable USDC)
 */
function extractChainUsdcBalances(
    res: MerchantResponse,
    isSandbox: boolean,
): Record<string, number> {
    const envKey = isSandbox ? 'sandbox' : 'production';
    const balances: Record<string, number> = {};

    if ((res as any).chain_usdc_balances) {
        for (const [network, envMap] of Object.entries((res as any).chain_usdc_balances)) {
            const raw = (envMap as any)[envKey];
            if (raw !== undefined) {
                balances[network] = Number(raw);
            }
        }
    }

    return balances;
}

export const useUserStore = defineStore('user', {
    state: (): UserState => ({
        name: undefined,
        avatar: undefined,
        email: undefined,
        phone: undefined,
        accountId: undefined,
        userId: undefined,
        orgId: undefined,
        orgName: undefined,
        orgRole: undefined,
        balance: 0,
        chainBalances: {},
        usdcBalance: 0,
        chainUsdcBalances: {},
        collectionAddress: undefined,
        collectionAddresses: {},
        is_2fa_enabled: false,
        role: '',
        isAgent: false,
        agentInfo: null,
        pendingApprovalCount: 0,
    }),

    getters: {
        userInfo(state: UserState): UserState {
            return { ...state };
        },
        initials(state: UserState): string {
            if (!state.name) return '?';
            return state.name
                .split(' ')
                .map((w) => w[0])
                .join('')
                .toUpperCase()
                .slice(0, 2);
        },
    },

    actions: {
        setInfo(partial: Partial<UserState>) {
            this.$patch(partial);
        },

        resetInfo() {
            this.$reset();
        },

        /**
         * Restore org fields from JWT claims on page refresh.
         * JWT has: sub (org_id), uid (user_id), role (org role).
         * This avoids losing orgRole after a full page reload.
         */
        restoreFromJwt() {
            const token = getToken();
            if (!token) return;

            try {
                const payload = token.split('.')[1] as string;
                const decoded = JSON.parse(atob(payload));
                if (decoded.sub && !this.orgId) {
                    this.orgId = decoded.sub;
                    this.accountId = decoded.sub;
                }
                if (decoded.uid && !this.userId) {
                    this.userId = decoded.uid;
                }
                if (decoded.role && !this.orgRole) {
                    this.orgRole = decoded.role as OrgRole;
                }
            } catch {
                // Malformed JWT — ignore
            }
        },

        async info() {
            try {
                const { useEnvironmentStore } = await import('@/stores');
                const envStore = useEnvironmentStore();

                if (envStore.isSandbox) {
                    // Sandbox: fetch identity from prod + balance from sandbox in parallel.
                    // Sandbox DB only has JIT "Shadow Account" placeholders.
                    const { AUTH_BASE_URL } = await import('@/utils/request');

                    // Local dev: both URLs point to the same backend, skip dual request
                    const isSameBackend = AUTH_BASE_URL === (import.meta.env.VITE_API_SANDBOX_URL || '');
                    if (isSameBackend) {
                        const res = await apiClient.getProfile();
                        const rawBalance = res.balance_sandbox;
                        const balance = Number(rawBalance);
                        const addresses = extractAddresses(res, true);
                        const collectionAddress = primaryAddress(addresses);
                        const chainBalances = extractChainBalances(res, true);
                        const usdcBalance = Number((res as any).usdc_balance_sandbox || 0);
                        const chainUsdcBalances = extractChainUsdcBalances(res, true);
                        // Preserve org fields from JWT (login set them; info() must not overwrite)
                        this.setInfo({ ...res, name: res.user_name || res.name, orgName: res.org_name || res.name, balance, chainBalances, usdcBalance, chainUsdcBalances, collectionAddress, collectionAddresses: addresses, role: 'user', orgRole: this.orgRole, userId: this.userId, orgId: this.orgId });
                    } else {
                        const { getToken } = await import('@/utils/auth');
                        const token = getToken();
                        const headers: Record<string, string> = {
                            'X-Environment': 'live',
                        };
                        if (token) headers.Authorization = `Bearer ${token}`;

                        const [prodRes, sandboxRes] = await Promise.all([
                            import('axios').then(({ default: axios }) =>
                                axios.get(`${AUTH_BASE_URL}/api/internal/merchants/me`, { headers }),
                            ),
                            apiClient.getProfile(),
                        ]);

                        const identity = prodRes.data; // real name, email
                        const rawBalance = sandboxRes.balance_sandbox;
                        const balance = Number(rawBalance);
                        const addresses = extractAddresses(sandboxRes, true);
                        const collectionAddress = primaryAddress(addresses);
                        const chainBalances = extractChainBalances(sandboxRes, true);
                        const usdcBalance = Number((sandboxRes as any).usdc_balance_sandbox || 0);
                        const chainUsdcBalances = extractChainUsdcBalances(sandboxRes, true);
                        this.setInfo({ ...identity, name: identity.user_name || identity.name, orgName: identity.org_name || identity.name, balance, chainBalances, usdcBalance, chainUsdcBalances, collectionAddress, collectionAddresses: addresses, role: 'user', orgRole: this.orgRole, userId: this.userId, orgId: this.orgId });
                    }
                } else {
                    const res = await apiClient.getProfile();
                    const rawBalance = res.balance_prod;
                    const balance = Number(rawBalance);
                    const addresses = extractAddresses(res, false);
                    const collectionAddress = primaryAddress(addresses);
                    const chainBalances = extractChainBalances(res, false);
                    const usdcBalance = Number((res as any).usdc_balance_prod || 0);
                    const chainUsdcBalances = extractChainUsdcBalances(res, false);

                    this.setInfo({ ...res, name: res.user_name || res.name, orgName: res.org_name || res.name, email: res.email, balance, chainBalances, usdcBalance, chainUsdcBalances, collectionAddress, collectionAddresses: addresses, role: 'user', orgRole: this.orgRole, userId: this.userId, orgId: this.orgId });

                    // Agent check — production only (agent_profiles don't exist in sandbox)
                    // Fire-and-forget: never blocks login flow
                    fetchAgentMe()
                        .then((agentRes) => {
                            this.isAgent = agentRes.is_agent;
                            this.agentInfo = agentRes;
                        })
                        .catch(() => {
                            this.isAgent = false;
                            this.agentInfo = null;
                        });
                }
            } catch (e: any) {
                console.error('Failed to get user info:', {
                    status: e.response?.status,
                    data: e.response?.data,
                    message: e.message,
                });
                // Only clear token on explicit 401 (auth failure).
                // Network errors, 429 rate-limits, etc. should NOT force logout.
                if (e.response?.status === 401) {
                    clearToken();
                }
                throw e;
            }

            // Start pending approval polling after profile load
            this.startPendingApprovalPolling();
        },

        async login(loginForm: LoginRequest) {
            try {
                const res = await apiClient.login(loginForm);
                if (res.status === 'success') {
                    setToken(res.token);
                    this.setInfo({
                        accountId: res.merchant_id,
                        userId: res.user_id,
                        orgId: res.merchant_id,
                        orgRole: res.role as OrgRole,
                        orgName: res.org_name || res.merchant.name,
                        name: res.merchant.user_name || res.merchant.name,
                        email: res.merchant.email,
                        role: 'user',
                    });
                    return null;
                }
                if (res.status === 'requires_2fa') {
                    return res.temp_token;
                }
                return null;
            } catch (err) {
                clearToken();
                throw err;
            }
        },

        async verify2fa(tempToken: string, code: string) {
            try {
                const res = await apiClient.verify2fa({
                    temp_token: tempToken,
                    code,
                });
                if (res.status === 'success') {
                    setToken(res.token);
                    this.setInfo({
                        accountId: res.merchant_id,
                        userId: res.user_id,
                        orgId: res.merchant_id,
                        orgRole: res.role as OrgRole,
                        orgName: res.org_name || res.merchant.name,
                        name: res.merchant.user_name || res.merchant.name,
                        email: res.merchant.email,
                        role: 'user',
                    });
                } else {
                    throw new Error('2FA verification failed');
                }
            } catch (err) {
                clearToken();
                throw err;
            }
        },

        async register(registerForm: any) {
            return apiClient.register(registerForm);
        },

        logoutCallBack() {
            this.resetInfo();
            clearToken();
        },

        async logout() {
            try {
                // await userLogout();
            } finally {
                this.stopPendingApprovalPolling();
                this.logoutCallBack();
            }
        },

        // ── Pending Approval Polling ──

        startPendingApprovalPolling() {
            // Prevent duplicate intervals
            if ((this as any)._pendingPollTimer) return;

            const poll = () => fetchPendingApprovalCount().then(c => { this.pendingApprovalCount = c; });
            poll(); // immediate first fetch

            (this as any)._pendingPollTimer = setInterval(poll, 30_000);

            // Pause when tab hidden, resume + immediate fetch when visible
            const onVisChange = () => {
                if (document.hidden) {
                    // Pause: stop interval to avoid wasting requests
                    if ((this as any)._pendingPollTimer) {
                        clearInterval((this as any)._pendingPollTimer);
                        (this as any)._pendingPollTimer = null;
                    }
                } else {
                    // Resume: immediate fetch + restart interval
                    poll();
                    if (!(this as any)._pendingPollTimer) {
                        (this as any)._pendingPollTimer = setInterval(poll, 30_000);
                    }
                }
            };
            document.addEventListener('visibilitychange', onVisChange);
            (this as any)._pendingVisHandler = onVisChange;
        },

        stopPendingApprovalPolling() {
            if ((this as any)._pendingPollTimer) {
                clearInterval((this as any)._pendingPollTimer);
                (this as any)._pendingPollTimer = null;
            }
            if ((this as any)._pendingVisHandler) {
                document.removeEventListener('visibilitychange', (this as any)._pendingVisHandler);
                (this as any)._pendingVisHandler = null;
            }
            this.pendingApprovalCount = 0;
        },
    },
});
