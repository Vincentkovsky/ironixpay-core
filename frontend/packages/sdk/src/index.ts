/**
 * IronixPay Embedded Checkout SDK
 *
 * @packageDocumentation
 * @module @ironix-pay/sdk
 */

export { IronixPay } from './IronixPay';
export { CheckoutElement } from './CheckoutElement';

// Re-export types for consumers
export type {
    IronixPayOptions,
    CreateElementOptions,
    ElementStyle,
    PaymentResult,
    PaymentError,
    IronixPayEventType,
    IronixPayEventMap
} from './types';

// Re-export event constants for advanced usage
export { CheckoutEvents, SdkCommands, MESSAGE_SOURCE } from './events';

// Default export for CDN usage: new IronixPay.default({...})
import { IronixPay } from './IronixPay';
export default IronixPay;
