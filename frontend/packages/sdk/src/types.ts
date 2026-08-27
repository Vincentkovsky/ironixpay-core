/**
 * IronixPay SDK Type Definitions
 */

/**
 * SDK initialization options
 */
export interface IronixPayOptions {
    /**
     * Environment to use
     * @default 'production'
     */
    environment?: 'production' | 'sandbox';

    /**
     * Custom checkout base URL (for self-hosted deployments)
     */
    checkoutUrl?: string;
}

/**
 * Options for creating a payment element
 */
export interface CreateElementOptions {
    /**
     * The checkout session ID obtained from your backend
     */
    sessionId: string;

    /**
     * UI theme
     * @default 'dark'
     */
    theme?: 'light' | 'dark' | 'auto';

    /**
     * Locale for the checkout UI
     * @default 'en'
     */
    locale?: 'en' | 'zh-CN';

    /**
     * Custom styling options
     */
    style?: ElementStyle;
}

/**
 * Element styling options
 */
export interface ElementStyle {
    /**
     * Width of the payment element
     * @default '100%'
     */
    width?: string;

    /**
     * Height of the payment element
     * @default '480px'
     */
    height?: string;

    /**
     * Border radius
     * @default '12px'
     */
    borderRadius?: string;
}

/**
 * Payment result returned on success
 */
export interface PaymentResult {
    /**
     * The session ID
     */
    sessionId: string;

    /**
     * Final payment status
     */
    status: 'Paid' | 'Overpaid';

    /**
     * Amount received in standard units (e.g. 10.5)
     */
    amountReceived: number;

    /**
     * Transaction hash on the blockchain
     */
    transactionHash?: string;
}

/**
 * Error information
 */
export interface PaymentError {
    /**
     * Error code
     */
    code: 'LOAD_TIMEOUT' | 'SESSION_NOT_FOUND' | 'NETWORK_ERROR' | 'UNKNOWN';

    /**
     * Human-readable error message
     */
    message: string;
}

/**
 * Event types that can be listened to
 */
export type IronixPayEventType =
    | 'ready'
    | 'payment_success'
    | 'payment_expired'
    | 'payment_detected'
    | 'error'
    | 'resize';

/**
 * Event callback signatures
 */
export interface IronixPayEventMap {
    ready: () => void;
    payment_success: (result: PaymentResult) => void;
    payment_expired: (data: { sessionId: string }) => void;
    payment_detected: (data: { sessionId: string; amountReceived: number }) => void;
    error: (error: PaymentError) => void;
    resize: (data: { height: number }) => void;
}

/**
 * Internal element options (after processing)
 */
export interface InternalElementOptions extends CreateElementOptions {
    baseUrl: string;
    allowedOrigin: string;
}
