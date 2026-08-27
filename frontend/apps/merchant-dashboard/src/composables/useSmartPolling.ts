import { ref, watch, onUnmounted } from 'vue';
import { useDocumentVisibility, useIntervalFn } from '@vueuse/core';

/**
 * Smart polling composable with visibility awareness.
 *
 * - Polls at `intervalMs` while the page is visible
 * - Pauses when the tab is hidden
 * - Immediately fires on tab re-focus
 * - Skips ticks if the previous fetch is still running (concurrent guard)
 * - Auto-cleans up on unmount
 *
 * @param fetchFn  Async function to call on each tick
 * @param intervalMs  Polling interval in milliseconds (default: 15000)
 */
export function useSmartPolling(fetchFn: () => Promise<void> | void, intervalMs = 15_000) {
  const isRunning = ref(false);
  const visibility = useDocumentVisibility();

  const guardedFetch = async () => {
    if (isRunning.value) return; // skip if previous tick still in-flight
    isRunning.value = true;
    try {
      await fetchFn();
    } catch {
      // errors handled by fetchFn itself
    } finally {
      isRunning.value = false;
    }
  };

  const { pause, resume } = useIntervalFn(guardedFetch, intervalMs, { immediate: false });

  // Visibility-aware: pause when hidden, resume + immediate fetch when visible
  watch(visibility, (current, previous) => {
    if (current === 'visible' && previous === 'hidden') {
      guardedFetch(); // immediate refresh on tab focus
      resume();
    } else if (current === 'hidden') {
      pause();
    }
  });

  // Start polling if page is currently visible, and fetch immediately
  if (visibility.value === 'visible') {
    guardedFetch(); // load data immediately on mount
    resume();
  }

  onUnmounted(() => {
    pause();
  });

  return { pause, resume, isRunning };
}
