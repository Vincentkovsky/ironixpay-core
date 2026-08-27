<script setup lang="ts">
import {
  ChevronRight,
  ChevronDown,
  LayoutDashboard
} from 'lucide-vue-next'
import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'

interface NavigationItem {
  name: string
  href?: string
  icon?: any
  children?: NavigationItem[]
}

const props = defineProps<{
  title: string
  subtitle?: string
  logo?: any // Component
  navigation: NavigationItem[]
}>()

const route = useRoute()
const currentRoute = computed(() => route.path)

// State for expandable menus
const expandedMenus = ref<Record<string, boolean>>({})

const toggleMenu = (name: string) => {
  expandedMenus.value[name] = !expandedMenus.value[name]
}

// Initialize expanded state based on current route
props.navigation.forEach(item => {
    if (item.children) {
        // If child is active, expand parent
        if (item.children.some(child => child.href && currentRoute.value.startsWith(child.href))) {
            expandedMenus.value[item.name] = true
        }
    }
})
</script>

<template>
  <div class="flex h-screen w-64 flex-col bg-[#0B1120] border-r border-gray-800 text-white transition-all duration-300">
    <!-- Logo Area -->
    <div class="flex h-20 items-center px-6">
      <div class="flex items-center gap-3 font-bold text-xl tracking-tight">
        <div class="h-9 w-9 rounded-xl bg-gradient-to-br from-blue-500 to-blue-600 flex items-center justify-center shadow-lg shadow-blue-500/20">
          <component :is="logo || LayoutDashboard" class="h-5 w-5 text-white" />
        </div>
        <span class="text-white">{{ title }}</span>
      </div>
      <div v-if="subtitle" class="ml-2 flex flex-col">
          <span class="text-[10px] text-gray-500 leading-none">{{ subtitle }}</span>
      </div>
    </div>

    <!-- Navigation List -->
    <nav class="flex-1 space-y-1 px-4 py-4 overflow-y-auto custom-scrollbar">
      <slot name="header-actions"></slot>

      <template v-for="item in navigation" :key="item.name">
        <!-- Single Item -->
        <router-link
          v-if="!item.children && item.href"
          :to="item.href"
          class="group flex items-center rounded-lg px-3 py-2.5 text-sm font-medium transition-all duration-200"
          :class="[
            currentRoute.startsWith(item.href)
              ? 'text-blue-400 bg-blue-500/10'
              : 'text-gray-400 hover:text-white hover:bg-white/5'
          ]"
        >
          <component
            v-if="item.icon"
            :is="item.icon"
            class="mr-3 h-5 w-5 flex-shrink-0 transition-colors"
            :class="[
               currentRoute.startsWith(item.href) ? 'text-blue-400' : 'text-gray-500 group-hover:text-white'
            ]"
          />
          {{ item.name }}
        </router-link>

        <!-- Expandable Item -->
        <div v-else-if="item.children">
          <button
            @click="toggleMenu(item.name)"
            class="group flex w-full items-center justify-between rounded-lg px-3 py-2.5 text-sm font-medium text-gray-400 hover:text-white hover:bg-white/5 transition-all duration-200"
          >
            <div class="flex items-center">
              <component
                v-if="item.icon"
                :is="item.icon"
                class="mr-3 h-5 w-5 flex-shrink-0 text-gray-500 group-hover:text-white transition-colors"
              />
              {{ item.name }}
            </div>
            <component
              :is="expandedMenus[item.name] ? ChevronDown : ChevronRight"
              class="h-4 w-4 text-gray-600 group-hover:text-gray-400 transition-transform duration-200"
            />
          </button>

          <div
            v-show="expandedMenus[item.name]"
            class="mt-1 space-y-1 pl-11 relative"
          >
             <!-- Vertical line for hierarchy -->
            <div class="absolute left-[22px] top-0 bottom-0 w-px bg-gray-800"></div>

            <router-link
              v-for="child in item.children"
              :key="child.name"
              :to="child.href || '#'"
              class="block rounded-md py-2 px-2 text-sm font-medium transition-colors hover:text-white relative z-10"
              :class="[
                child.href && currentRoute === child.href
                  ? 'text-white'
                  : 'text-gray-500'
              ]"
            >
              {{ child.name }}
            </router-link>
          </div>
        </div>
      </template>
    </nav>

    <div class="p-4 border-t border-gray-800 text-xs text-gray-600">
      <slot name="footer">
         <p>&copy; 2026 CryptoPay</p>
      </slot>
    </div>
  </div>
</template>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 4px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: #374151;
  border-radius: 2px;
}
</style>
