<script setup lang="ts">
import { computed, ref, provide, watch } from 'vue'
import { useRoute } from 'vue-router'
import SidebarNav from '@/components/SidebarNav.vue'
import TopBar from '@/components/TopBar.vue'
import EnvironmentBanner from '@/components/EnvironmentBanner.vue'
import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import { useUserStore, useEnvironmentStore } from '@/stores'

const userStore = useUserStore()
const envStore = useEnvironmentStore()
const route = useRoute()

const displayName = computed(() => userStore.name || userStore.email || 'User')
const displayEmail = computed(() => userStore.email || '')
const initials = computed(() => userStore.initials || '?')

// Mobile sidebar drawer state
const sidebarOpen = ref(false)
provide('sidebarOpen', sidebarOpen)

// Close drawer on route change
watch(() => route.path, () => { sidebarOpen.value = false })

// Lock body scroll when drawer is open
watch(sidebarOpen, (open) => {
  document.body.style.overflow = open ? 'hidden' : ''
})

const closeSidebar = () => { sidebarOpen.value = false }

// Shared locale-switching state for fade animation
const localeSwitching = ref(false)
provide('localeSwitching', localeSwitching)
</script>

<template>
  <div class="flex h-screen flex-col overflow-hidden">
    <!-- Environment Banner (full width, above everything) -->
    <EnvironmentBanner />

    <div class="flex flex-1 overflow-hidden">
      <!-- Desktop Sidebar (hidden on mobile) -->
      <aside class="hidden md:flex w-60 shrink-0 flex-col bg-white border-r border-border">
        <!-- Logo -->
        <div class="flex h-14 items-center px-5">
          <img
            src="/brand/logo-wordmark.svg"
            alt="IronixPay"
            class="h-6"
          />
        </div>

        <!-- Nav -->
        <div class="flex-1 overflow-y-auto py-3">
          <SidebarNav />
        </div>

        <!-- User info at bottom -->
        <div class="border-t border-border px-5 py-4">
          <div class="flex items-center gap-3">
            <Avatar class="h-8 w-8 shrink-0">
              <AvatarFallback class="bg-brand/10 text-xs font-semibold text-brand">{{ initials }}</AvatarFallback>
            </Avatar>
            <div class="min-w-0">
              <p class="truncate text-sm font-medium text-foreground">{{ displayName }}</p>
              <p class="truncate text-[11px] text-muted-foreground">{{ displayEmail }}</p>
            </div>
          </div>
        </div>
      </aside>

      <!-- Mobile Drawer Overlay -->
      <Transition name="drawer-overlay">
        <div
          v-if="sidebarOpen"
          class="fixed inset-0 z-40 bg-black/40 backdrop-blur-[2px] md:hidden"
          @click="closeSidebar"
        />
      </Transition>

      <!-- Mobile Drawer -->
      <Transition name="drawer">
        <aside
          v-if="sidebarOpen"
          class="fixed inset-y-0 left-0 z-50 flex w-72 flex-col bg-white shadow-2xl md:hidden"
        >
          <!-- Logo -->
          <div class="flex h-14 items-center justify-between px-5">
            <img
              src="/brand/logo-wordmark.svg"
              alt="IronixPay"
              class="h-6"
            />
            <button
              class="flex h-9 w-9 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
              @click="closeSidebar"
              aria-label="Close menu"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>
            </button>
          </div>

          <!-- Nav -->
          <div class="flex-1 overflow-y-auto py-3">
            <SidebarNav />
          </div>

          <!-- User info at bottom -->
          <div class="border-t border-border px-5 py-4">
            <div class="flex items-center gap-3">
              <Avatar class="h-8 w-8 shrink-0">
                <AvatarFallback class="bg-brand/10 text-xs font-semibold text-brand">{{ initials }}</AvatarFallback>
              </Avatar>
              <div class="min-w-0">
                <p class="truncate text-sm font-medium text-foreground">{{ displayName }}</p>
                <p class="truncate text-[11px] text-muted-foreground">{{ displayEmail }}</p>
              </div>
            </div>
          </div>
        </aside>
      </Transition>

      <!-- Main content area -->
      <div class="flex flex-1 flex-col overflow-hidden">
        <!-- Top bar -->
        <TopBar />

        <!-- Content -->
        <main class="flex-1 overflow-y-auto bg-background">
          <div class="mx-auto max-w-6xl px-4 py-5 md:p-6 lg:p-8 locale-fade" :class="{ 'locale-switching': localeSwitching }">
            <RouterView v-slot="{ Component }">
              <component :is="Component" :key="envStore.currentEnv" />
            </RouterView>
          </div>
        </main>
      </div>
    </div>
  </div>
</template>

<style scoped>
.locale-fade {
  transition: opacity 0.15s ease, transform 0.15s ease;
}
.locale-switching {
  opacity: 0.3;
  transform: translateY(2px);
}

/* Drawer slide-in */
.drawer-enter-active,
.drawer-leave-active {
  transition: transform 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}
.drawer-enter-from,
.drawer-leave-to {
  transform: translateX(-100%);
}

/* Overlay fade */
.drawer-overlay-enter-active,
.drawer-overlay-leave-active {
  transition: opacity 0.25s ease;
}
.drawer-overlay-enter-from,
.drawer-overlay-leave-to {
  opacity: 0;
}
</style>
