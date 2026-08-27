<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, RouterLink } from 'vue-router'
import { useUserStore } from '@/stores'
import {
  LayoutGrid,
  List,
  AlertCircle,
  Wallet,
  Receipt,
  Send,
  Webhook,
  KeyRound,
  Settings,
  Users,
  ShieldCheck,
  UserCheck,
  Store,
  Plug,
} from 'lucide-vue-next'

interface NavItem {
  nameKey: string
  to: string
  icon: any
  routeName: string
  /** If set, only users with one of these org roles can see this item */
  roles?: string[]
  /** If true, only visible when merchant is an agent */
  agentOnly?: boolean
  /** Store getter key for badge count (e.g. 'pendingApprovalCount') */
  badgeCountKey?: keyof import('@/stores/user').UserState
}

interface NavGroup {
  labelKey: string
  items: NavItem[]
}

const { t } = useI18n()
const route = useRoute()
const userStore = useUserStore()

const navGroups: NavGroup[] = [
  {
    labelKey: 'menu.overview',
    items: [
      { nameKey: 'menu.dashboard', to: '/dashboard', icon: LayoutGrid, routeName: 'Dashboard' },
    ],
  },
  {
    labelKey: 'menu.payments',
    items: [
      { nameKey: 'menu.sessions', to: '/sessions', icon: List, routeName: 'Sessions' },
      { nameKey: 'menu.resolution', to: '/resolution', icon: AlertCircle, routeName: 'Resolution' },
      { nameKey: 'menu.funds', to: '/funds', icon: Wallet, routeName: 'Funds', roles: ['owner', 'admin', 'finance'] },
      { nameKey: 'menu.billing', to: '/billing', icon: Receipt, routeName: 'Billing', roles: ['owner', 'admin', 'finance'] },
      { nameKey: 'menu.payouts', to: '/payouts', icon: Send, routeName: 'Payouts', roles: ['owner', 'admin', 'finance'], badgeCountKey: 'pendingApprovalCount' },
      { nameKey: 'menu.subMerchants', to: '/sub-merchants', icon: Store, routeName: 'SubMerchants', roles: ['owner', 'admin'] },
    ],
  },
  {
    labelKey: 'menu.developer_group',
    items: [
      { nameKey: 'menu.webhooks', to: '/webhooks', icon: Webhook, routeName: 'Webhooks', roles: ['owner', 'admin', 'developer'] },
      { nameKey: 'menu.apiKeys', to: '/api-keys', icon: KeyRound, routeName: 'ApiKeys', roles: ['owner', 'admin', 'developer'] },
    ],
  },
  {
    labelKey: 'menu.account',
    items: [
      { nameKey: 'menu.settings', to: '/settings', icon: Settings, routeName: 'Settings' },
      { nameKey: 'menu.integrations', to: '/settings/integrations', icon: Plug, routeName: 'Integrations', roles: ['owner', 'admin'] },
      { nameKey: 'menu.team', to: '/settings/team', icon: Users, routeName: 'Team', roles: ['owner', 'admin'] },
      { nameKey: 'menu.payoutSettings', to: '/settings/payout', icon: ShieldCheck, routeName: 'PayoutSettings', roles: ['owner', 'admin'] },
    ],
  },
  {
    labelKey: 'menu.agent_group',
    items: [
      { nameKey: 'menu.agent', to: '/agent', icon: UserCheck, routeName: 'AgentDashboard', agentOnly: true },
    ],
  },
]

/** Filter nav items based on the user's org role. */
const filteredNavGroups = computed(() => {
  const role = userStore.orgRole
  return navGroups
    .map(group => ({
      ...group,
      items: group.items.filter(item => {
        if (item.agentOnly && !userStore.isAgent) return false
        if (!item.roles) return true
        if (!role) return true
        return item.roles.includes(role)
      }),
    }))
    .filter(group => group.items.length > 0)
})

function isActive(item: NavItem): boolean {
  const currentName = route.name as string
  const activeMeta = route.meta?.activeMenu as string | undefined
  return currentName === item.routeName || activeMeta === item.routeName
}

function badgeCount(item: NavItem): number {
  if (!item.badgeCountKey) return 0
  return (userStore[item.badgeCountKey] as number) || 0
}

function badgeLabel(count: number): string {
  return count > 99 ? '99+' : String(count)
}
</script>

<template>
  <nav class="flex flex-col gap-6 px-3 py-2">
    <div v-for="group in filteredNavGroups" :key="group.labelKey">
      <p class="mb-2 px-3 text-[10px] font-semibold uppercase tracking-[0.15em] text-muted-foreground/60">
        {{ t(group.labelKey) }}
      </p>
      <ul class="flex flex-col gap-0.5">
        <li v-for="item in group.items" :key="item.nameKey">
          <RouterLink
            :to="item.to"
            class="group relative flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors duration-150"
            :class="[
              isActive(item)
                ? 'bg-brand/8 text-brand'
                : 'text-muted-foreground hover:bg-accent hover:text-foreground',
            ]"
          >
            <!-- Active indicator bar -->
            <span
              v-if="isActive(item)"
              class="sidebar-active-indicator"
            />
            <component
              :is="item.icon"
              class="h-[18px] w-[18px] shrink-0 transition-colors duration-150"
              :class="[
                isActive(item)
                  ? 'text-brand'
                  : 'text-muted-foreground/60 group-hover:text-foreground',
              ]"
              :stroke-width="1.75"
            />
            <span>{{ t(item.nameKey) }}</span>
            <!-- Pending approval badge -->
            <span
              v-if="badgeCount(item) > 0"
              class="ml-auto inline-flex min-w-[18px] items-center justify-center rounded-full bg-red-500 px-1.5 h-[18px] text-[10px] font-bold text-white leading-none"
            >
              {{ badgeLabel(badgeCount(item)) }}
            </span>
          </RouterLink>
        </li>
      </ul>
    </div>
  </nav>
</template>
