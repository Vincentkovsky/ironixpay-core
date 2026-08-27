import { createRouter, createWebHistory } from 'vue-router'
import { getToken, clearToken } from '@/utils/auth'

const AppLayout = () => import('@/layouts/AppLayout.vue')

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/verify-email',
      name: 'VerifyEmail',
      component: () => import('@/views/auth/VerifyEmail.vue'),
      meta: { requiresAuth: false },
    },
    {
      path: '/forgot-password',
      name: 'ForgotPassword',
      component: () => import('@/views/auth/ForgotPassword.vue'),
      meta: { requiresAuth: false },
    },
    {
      path: '/reset-password',
      name: 'ResetPassword',
      component: () => import('@/views/auth/ResetPassword.vue'),
      meta: { requiresAuth: false },
    },
    {
      path: '/verify-pending',
      name: 'VerifyPending',
      component: () => import('@/views/auth/VerifyPending.vue'),
      meta: { requiresAuth: false },
    },
    {
      path: '/login',
      name: 'Login',
      component: () => import('@/views/login/index.vue'),
      meta: { requiresAuth: false },
    },
    {
      path: '/',
      component: AppLayout,
      redirect: '/dashboard',
      children: [
        {
          path: 'dashboard',
          name: 'Dashboard',
          component: () => import('@/views/dashboard/index.vue'),
          meta: { locale: 'menu.dashboard', requiresAuth: true },
        },
        {
          path: 'sessions',
          name: 'Sessions',
          component: () => import('@/views/sessions/index.vue'),
          meta: { locale: 'menu.sessions', requiresAuth: true },
        },
        {
          path: 'session/:id',
          name: 'SessionDetail',
          component: () => import('@/views/sessions/detail.vue'),
          meta: { locale: 'menu.sessions', requiresAuth: true, hideInMenu: true, activeMenu: 'Sessions' },
        },
        {
          path: 'resolution',
          name: 'Resolution',
          component: () => import('@/views/resolution-center/index.vue'),
          meta: { locale: 'menu.resolution', requiresAuth: true },
        },
        {
          path: 'funds',
          name: 'Funds',
          component: () => import('@/views/funds/index.vue'),
          meta: { locale: 'menu.funds', requiresAuth: true },
        },
        {
          path: 'billing',
          name: 'Billing',
          component: () => import('@/views/billing/index.vue'),
          meta: { locale: 'menu.billing', requiresAuth: true },
        },
        {
          path: 'payouts',
          name: 'Payouts',
          component: () => import('@/views/payouts/index.vue'),
          meta: { locale: 'menu.payouts', requiresAuth: true },
        },
        {
          path: 'payout/:id',
          name: 'PayoutDetail',
          component: () => import('@/views/payouts/detail.vue'),
          meta: { locale: 'menu.payouts', requiresAuth: true, hideInMenu: true, activeMenu: 'Payouts' },
        },
        {
          path: 'webhooks',
          name: 'Webhooks',
          component: () => import('@/views/developer/index.vue'),
          meta: { locale: 'menu.webhooks', requiresAuth: true },
        },
        {
          path: 'api-keys',
          name: 'ApiKeys',
          component: () => import('@/views/settings/api-keys/index.vue'),
          meta: { locale: 'menu.apiKeys', requiresAuth: true },
        },
        {
          path: 'settings',
          name: 'Settings',
          component: () => import('@/views/settings/index.vue'),
          meta: { locale: 'menu.settings', requiresAuth: true },
        },
        {
          path: 'settings/2fa',
          name: 'TwoFactorAuth',
          component: () => import('@/views/settings/2fa/index.vue'),
          meta: { locale: 'menu.settings', requiresAuth: true, hideInMenu: true, activeMenu: 'Settings' },
        },
        {
          path: 'settings/team',
          name: 'Team',
          component: () => import('@/views/settings/team/index.vue'),
          meta: { locale: 'menu.team', requiresAuth: true, hideInMenu: true, activeMenu: 'Team', roles: ['owner', 'admin'] },
        },
        {
          path: 'settings/payout',
          name: 'PayoutSettings',
          component: () => import('@/views/settings/payout-settings/index.vue'),
          meta: { locale: 'menu.payoutSettings', requiresAuth: true, hideInMenu: true, activeMenu: 'PayoutSettings', roles: ['owner', 'admin'] },
        },
        {
          path: 'settings/integrations',
          name: 'Integrations',
          component: () => import('@/views/settings/integrations/index.vue'),
          meta: { locale: 'menu.integrations', requiresAuth: true, hideInMenu: true, activeMenu: 'Integrations', roles: ['owner', 'admin'] },
        },
        {
          path: 'settings/integrations/xero',
          name: 'XeroIntegration',
          component: () => import('@/views/settings/integrations/xero/index.vue'),
          meta: { locale: 'menu.integrations', requiresAuth: true, hideInMenu: true, activeMenu: 'Integrations', roles: ['owner', 'admin'] },
        },
        {
          path: 'settings/integrations/xero/callback',
          name: 'XeroCallback',
          component: () => import('@/views/settings/integrations/xero/callback.vue'),
          meta: { locale: 'menu.integrations', requiresAuth: true, hideInMenu: true, activeMenu: 'Integrations', roles: ['owner', 'admin'] },
        },
        {
          path: 'sub-merchants',
          name: 'SubMerchants',
          component: () => import('@/views/sub-merchants/index.vue'),
          meta: { locale: 'menu.subMerchants', requiresAuth: true, roles: ['owner', 'admin'] },
        },
        {
          path: 'agent',
          name: 'AgentDashboard',
          component: () => import('@/views/agent/index.vue'),
          meta: { locale: 'menu.agent', requiresAuth: true },
        },
      ],
    },
    {
      path: '/accept-invite',
      name: 'AcceptInvite',
      component: () => import('@/views/auth/AcceptInvite.vue'),
      meta: { requiresAuth: false },
    },
    {
      path: '/:pathMatch(.*)*',
      redirect: '/dashboard',
    },
  ],
})

/**
 * Parse JWT exp claim without external dependencies.
 * Returns expiry as epoch seconds, or 0 if parsing fails.
 */
function getJwtExp(token: string): number {
  try {
    const payload = token.split('.')[1] as string
    const decoded = JSON.parse(atob(payload))
    return decoded.exp || 0
  } catch {
    return 0
  }
}

// Auto-recover from stale chunk errors (e.g. after browser freezes a background tab)
router.onError((error, to) => {
  if (
    error.message.includes('Failed to fetch dynamically imported module') ||
    error.message.includes('Loading chunk')
  ) {
    window.location.assign(to.fullPath)
  }
})

// Router guard — redirect to login if token is missing or expired
router.beforeEach(async (to, _from, next) => {
  if (to.meta.requiresAuth === false) {
    return next()
  }

  const token = getToken()
  if (!token) {
    clearToken()
    return next({ path: '/login', replace: true })
  }

  const exp = getJwtExp(token)
  if (exp > 0 && Date.now() / 1000 > exp) {
    // Token expired — clear and redirect
    clearToken()
    return next({ path: '/login', replace: true })
  }

  // Role guard — check if route requires specific org roles
  const requiredRoles = to.meta.roles as string[] | undefined
  if (requiredRoles && requiredRoles.length > 0) {
    // Lazy-load user store to avoid circular imports
    const { useUserStore } = await import('@/stores')
    const userStore = useUserStore()

    // Restore org fields from JWT if store was cleared (e.g. page refresh)
    if (!userStore.orgRole) {
      userStore.restoreFromJwt()
    }

    const userRole = userStore.orgRole
    // Block if role unknown (JWT didn't have it) or not in allowed list
    if (!userRole || !requiredRoles.includes(userRole)) {
      return next({ path: '/dashboard', replace: true })
    }
  }

  next()
})

export default router
