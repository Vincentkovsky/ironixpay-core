<template>
  <div class="px-6 py-4 bg-white dark:bg-slate-800 border-t border-gray-200 dark:border-slate-700 flex items-center justify-between text-xs text-gray-400 dark:text-gray-500">
    <div class="flex items-center gap-2">
        <div class="w-4 h-4 bg-[#2563eb] rounded flex items-center justify-center">
            <span class="text-[7px] font-bold text-white leading-none">IX</span>
        </div>
        <span>{{ t('footer.poweredBy') }}</span>
        <span class="font-bold text-blue-600 dark:text-blue-400">IronixPay</span>
    </div>
    <div class="flex items-center gap-3">
        <button
            @click="toggleLocale"
            :aria-label="locale === 'zh-CN' ? 'Switch to English' : '切换到中文'"
            class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 text-gray-500 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 hover:border-gray-300 dark:hover:border-slate-600 transition-all text-xs font-medium cursor-pointer"
        >
            <Languages class="w-3.5 h-3.5" />
            {{ locale === 'zh-CN' ? 'English' : '中文' }}
        </button>
        <span class="text-gray-200 dark:text-slate-700">|</span>
        <a :href="locale === 'zh-CN' ? 'https://ironixpay.com/terms' : 'https://ironixpay.com/en/terms'" target="_blank" rel="noopener" class="hover:text-gray-600 dark:hover:text-gray-400 transition-colors">{{ t('footer.terms') }}</a>
        <a :href="locale === 'zh-CN' ? 'https://ironixpay.com/privacy' : 'https://ironixpay.com/en/privacy'" target="_blank" rel="noopener" class="hover:text-gray-600 dark:hover:text-gray-400 transition-colors">{{ t('footer.privacy') }}</a>
    </div>
  </div>
</template>
<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import { Languages } from 'lucide-vue-next';
const { t, locale } = useI18n();

function toggleLocale() {
    const next = locale.value === 'zh-CN' ? 'en-US' : 'zh-CN';
    locale.value = next;
    localStorage.setItem('app-locale', next);
}
</script>
