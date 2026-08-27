import { defineStore } from 'pinia';
import { ref, computed, watch, nextTick } from 'vue';
import { differenceInSeconds } from 'date-fns';
import { ApiClient } from '@ironix-pay/api-client';
import { pollChainForTransfer, createPollerConfig, type PollerConfig } from '../utils/chainPoller';

// Helper to get typed client
// In a real app, this might come from a plugin or global provider
const apiClient = new ApiClient({
    baseURL: import.meta.env.VITE_API_BASE_URL || 'http://localhost:3000',
    getToken: () => null, // Public checkout does not need a token
    setToken: () => { },   // No-op
    onAuthError: () => console.warn('Auth error in public checkout')
});

// Redirect delay in seconds (allows user to see the success/expired state)
const REDIRECT_DELAY_SECONDS = 3;

// SSE retry configuration
const SSE_RETRY_DELAY_MS = 3000;
const SSE_MAX_RETRIES = 3;

// ========================================================
// Embed Mode (SDK Integration) Constants
// ========================================================
const MESSAGE_SOURCE = 'ironix-pay-checkout';

const CheckoutEvents = {
    READY: 'IRONIX_PAY_READY',
    RESIZE: 'IRONIX_PAY_RESIZE',
    PAYMENT_SUCCESS: 'IRONIX_PAY_PAYMENT_SUCCESS',
    PAYMENT_EXPIRED: 'IRONIX_PAY_PAYMENT_EXPIRED',
    PAYMENT_DETECTED: 'IRONIX_PAY_PAYMENT_DETECTED',
    ERROR: 'IRONIX_PAY_ERROR'
} as const;

export const useCheckoutStore = defineStore('checkout', () => {
    const session = ref<any | null>(null); // Use 'any' temporarily or import proper type from api-client
    const loading = ref(false);
    const error = ref<string | null>(null);
    const timeLeft = ref(0);
    const pollingInterval = ref<any>(null);
    const redirectCountdown = ref(0);
    const redirectTimerInterval = ref<any>(null);
    const timeLeftInterval = ref<any>(null); // Track the 1-second timer for proper cleanup

    // SSE state
    const eventSource = ref<EventSource | null>(null);
    const sseRetryCount = ref(0);
    const useSSE = ref(true); // Whether to attempt SSE (can be disabled on repeated failures)

    // Embed mode state
    const isEmbedMode = ref(false);
    const sdkInitialized = ref(false);
    const isDarkTheme = ref(false);

    // Multi-transaction tracking: tx_hashes that are detected but not yet confirmed
    const pendingTxHashes = ref<string[]>([]);

    // Frontend-assisted payment detection
    const notifiedTxHashes = ref<Set<string>>(new Set());
    let detectionTimeoutId: ReturnType<typeof setTimeout> | null = null;
    let pollerConfig: PollerConfig | null = null;

    const isExpired = computed(() => {
        if (!session.value) return false;
        return session.value.status === 'Expired' || timeLeft.value <= 0;
    });

    const isPaid = computed(() => {
        return session.value?.status === 'Paid' || session.value?.status === 'Overpaid';
    });

    // Check if any payment is currently being confirmed on blockchain
    const isConfirming = computed(() => {
        return pendingTxHashes.value.length > 0;
    });

    // Check if success redirect URL is configured
    const hasSuccessUrl = computed(() => {
        return !!session.value?.success_url;
    });

    // Check if cancel redirect URL is configured
    const hasCancelUrl = computed(() => {
        return !!session.value?.cancel_url;
    });

    // Get the appropriate redirect URL based on status
    const redirectUrl = computed(() => {
        if (isPaid.value && session.value?.success_url) {
            return session.value.success_url;
        }
        if (isExpired.value && session.value?.cancel_url) {
            return session.value.cancel_url;
        }
        return null;
    });

    // ========================================================
    // Theme Management
    // ========================================================

    /**
     * Apply or remove the dark theme class on <html>
     */
    function applyTheme(theme: string): void {
        const isDark = theme === 'dark' ||
            (theme === 'auto' && window.matchMedia('(prefers-color-scheme: dark)').matches);
        isDarkTheme.value = isDark;
        document.documentElement.classList.toggle('dark', isDark);
        console.log(`[Theme] Applied: ${isDark ? 'dark' : 'light'}`);
    }

    // ========================================================
    // Embed Mode (postMessage) Functions
    // ========================================================

    /**
     * Initialize embed mode based on URL query parameter
     */
    function initEmbedMode(): void {
        const urlParams = new URLSearchParams(window.location.search);
        isEmbedMode.value = urlParams.get('embed') === '1';

        // Apply theme from URL param (works for both embed and normal mode)
        const themeParam = urlParams.get('theme');
        if (themeParam) {
            applyTheme(themeParam);
        }

        if (isEmbedMode.value) {
            console.log('[Embed] Mode enabled');
            // Prevent body scroll in embed mode
            document.body.style.overflow = 'hidden';
            // Listen for SDK init command
            window.addEventListener('message', handleSdkMessage);
        }
    }

    /**
     * Handle incoming messages from SDK
     */
    function handleSdkMessage(event: MessageEvent): void {
        // We accept messages from any origin in embed mode since the SDK controls the iframe
        const data = event.data;
        if (!data || typeof data !== 'object') return;

        if (data.source === 'ironix-pay-sdk' && data.type === 'IRONIX_PAY_INIT') {
            console.log('[Embed] Received INIT from SDK:', data.payload);
            sdkInitialized.value = true;
            // Apply theme from SDK init payload
            if (data.payload?.theme) {
                applyTheme(data.payload.theme);
            }
        }
    }

    /**
     * Send postMessage to parent window (SDK)
     */
    function sendToSdk(type: string, payload: unknown): void {
        if (!isEmbedMode.value) return;

        if (window.parent && window.parent !== window) {
            window.parent.postMessage({
                source: MESSAGE_SOURCE,
                type,
                payload
            }, '*'); // SDK validates origin on its side
            console.log('[Embed] Sent to SDK:', type, payload);
        }
    }

    /**
     * Send READY event to SDK when checkout is initialized
     */
    function notifySdkReady(): void {
        sendToSdk(CheckoutEvents.READY, {
            sessionId: session.value?.id
        });
    }

    /**
     * Send resize event to SDK for auto-height adjustment
     */
    function notifySdkResize(height: number): void {
        sendToSdk(CheckoutEvents.RESIZE, { height });
    }

    /**
     * Notify SDK of payment success
     */
    function notifySdkPaymentSuccess(): void {
        sendToSdk(CheckoutEvents.PAYMENT_SUCCESS, {
            sessionId: session.value?.id,
            status: session.value?.status,
            amountReceived: session.value?.amount_received,
            transactionHash: session.value?.transaction_hash
        });
    }

    /**
     * Notify SDK of payment expiry
     */
    function notifySdkPaymentExpired(): void {
        sendToSdk(CheckoutEvents.PAYMENT_EXPIRED, {
            sessionId: session.value?.id
        });
    }

    /**
     * Notify SDK of payment detection (pending confirmation)
     */
    function notifySdkPaymentDetected(): void {
        sendToSdk(CheckoutEvents.PAYMENT_DETECTED, {
            sessionId: session.value?.id,
            amountReceived: session.value?.amount_received
        });
    }

    // Fetch session details
    async function fetchSession(sessionId: string) {
        loading.value = true;
        error.value = null;
        try {
            const res = await apiClient.getCheckoutSessionPublic(sessionId);
            session.value = res;
            updateTimeLeft();

            // Notify SDK that checkout is ready (after Vue renders)
            if (isEmbedMode.value) {
                nextTick(() => {
                    notifySdkReady();
                    // Send initial height
                    notifySdkResize(document.body.scrollHeight);
                });
            }
        } catch (err: any) {
            console.error('Failed to fetch session:', err);
            error.value = err.message || 'Failed to load session details';

            // Notify SDK of error
            if (isEmbedMode.value) {
                sendToSdk(CheckoutEvents.ERROR, {
                    code: 'SESSION_NOT_FOUND',
                    message: error.value
                });
            }
        } finally {
            loading.value = false;
        }
    }

    // Update countdown timer
    function updateTimeLeft() {
        if (!session.value) return;
        const now = new Date();
        // Backend returns Postgres-style: "2026-02-12 18:30:00 +00:00"
        // Safari needs ISO 8601: "2026-02-12T18:30:00+00:00"
        const raw = session.value.expires_at || '';
        const isoStr = raw.replace(' ', 'T').replace(' +', '+').replace(' -', '-');
        const expiresAt = new Date(isoStr);
        if (isNaN(expiresAt.getTime())) {
            timeLeft.value = 0;
            return;
        }
        const diff = differenceInSeconds(expiresAt, now);
        timeLeft.value = Math.max(0, diff);
    }

    // Start redirect countdown and perform redirect
    function startRedirectCountdown() {
        if (redirectTimerInterval.value) return; // Already started

        const targetUrl = redirectUrl.value;

        // In embed mode, don't redirect - SDK handles navigation
        if (isEmbedMode.value) {
            return;
        }

        if (!targetUrl) return;

        redirectCountdown.value = REDIRECT_DELAY_SECONDS;

        redirectTimerInterval.value = setInterval(() => {
            redirectCountdown.value--;
            if (redirectCountdown.value <= 0) {
                clearInterval(redirectTimerInterval.value);
                redirectTimerInterval.value = null;
                // Perform redirect
                window.location.href = targetUrl;
            }
        }, 1000);
    }

    // Watch for terminal states and trigger redirect or SDK notification
    watch([isPaid, isExpired], ([paid, expired]) => {
        if (paid || expired) {
            stopPolling();
            disconnectSSE();

            if (isEmbedMode.value) {
                // Notify SDK instead of redirecting
                if (paid) {
                    notifySdkPaymentSuccess();
                } else if (expired) {
                    notifySdkPaymentExpired();
                }
            } else {
                // Normal mode: start redirect countdown
                if (redirectUrl.value) {
                    startRedirectCountdown();
                }
            }
        }
    });

    // Watch for payment detection (Detected status)
    watch(() => session.value?.status, (newStatus, oldStatus) => {
        if (newStatus === 'Detected' && oldStatus !== 'Detected') {
            if (isEmbedMode.value) {
                notifySdkPaymentDetected();
            }
        }
    });

    // ========================================================
    // SSE (Server-Sent Events) - Real-time updates
    // ========================================================

    function connectSSE(sessionId: string) {
        if (!useSSE.value) {
            console.log('[SSE] Disabled, using polling fallback');
            startPolling(sessionId);
            return;
        }

        if (eventSource.value) {
            disconnectSSE();
        }

        const baseUrl = import.meta.env.VITE_API_BASE_URL || 'http://localhost:3000';
        const sseUrl = `${baseUrl}/v1/checkout/sessions/${sessionId}/events`;

        console.log('[SSE] Connecting to:', sseUrl);

        try {
            eventSource.value = new EventSource(sseUrl);

            eventSource.value.onopen = () => {
                console.log('[SSE] Connection opened');
                sseRetryCount.value = 0; // Reset retry count on successful connection
            };

            // Listen for session_updated events (all SSE events come through this channel)
            eventSource.value.addEventListener('session_updated', (event: MessageEvent) => {
                try {
                    const data = JSON.parse(event.data);
                    console.log('[SSE] Event received:', data);

                    if (!session.value) return;

                    // Handle based on event type
                    if (data.type === 'payment_detected') {
                        // PaymentDetected: add to pending list for "Confirming..." UI
                        console.log('[SSE] Payment detected - adding to pending:', data.tx_hash);
                        if (data.tx_hash && !pendingTxHashes.value.includes(data.tx_hash)) {
                            pendingTxHashes.value.push(data.tx_hash);
                        }
                    } else if (data.type === 'session_updated' && data.status) {
                        // SessionUpdated: payment confirmed, clear pending list and update status
                        console.log('[SSE] Session updated - payment confirmed');
                        pendingTxHashes.value = []; // Clear pending after confirmation
                        session.value.status = data.status;
                        if (typeof data.amount_received === 'string') {
                            session.value.amount_received = data.amount_received;
                        }
                        // Sync expires_at (critical for Underpaid → 24h rolling extension)
                        if (data.expires_at) {
                            session.value.expires_at = data.expires_at;
                            updateTimeLeft();
                        }
                    }
                } catch (e) {
                    console.warn('[SSE] Failed to parse event:', e);
                }
            });

            eventSource.value.onerror = (err) => {
                console.error('[SSE] Connection error:', err);
                disconnectSSE();

                sseRetryCount.value++;
                if (sseRetryCount.value <= SSE_MAX_RETRIES) {
                    console.log(`[SSE] Retrying (${sseRetryCount.value}/${SSE_MAX_RETRIES}) in ${SSE_RETRY_DELAY_MS}ms...`);
                    setTimeout(() => connectSSE(sessionId), SSE_RETRY_DELAY_MS);
                } else {
                    console.log('[SSE] Max retries reached, falling back to polling');
                    useSSE.value = false;
                    startPolling(sessionId);
                }
            };
        } catch (e) {
            console.error('[SSE] Failed to create EventSource:', e);
            useSSE.value = false;
            startPolling(sessionId);
        }
    }

    function disconnectSSE() {
        if (eventSource.value) {
            eventSource.value.close();
            eventSource.value = null;
            console.log('[SSE] Disconnected');
        }
    }

    // Start polling for status updates (fallback when SSE fails)
    function startPolling(sessionId: string) {
        if (pollingInterval.value) clearInterval(pollingInterval.value);

        console.log('[Polling] Starting fallback polling');

        // Poll every 3 seconds
        pollingInterval.value = setInterval(async () => {
            if (isPaid.value || isExpired.value) {
                stopPolling();
                return;
            }
            try {
                // Background fetch (no loading state)
                const res = await apiClient.getCheckoutSessionPublic(sessionId);
                session.value = res;
                updateTimeLeft();
            } catch (err) {
                console.warn('Polling failed temporarily', err);
            }
        }, 3000);
        // Note: timeLeftInterval is managed by startRealTimeUpdates, not here
    }

    function stopPolling() {
        if (pollingInterval.value) {
            clearInterval(pollingInterval.value);
            pollingInterval.value = null;
        }
    }

    // Start real-time updates (SSE with polling fallback)
    function startRealTimeUpdates(sessionId: string) {
        // Clear any existing timer first to prevent duplicates
        if (timeLeftInterval.value) {
            clearInterval(timeLeftInterval.value);
        }

        // Always update timer every second for UI smoothness
        timeLeftInterval.value = setInterval(() => {
            updateTimeLeft();
        }, 1000);

        // Try SSE first, fallback to polling
        connectSSE(sessionId);

        // Payment detection is started reactively via watch(session) below,
        // because session.value is null here (fetchSession hasn't completed yet).
    }

    // Start payment detection when session data first becomes available
    watch(session, (newSession, oldSession) => {
        if (newSession?.id && !oldSession && !isPaid.value && !isExpired.value) {
            startPaymentDetection(newSession.id);
        }
    });

    // Frontend payment detection: poll chain directly for USDT transfers
    function startPaymentDetection(sessionId: string) {
        const s = session.value;
        if (!s?.pay_address) return;

        pollerConfig = createPollerConfig(s);
        if (!pollerConfig) {
            console.log('[Detection] No detection config (missing chain fields), skipping');
            return;
        }

        console.log('[Detection] Started for', pollerConfig.chainFamily, 'chain');
        const apiBaseUrl = import.meta.env.VITE_API_BASE_URL || 'http://localhost:3000';

        async function poll() {
            // Only stop detection on terminal states.
            // Keep running during isConfirming to support underpaid → second payment.
            if (isPaid.value || isExpired.value) {
                stopPaymentDetection();
                return;
            }
            try {
                const txHash = await pollChainForTransfer(pollerConfig!);
                if (txHash && !notifiedTxHashes.value.has(txHash)) {
                    notifiedTxHashes.value.add(txHash);
                    // Immediately show "Confirming on blockchain" UI
                    // (don't wait for the slower SSE payment_detected from backend indexer)
                    if (!pendingTxHashes.value.includes(txHash)) {
                        pendingTxHashes.value.push(txHash);
                    }
                    console.log('[Detection] Found tx, notifying backend:', txHash);
                    fetch(`${apiBaseUrl}/v1/checkout/sessions/${sessionId}/notify-payment`, {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ tx_hash: txHash }),
                    }).catch(() => { /* fire-and-forget */ });
                }
            } catch (err) {
                // Silently continue — detection is best-effort
            }
            detectionTimeoutId = setTimeout(poll, 5000);
        }
        // Start first poll after 1 second (let session load settle)
        detectionTimeoutId = setTimeout(poll, 1000);
    }

    function stopPaymentDetection() {
        if (detectionTimeoutId) {
            clearTimeout(detectionTimeoutId);
            detectionTimeoutId = null;
        }
    }

    /**
     * Cleanup function for embed mode and timers
     */
    function cleanup(): void {
        // Remove SDK message listener
        if (isEmbedMode.value) {
            window.removeEventListener('message', handleSdkMessage);
            // Restore body overflow that was set to 'hidden' in initEmbedMode
            document.body.style.overflow = '';
        }

        // Clear the 1-second timeLeft interval
        if (timeLeftInterval.value) {
            clearInterval(timeLeftInterval.value);
            timeLeftInterval.value = null;
        }

        // Clear redirect countdown timer
        if (redirectTimerInterval.value) {
            clearInterval(redirectTimerInterval.value);
            redirectTimerInterval.value = null;
        }

        // Stop frontend payment detection
        stopPaymentDetection();
    }

    return {
        session,
        loading,
        error,
        timeLeft,
        isExpired,
        isPaid,
        hasSuccessUrl,
        hasCancelUrl,
        redirectUrl,
        redirectCountdown,
        isEmbedMode,
        sdkInitialized,
        isDarkTheme,
        pendingTxHashes,
        isConfirming,
        fetchSession,
        startPolling,
        stopPolling,
        startRealTimeUpdates,
        disconnectSSE,
        initEmbedMode,
        notifySdkResize,
        cleanup
    };
});
