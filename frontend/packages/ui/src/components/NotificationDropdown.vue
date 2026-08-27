<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { Bell } from 'lucide-vue-next'

const isOpen = ref(false)
const dropdownRef = ref<HTMLElement | null>(null)

const notifications = [
  {
    id: 1,
    title: 'Low Gas Balance Alert',
    badge: 'Critical',
    badgeClass: 'bg-red-600 text-white',
    description: 'Your gas credit balance is running low. Refill to avoid sweep delays.',
    time: '2 hours ago'
  },
  {
    id: 2,
    title: 'Webhook Failure',
    badge: 'Warning',
    badgeClass: 'border border-yellow-500 text-yellow-500',
    description: '3 webhook deliveries failed for endpoint /api/payments',
    time: '5 hours ago'
  }
]

const toggleDropdown = () => {
  isOpen.value = !isOpen.value
}

const handleClickOutside = (event: MouseEvent) => {
  if (dropdownRef.value && !dropdownRef.value.contains(event.target as Node)) {
    isOpen.value = false
  }
}

onMounted(() => {
  document.addEventListener('click', handleClickOutside)
})

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside)
})
</script>

<template>
  <div class="relative" ref="dropdownRef">
    <button
      @click.stop="toggleDropdown"
      class="relative p-2 text-gray-400 hover:text-white transition-colors outline-none focus:text-white"
    >
      <Bell class="h-5 w-5" />
      <span class="absolute top-1.5 right-1.5 h-2 w-2 rounded-full bg-red-500"></span>
    </button>

    <div
      v-if="isOpen"
      class="absolute right-0 mt-2 w-96 origin-top-right rounded-xl border border-gray-700 bg-[#0F172A] shadow-2xl z-50 overflow-hidden"
    >
      <div class="px-4 py-3 border-b border-gray-700 bg-[#0F172A]">
        <h3 class="text-sm font-semibold text-white">Notifications</h3>
      </div>

      <div class="max-h-[400px] overflow-y-auto">
        <div
          v-for="notification in notifications"
          :key="notification.id"
          class="p-4 border-b border-gray-800 last:border-0 hover:bg-gray-800/50 transition-colors cursor-pointer"
        >
          <div class="flex justify-between items-start mb-1">
            <span class="font-medium text-white text-sm">{{ notification.title }}</span>
            <span
              :class="[
                notification.badgeClass,
                'text-[10px] px-2 py-0.5 rounded-full font-medium'
              ]"
            >
              {{ notification.badge }}
            </span>
          </div>
          <p class="text-xs text-gray-400 leading-relaxed mb-2">
            {{ notification.description }}
          </p>
          <span class="text-xs text-gray-500">{{ notification.time }}</span>
        </div>
      </div>
    </div>
  </div>
</template>
