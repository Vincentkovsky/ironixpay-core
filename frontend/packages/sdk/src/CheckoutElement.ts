import type {
    InternalElementOptions,
    IronixPayEventType,
    IronixPayEventMap,
    PaymentResult,
    PaymentError
} from './types';
import { CheckoutEvents, SdkCommands, isIronixPayMessage } from './events';

/**
 * Default load timeout in milliseconds
 */
const LOAD_TIMEOUT_MS = 10000;



/**
 * CheckoutElement - Manages the embedded checkout iframe
 *
 * Handles:
 * - iFrame creation and lifecycle
 * - Secure postMessage communication
 * - Event dispatching to merchant callbacks
 * - Auto-resize based on content height
 */
export class CheckoutElement {
    private iframe: HTMLIFrameElement | null = null;
    private container: HTMLElement | null = null;
    private options: InternalElementOptions;
    private listeners: Map<IronixPayEventType, Set<Function>> = new Map();
    private messageHandler: ((event: MessageEvent) => void) | null = null;
    private loadTimeoutId: ReturnType<typeof setTimeout> | null = null;
    private isReady = false;

    constructor(options: InternalElementOptions) {
        this.options = {
            theme: 'dark',
            locale: 'en',
            style: {
                width: '100%',
                height: '480px',
                borderRadius: '12px'
            },
            ...options
        };
    }

    /**
     * Mount the payment element to a DOM container
     *
     * @param selector - CSS selector string or HTMLElement
     */
    mount(selector: string | HTMLElement): void {
        // Resolve container
        if (typeof selector === 'string') {
            const el = document.querySelector(selector);
            if (!el) {
                throw new Error(`IronixPay: Container not found: ${selector}`);
            }
            this.container = el as HTMLElement;
        } else {
            this.container = selector;
        }

        // Clean up any existing iframe
        this.unmount();

        // Set up message listener BEFORE creating iframe
        this.setupMessageListener();

        // Create iframe
        this.iframe = document.createElement('iframe');

        // Build checkout URL with embed mode
        const checkoutUrl = new URL(`/checkout/${this.options.sessionId}`, this.options.baseUrl);
        checkoutUrl.searchParams.set('embed', '1');
        if (this.options.theme) {
            checkoutUrl.searchParams.set('theme', this.options.theme);
        }
        if (this.options.locale) {
            checkoutUrl.searchParams.set('locale', this.options.locale);
        }

        this.iframe.src = checkoutUrl.toString();
        this.iframe.style.cssText = `
            width: ${this.options.style?.width || '100%'};
            height: ${this.options.style?.height || '480px'};
            border: none;
            border-radius: ${this.options.style?.borderRadius || '12px'};
            background: transparent;
        `;
        this.iframe.allow = 'clipboard-write';
        this.iframe.setAttribute('loading', 'eager');

        this.container.appendChild(this.iframe);

        // Set up load timeout
        this.loadTimeoutId = setTimeout(() => {
            if (!this.isReady) {
                this.emit('error', {
                    code: 'LOAD_TIMEOUT',
                    message: 'Failed to load payment element. Please check your network connection.'
                } as PaymentError);
            }
        }, LOAD_TIMEOUT_MS);
    }

    /**
     * Unmount and clean up the payment element
     */
    unmount(): void {
        // Clear timeout
        if (this.loadTimeoutId) {
            clearTimeout(this.loadTimeoutId);
            this.loadTimeoutId = null;
        }

        // Remove message listener
        if (this.messageHandler) {
            window.removeEventListener('message', this.messageHandler);
            this.messageHandler = null;
        }

        // Remove iframe
        if (this.iframe && this.container) {
            this.container.removeChild(this.iframe);
            this.iframe = null;
        }

        this.isReady = false;
    }

    /**
     * Register an event listener
     *
     * @param event - Event type to listen for
     * @param callback - Callback function
     */
    on<K extends IronixPayEventType>(
        event: K,
        callback: IronixPayEventMap[K]
    ): void {
        if (!this.listeners.has(event)) {
            this.listeners.set(event, new Set());
        }
        this.listeners.get(event)!.add(callback);
    }

    /**
     * Remove an event listener
     *
     * @param event - Event type
     * @param callback - Callback function to remove
     */
    off<K extends IronixPayEventType>(
        event: K,
        callback: IronixPayEventMap[K]
    ): void {
        const callbacks = this.listeners.get(event);
        if (callbacks) {
            callbacks.delete(callback);
        }
    }

    /**
     * Set up the postMessage listener with strict origin validation
     */
    private setupMessageListener(): void {
        this.messageHandler = (event: MessageEvent) => {
            // SECURITY: Strict origin validation
            if (event.origin !== this.options.allowedOrigin) {
                return;
            }

            // Validate message structure
            if (!isIronixPayMessage(event.data)) {
                return;
            }

            const { type, payload } = event.data;

            switch (type) {
                case CheckoutEvents.READY:
                    this.handleReady();
                    break;

                case CheckoutEvents.RESIZE:
                    this.handleResize(payload as { height: number });
                    break;

                case CheckoutEvents.PAYMENT_SUCCESS:
                    this.emit('payment_success', payload as PaymentResult);
                    break;

                case CheckoutEvents.PAYMENT_EXPIRED:
                    this.emit('payment_expired', payload as { sessionId: string });
                    break;

                case CheckoutEvents.PAYMENT_DETECTED:
                    this.emit('payment_detected', payload as { sessionId: string; amountReceived: number });
                    break;

                case CheckoutEvents.ERROR:
                    this.emit('error', payload as PaymentError);
                    break;
            }
        };

        window.addEventListener('message', this.messageHandler);
    }

    /**
     * Handle the READY event from checkout iframe
     */
    private handleReady(): void {
        if (this.loadTimeoutId) {
            clearTimeout(this.loadTimeoutId);
            this.loadTimeoutId = null;
        }

        this.isReady = true;

        // Send configuration to iframe
        this.sendToIframe(SdkCommands.INIT, {
            theme: this.options.theme,
            locale: this.options.locale
        });

        this.emit('ready');
    }

    /**
     * Handle resize events from the iframe.
     * The checkout app already adds buffer to the reported height,
     * so we use it directly without adding additional buffer.
     */
    private handleResize(data: { height: number }): void {
        if (this.iframe && data.height > 0) {
            // Use height directly - checkout app already includes buffer
            this.iframe.style.height = `${data.height}px`;
        }
        this.emit('resize', data);
    }

    /**
     * Send a message to the checkout iframe
     */
    private sendToIframe(type: string, payload: unknown): void {
        if (this.iframe?.contentWindow) {
            this.iframe.contentWindow.postMessage(
                { source: 'ironix-pay-sdk', type, payload },
                this.options.allowedOrigin
            );
        }
    }

    /**
     * Emit an event to registered listeners
     */
    private emit<K extends IronixPayEventType>(
        event: K,
        ...args: Parameters<IronixPayEventMap[K]>
    ): void {
        const callbacks = this.listeners.get(event);
        if (callbacks) {
            callbacks.forEach((cb) => {
                try {
                    (cb as (...args: unknown[]) => void)(...args);
                } catch (err) {
                    console.error(`IronixPay: Error in ${event} handler:`, err);
                }
            });
        }
    }
}
