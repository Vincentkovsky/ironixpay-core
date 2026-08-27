<template>
  <button
    type="button"
    :title="title || value"
    class="copy-btn"
    :class="{ 'copy-btn--copied': copied }"
    @click.stop="doCopy"
  >
    <Transition name="copy-icon" mode="out-in">
      <Check v-if="copied" class="copy-btn__icon copy-btn__icon--check" />
      <Copy v-else class="copy-btn__icon" />
    </Transition>
  </button>
</template>

<script lang="ts" setup>
import { ref } from 'vue';
import { useClipboard } from '@vueuse/core';
import { Copy, Check } from 'lucide-vue-next';
import { toast } from 'vue-sonner';
import { useI18n } from 'vue-i18n';

const props = defineProps<{
  value?: string;
  title?: string;
  showToast?: boolean;
}>();

const { t } = useI18n();
const { copy } = useClipboard();
const copied = ref(false);
let timer: ReturnType<typeof setTimeout>;

const doCopy = () => {
  if (!props.value) return;
  copy(props.value);
  copied.value = true;
  if (props.showToast) {
    toast.success(t('sessionDetail.copied'));
  }
  clearTimeout(timer);
  timer = setTimeout(() => { copied.value = false; }, 2000);
};
</script>

<style scoped>
.copy-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 2px;
  border-radius: 4px;
  color: var(--muted-foreground, #6b7280);
  cursor: pointer;
  transition: color 0.15s, background-color 0.15s, transform 0.1s;
}

.copy-btn:hover {
  color: var(--foreground, #111827);
  background-color: hsl(var(--muted) / 0.5);
}

.copy-btn:active {
  transform: scale(0.9);
}

.copy-btn--copied {
  color: #22c55e;
}

.copy-btn--copied:hover {
  color: #22c55e;
}

.copy-btn__icon {
  width: 14px;
  height: 14px;
}

.copy-btn__icon--check {
  color: #22c55e;
}

/* Transition */
.copy-icon-enter-active,
.copy-icon-leave-active {
  transition: opacity 0.12s ease, transform 0.12s ease;
}

.copy-icon-enter-from {
  opacity: 0;
  transform: scale(0.6);
}

.copy-icon-leave-to {
  opacity: 0;
  transform: scale(0.6);
}
</style>
