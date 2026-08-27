import { defineStore } from 'pinia';
import router from '@/router';

export interface EnvironmentState {
    currentEnv: 'live' | 'sandbox';
}

export const useEnvironmentStore = defineStore('environment', {
    state: (): EnvironmentState => {
        const storedEnv = localStorage.getItem('app_environment');
        return {
            currentEnv:
                storedEnv === 'live' || storedEnv === 'sandbox'
                    ? storedEnv
                    : 'sandbox',
        };
    },

    getters: {
        isSandbox(state: EnvironmentState): boolean {
            return state.currentEnv === 'sandbox';
        },
        envName(state: EnvironmentState): string {
            return state.currentEnv === 'sandbox' ? 'Sandbox' : 'Live';
        },
    },

    actions: {
        async toggleEnvironment(env: 'live' | 'sandbox') {
            this.currentEnv = env;
            localStorage.setItem('app_environment', env);

            // Detail pages (e.g. /session/:id) contain environment-specific IDs
            // that won't exist in the other environment. Redirect to Dashboard
            // to avoid 404 errors. List pages stay in place — remount handles refresh.
            const route = router.currentRoute.value;
            const hasParams = Object.keys(route.params).length > 0;
            if (hasParams) {
                router.push({ name: 'Dashboard' });
            }

            // Re-fetch user data for the new environment (balance, identity).
            // Layout (sidebar name/balance) doesn't remount with router-view :key,
            // so we need this explicit call. Page-level data refreshes automatically
            // because :key forces component remount → onMounted re-fires.
            try {
                const { useUserStore } = await import('@/stores');
                const userStore = useUserStore();
                await userStore.info();
            } catch {
                // Profile fetch may fail transiently; UI still shows correctly
            }

            // Invalidate fee config cache so it re-fetches for the new environment.
            // Each environment may have different enabled networks / fee schedules.
            try {
                const { useFeeConfig } = await import('@/composables/useFeeConfig');
                const { invalidate, load } = useFeeConfig();
                invalidate();
                await load();
            } catch {
                // Non-critical; billing page will retry on next mount
            }
        },
    },
});
