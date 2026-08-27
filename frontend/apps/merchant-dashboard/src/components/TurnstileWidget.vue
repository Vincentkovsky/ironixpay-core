<template>
  <div id="turnstile-register" ref="container" class="min-h-[65px]" />
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue';

type TurnstileWidgetId = string;

interface TurnstileApi {
  render: (
    container: HTMLElement,
    options: {
      sitekey: string;
      action: string;
      theme: 'auto';
      callback: (token: string) => void;
      'expired-callback': () => void;
      'error-callback': () => void;
    },
  ) => TurnstileWidgetId;
  reset: (widgetId: TurnstileWidgetId) => void;
  remove: (widgetId: TurnstileWidgetId) => void;
}

declare global {
  interface Window {
    turnstile?: TurnstileApi;
  }
}

const props = withDefaults(
  defineProps<{
    siteKey: string;
    action?: string;
  }>(),
  { action: 'register' },
);

const emit = defineEmits<{
  verified: [token: string];
  expired: [];
  error: [];
}>();

const container = ref<HTMLElement | null>(null);
let widgetId: TurnstileWidgetId | null = null;

const loadTurnstile = (): Promise<TurnstileApi> => {
  if (window.turnstile) return Promise.resolve(window.turnstile);

  return new Promise((resolve, reject) => {
    const scriptId = 'cloudflare-turnstile-script';
    const existing = document.getElementById(scriptId) as HTMLScriptElement | null;
    const script = existing ?? document.createElement('script');

    const handleLoad = () => {
      if (window.turnstile) resolve(window.turnstile);
      else reject(new Error('Turnstile API unavailable after script load'));
    };
    const handleError = () => reject(new Error('Failed to load Turnstile API'));

    script.addEventListener('load', handleLoad, { once: true });
    script.addEventListener('error', handleError, { once: true });

    if (!existing) {
      script.id = scriptId;
      script.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit';
      script.async = true;
      script.defer = true;
      document.head.appendChild(script);
    }
  });
};

const render = async () => {
  await nextTick();
  if (!container.value || widgetId) return;

  try {
    const turnstile = await loadTurnstile();
    widgetId = turnstile.render(container.value, {
      sitekey: props.siteKey,
      action: props.action,
      theme: 'auto',
      callback: (token) => emit('verified', token),
      'expired-callback': () => emit('expired'),
      'error-callback': () => emit('error'),
    });
  } catch {
    emit('error');
  }
};

const reset = () => {
  if (widgetId && window.turnstile) {
    window.turnstile.reset(widgetId);
  }
};

defineExpose({ reset });

onMounted(render);
onBeforeUnmount(() => {
  if (widgetId && window.turnstile) {
    window.turnstile.remove(widgetId);
  }
  widgetId = null;
});
</script>
