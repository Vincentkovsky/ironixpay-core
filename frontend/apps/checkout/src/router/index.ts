import { createRouter, createWebHistory } from 'vue-router';
import CheckoutView from '../views/CheckoutView.vue';

const router = createRouter({
    history: createWebHistory(),
    routes: [
        {
            path: '/checkout/:sessionId',
            name: 'checkout',
            component: CheckoutView,
        },
        {
            path: '/',
            redirect: () => {
                // In reality this might redirect to a demo or documentation
                return '/checkout/demo';
            }
        },
        // 404
        {
            path: '/:pathMatch(.*)*',
            component: {
                template: '<div class="p-8 text-center text-slate-400">Page Not Found</div>'
            }
        }
    ],
});

export default router;
