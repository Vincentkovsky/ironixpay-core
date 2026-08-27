<script setup lang="ts">
import { useRegisterSW } from 'virtual:pwa-register/vue'
import { RefreshCw, X } from 'lucide-vue-next'

const {
  offlineReady,
  needRefresh,
  updateServiceWorker,
} = useRegisterSW()

function close() {
  offlineReady.value = false
  needRefresh.value = false
}
</script>

<template>
  <Transition name="slide-up">
    <div
      v-if="offlineReady || needRefresh"
      id="pwa-reload-prompt"
      class="fixed bottom-6 right-6 z-[9999] flex items-center gap-3 rounded-xl border border-border/50 bg-card px-5 py-3.5 shadow-xl backdrop-blur-sm"
      role="alert"
      aria-live="assertive"
    >
      <div class="flex-1 text-sm text-foreground">
        <span v-if="needRefresh">New version available</span>
        <span v-else>App ready to work offline</span>
      </div>

      <button
        v-if="needRefresh"
        id="pwa-reload-button"
        class="inline-flex items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
        @click="updateServiceWorker(true)"
      >
        <RefreshCw class="h-3.5 w-3.5" />
        Reload
      </button>

      <button
        id="pwa-dismiss-button"
        class="rounded-md p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        aria-label="Dismiss"
        @click="close"
      >
        <X class="h-4 w-4" />
      </button>
    </div>
  </Transition>
</template>

<style scoped>
.slide-up-enter-active,
.slide-up-leave-active {
  transition: all 0.3s ease;
}
.slide-up-enter-from,
.slide-up-leave-to {
  opacity: 0;
  transform: translateY(16px);
}
</style>
