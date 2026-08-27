import { CheckoutElement } from './CheckoutElement';
import type { IronixPayOptions, CreateElementOptions } from './types';

/**
 * IronixPay SDK Main Class
 *
 * @example
 * ```typescript
 * const ironixPay = new IronixPay({
 *   environment: 'production'
 * });
 *
 * const element = ironixPay.createPaymentElement({
 *   sessionId: 'cs_xxx',
 *   theme: 'dark'
 * });
 *
 * element.mount('#checkout-container');
 * element.on('payment_success', (result) => console.log('Paid!', result));
 * ```
 */
export class IronixPay {
    private baseUrl: string;
    private allowedOrigin: string;

    /**
     * Create a new IronixPay SDK instance
     */
    constructor(options: IronixPayOptions = {}) {
        // Determine base URL from environment or custom URL
        if (options.checkoutUrl) {
            this.baseUrl = options.checkoutUrl.replace(/\/$/, '');
        } else {
            this.baseUrl = options.environment === 'sandbox'
                ? 'https://pay-sandbox.ironixpay.com'
                : 'https://pay.ironixpay.com';
        }

        // Extract origin for postMessage security validation
        try {
            const url = new URL(this.baseUrl);
            this.allowedOrigin = url.origin;
        } catch {
            // For development with relative URLs
            this.allowedOrigin = window.location.origin;
        }
    }

    /**
     * Create a payment element that can be mounted to the DOM
     *
     * @param options - Configuration for the payment element
     * @returns A CheckoutElement instance
     */
    createPaymentElement(options: CreateElementOptions): CheckoutElement {
        if (!options.sessionId) {
            throw new Error('IronixPay: sessionId is required');
        }

        return new CheckoutElement({
            ...options,
            baseUrl: this.baseUrl,
            allowedOrigin: this.allowedOrigin
        });
    }

    /**
     * Get the current SDK version
     */
    get version(): string {
        return '0.2.0';
    }
}
