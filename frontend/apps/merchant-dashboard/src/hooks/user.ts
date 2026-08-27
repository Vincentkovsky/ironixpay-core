import { useRouter } from 'vue-router';
import { toast } from 'vue-sonner';
import { useUserStore } from '@/stores';

export default function useUser() {
    const router = useRouter();
    const userStore = useUserStore();

    const logout = async (logoutTo?: string) => {
        await userStore.logout();
        const currentRoute = router.currentRoute.value;
        toast.success('Logged out successfully');
        router.push({
            name: logoutTo && typeof logoutTo === 'string' ? logoutTo : 'Login',
            query: {
                ...router.currentRoute.value.query,
                redirect: currentRoute.name as string,
            },
        });
    };

    return { logout };
}
