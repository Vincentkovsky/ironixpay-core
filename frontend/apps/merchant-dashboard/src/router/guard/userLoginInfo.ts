import type { Router, LocationQueryRaw } from 'vue-router';
import { useUserStore } from '@/stores';
import { isLogin } from '@/utils/auth';

export default function setupUserLoginInfoGuard(router: Router) {
    router.beforeEach(async (to, _from, next) => {
        const userStore = useUserStore();

        if (isLogin()) {
            if (userStore.role) {
                next();
            } else {
                try {
                    userStore.restoreFromJwt();
                    await userStore.info();
                    next();
                } catch (error) {
                    await userStore.logout();
                    next({
                        name: 'Login',
                        query: {
                            redirect: to.name,
                            ...to.query,
                        } as LocationQueryRaw,
                    });
                }
            }
        } else {
            if (to.meta?.requiresAuth === false) {
                next();
                return;
            }
            next({
                name: 'Login',
                query: {
                    redirect: to.name,
                    ...to.query,
                } as LocationQueryRaw,
            });
        }
    });
}
