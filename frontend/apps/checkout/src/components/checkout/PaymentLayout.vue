<template>
  <!-- Embed mode: minimal container without header -->
  <main v-if="embedMode" class="embed-container bg-white dark:bg-slate-800 text-gray-800 dark:text-gray-200 font-sans">
    <!-- Loading State -->
    <div v-if="loading" class="flex items-center justify-center h-full">
      <div class="animate-spin rounded-full h-10 w-10 border-b-2 border-blue-600"></div>
    </div>

    <!-- Content -->
    <div v-else ref="embedContent" class="w-full flex flex-col items-center justify-center locale-fade" :class="{ 'locale-switching': isSwitching }">
      <slot></slot>
    </div>
  </main>

  <!-- Normal mode: full page layout with header -->
  <main v-else class="min-h-screen flex flex-col bg-[#F7F7F8] dark:bg-slate-900 text-gray-800 dark:text-gray-200 font-sans relative">
    <!-- Floating Language Toggle -->
    <button
       @click="toggleLocale"
       :aria-label="locale === 'zh-CN' ? 'Switch to English' : '切换到中文'"
       class="absolute top-4 right-6 z-20 inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-600 shadow-sm text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white hover:border-gray-300 dark:hover:border-slate-500 transition-all text-xs font-medium cursor-pointer"
    >
       <Languages class="w-3.5 h-3.5" />
       {{ locale === 'zh-CN' ? 'EN' : '中文' }}
    </button>

    <!-- Main Content -->
    <div class="flex-1 flex items-center justify-center p-4 sm:p-8">
      <!-- Card Container -->
      <div class="w-full max-w-5xl flex flex-col md:flex-row gap-8 relative locale-fade" :class="{ 'locale-switching': isSwitching }">
          <!-- Loading State Overlay -->
          <div v-if="loading" class="absolute inset-0 z-50 flex items-center justify-center bg-white/80 dark:bg-slate-900/80 backdrop-blur-sm rounded-3xl">
            <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
          </div>

          <slot></slot>
      </div>
    </div>
  </main>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { Languages } from 'lucide-vue-next';

const { locale } = useI18n();
const isSwitching = ref(false);

function toggleLocale() {
    isSwitching.value = true;
    setTimeout(() => {
        const next = locale.value === 'zh-CN' ? 'en-US' : 'zh-CN';
        locale.value = next;
        localStorage.setItem('app-locale', next);
        setTimeout(() => { isSwitching.value = false; }, 30);
    }, 150);
}

defineProps<{
  loading?: boolean;
  embedMode?: boolean;
}>();

// Expose the content element reference for ResizeObserver
const embedContent = ref<HTMLElement | null>(null);
defineExpose({ embedContent });
</script>

<style scoped>
.embed-container {
  /* Prevent double scrollbars in iFrame */
  overflow-y: auto;
  overflow-x: hidden;
  max-height: 100vh;
  padding: 16px;
  box-sizing: border-box;
}

.locale-fade {
  transition: opacity 0.15s ease, transform 0.15s ease;
}

.locale-switching {
  opacity: 0.3;
  transform: translateY(2px);
}
</style>
