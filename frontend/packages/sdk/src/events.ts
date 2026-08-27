/**
 * IronixPay SDK Event Constants
 *
 * Defines the message protocol between the SDK and the embedded checkout iframe.
 */

/**
 * Source identifier for all IronixPay messages
 * Used to distinguish from other postMessage traffic on merchant pages
 */
export const MESSAGE_SOURCE = 'ironix-pay-checkout';

/**
 * Message types sent FROM the checkout iframe TO the SDK
 */
export const CheckoutEvents = {
    /**
     * Sent when checkout page has fully initialized and is ready
     */
    READY: 'IRONIX_PAY_READY',

    /**
     * Sent when the iframe content height changes
     */
    RESIZE: 'IRONIX_PAY_RESIZE',

    /**
     * Sent when payment is successful
     */
    PAYMENT_SUCCESS: 'IRONIX_PAY_PAYMENT_SUCCESS',

    /**
     * Sent when payment session expires
     */
    PAYMENT_EXPIRED: 'IRONIX_PAY_PAYMENT_EXPIRED',

    /**
     * Sent when a payment transaction is detected (but not yet confirmed)
     */
    PAYMENT_DETECTED: 'IRONIX_PAY_PAYMENT_DETECTED',

    /**
     * Sent when an error occurs in the checkout
     */
    ERROR: 'IRONIX_PAY_ERROR'
} as const;

/**
 * Message types sent FROM the SDK TO the checkout iframe
 */
export const SdkCommands = {
    /**
     * Initialization command with configuration
     */
    INIT: 'IRONIX_PAY_INIT'
} as const;

/**
 * Type guard to check if a message is from IronixPay checkout
 */
export function isIronixPayMessage(data: unknown): data is { source: string; type: string; payload: unknown } {
    return (
        typeof data === 'object' &&
        data !== null &&
        'source' in data &&
        (data as { source: unknown }).source === MESSAGE_SOURCE
    );
}
