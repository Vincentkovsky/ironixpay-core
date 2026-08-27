<script setup lang="ts">
import { computed, inject, type Ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import { LogOut, Languages, Menu } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { useUserStore } from '@/stores'
import { LOCALE_OPTIONS } from '@/locale'

const { t, locale } = useI18n()
const route = useRoute()
const router = useRouter()
const userStore = useUserStore()

const pageTitle = computed(() => {
  const meta = route.meta?.locale as string
  if (meta) return t(meta)
  return t('menu.dashboard')
})

const localeSwitching = inject<Ref<boolean>>('localeSwitching')
const sidebarOpen = inject<Ref<boolean>>('sidebarOpen')

const toggleSidebar = () => {
  if (sidebarOpen) sidebarOpen.value = !sidebarOpen.value
}

const toggleLocale = () => {
  if (localeSwitching) localeSwitching.value = true
  setTimeout(() => {
    const next = locale.value === 'zh-CN' ? 'en-US' : 'zh-CN'
    locale.value = next
    localStorage.setItem('app-locale', next)
    setTimeout(() => { if (localeSwitching) localeSwitching.value = false }, 30)
  }, 150)
}

const currentLocaleLabel = computed(() => {
  return LOCALE_OPTIONS.find(o => o.value === locale.value)?.label || locale.value
})

const handleLogout = () => {
  userStore.logout()
  router.push('/login')
}
</script>

<template>
  <header class="flex h-14 shrink-0 items-center justify-between border-b border-border bg-white px-4 md:px-6">
    <div class="flex items-center gap-3">
      <!-- Hamburger (mobile only) -->
      <button
        class="flex h-10 w-10 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground transition-colors md:hidden"
        @click="toggleSidebar"
        aria-label="Open menu"
      >
        <Menu class="h-5 w-5" />
      </button>

      <!-- Page title -->
      <h1 class="text-base font-semibold capitalize text-foreground">
        {{ pageTitle }}
      </h1>
    </div>

    <!-- Right actions -->
    <div class="flex items-center gap-1">
      <!-- Language toggle -->
      <Button variant="ghost" size="sm" class="h-9 min-w-[36px] gap-1.5 text-muted-foreground hover:text-foreground" @click="toggleLocale">
        <Languages class="h-4 w-4 shrink-0" />
        <span class="hidden sm:inline">{{ currentLocaleLabel }}</span>
      </Button>
      <!-- Logout -->
      <Button variant="ghost" size="sm" class="h-9 min-w-[36px] gap-1.5 text-muted-foreground hover:text-destructive" @click="handleLogout">
        <LogOut class="h-4 w-4 shrink-0" />
        <span class="hidden sm:inline">{{ t('topbar.signOut') }}</span>
      </Button>
    </div>
  </header>
</template>
