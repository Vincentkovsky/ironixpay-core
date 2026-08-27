import { ref, computed } from 'vue';
import { useUserStore } from '@/stores';
import { getPayoutSettings } from '@/api/payout-settings';

/**
 * Cached approver roles from payout settings.
 * Fetched once per app session, shared across all components.
 */
const approverRoles = ref<string[]>(['owner', 'admin']); // sensible default
let fetched = false;

async function loadApproverRoles() {
  if (fetched) return;
  try {
    const data = await getPayoutSettings();
    approverRoles.value = data.approverRoles ?? ['owner', 'admin'];
    fetched = true;
  } catch {
    // keep defaults on error
  }
}

/**
 * Composable that returns a reactive `canApprove` computed based on
 * the current user's org role and the dynamic approverRoles from settings.
 *
 * Lazily fetches payout settings on first use, then caches.
 */
export function useCanApprove() {
  const userStore = useUserStore();

  // Trigger lazy load
  loadApproverRoles();

  const canApprove = computed(() =>
    approverRoles.value.includes(userStore.orgRole || ''),
  );

  return { canApprove, approverRoles };
}
