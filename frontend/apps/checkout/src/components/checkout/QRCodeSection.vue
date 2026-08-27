<template>
  <div class="p-8 flex flex-col items-center justify-center relative">

    <!-- Amount Display -->
    <div class="text-center mb-8">
        <div class="text-sm text-gray-500 dark:text-gray-400 mb-1">Total Amount</div>
        <div class="text-3xl font-bold text-gray-900 dark:text-white tracking-tight flex items-center justify-center">
            {{ amount }} <span class="text-lg text-gray-400 dark:text-gray-500 ml-1.5">{{ currency }}</span>
        </div>
    </div>

    <!-- QR Container -->
    <div class="relative bg-white dark:bg-slate-800 p-4 rounded-xl shadow-lg border border-gray-100 dark:border-slate-700 mb-6 group">
      <qrcode-vue
        :value="address"
        :size="200"
        level="H"
        :background="qrBackground"
        :foreground="qrForeground"
      />


      <!-- Copy Overlay (Hover) -->
      <div v-if="!disableInteractions"
           @click="copyAddress"
           class="absolute inset-0 bg-gray-900/60 rounded-xl flex items-center justify-center transition-all cursor-pointer opacity-0 hover:opacity-100 backdrop-blur-[1px]">
        <div class="bg-white dark:bg-slate-700 text-gray-900 dark:text-white px-3 py-1.5 rounded-lg text-sm font-medium shadow-lg flex items-center gap-2">
           <CopyIcon class="w-4 h-4" /> Copy
        </div>
      </div>
    </div>

    <!-- Address Box -->
    <div class="w-full">
        <div class="flex items-center justify-between bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-lg p-3 group hover:border-blue-300 dark:hover:border-blue-600 transition-colors">
            <span class="font-mono text-sm text-gray-700 dark:text-gray-300 truncate mr-2 select-all">{{ address }}</span>
            <button @click="copyAddress" aria-label="Copy address" class="p-2 hover:bg-gray-100 dark:hover:bg-slate-700 rounded-md transition-colors text-gray-400 dark:text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 cursor-pointer">
                <CopyIcon v-if="!copied" class="w-4 h-4" />
                <CheckIcon v-else class="w-4 h-4 text-green-500" />
            </button>
        </div>
        <p class="mt-3 text-xs text-center text-gray-500 dark:text-gray-400">
            Please only send <strong class="text-gray-600 dark:text-gray-300">{{ currency || 'USDT' }}</strong> via the <strong class="text-gray-600 dark:text-gray-300">{{ network }}</strong> network.
        </p>
    </div>

    <!-- Slot for Overlays (Success/Expired) -->
    <slot name="overlay"></slot>

  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import QrcodeVue from 'qrcode.vue';
import { Copy as CopyIcon, Check as CheckIcon } from 'lucide-vue-next';

const props = defineProps<{
  address: string;
  amount: number | string;
  currency?: string;
  network?: string;
  disableInteractions?: boolean;
  isDark?: boolean;
}>();


const copied = ref(false);

const qrBackground = computed(() => props.isDark ? '#1e293b' : '#ffffff');
const qrForeground = computed(() => props.isDark ? '#ffffff' : '#000000');

const copyAddress = async () => {
  if (props.disableInteractions) return;
  try {
    await navigator.clipboard.writeText(props.address);
    copied.value = true;
    setTimeout(() => copied.value = false, 2000);
  } catch (err) {
    console.error('Copy failed', err);
  }
};
</script>
