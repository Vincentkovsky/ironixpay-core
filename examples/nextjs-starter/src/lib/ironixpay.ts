// IronixPay API Helper
// Docs: https://ironixpay.com/guide/quickstart

// ─── Types ───────────────────────────────────────────────────

export type Network =
    | "TRON"
    | "BSC"
    | "ETHEREUM"
    | "POLYGON"
    | "ARBITRUM"
    | "OPTIMISM"
    | "BASE";

export interface CreateSessionParams {
    /** Amount in human-readable format, e.g. "10.50" for 10.50 USDT */
    pricing_amount: string;
    /** Pricing currency. Crypto ("USDT", "USDC") or fiat ("USD", "CNY", "EUR", etc.) */
    pricing_currency: string;
    /** On-chain settlement token: "USDT" or "USDC" */
    currency: "USDT" | "USDC";
    /** Blockchain network */
    network: Network;
    /** Redirect URL after successful payment */
    success_url: string;
    /** Redirect URL when session expires or is cancelled */
    cancel_url: string;
    /** Your internal order ID (optional) */
    client_reference_id?: string;
}

export interface CheckoutSession {
    id: string;
    livemode: boolean;
    url: string;
    status: string;
    amount_expected: string;
    amount_received: string;
    currency: string;
    network: string;
    merchant_name: string;
    pay_address: string;
    client_reference_id: string | null;
    pricing: {
        currency: string;
        amount: string;
        exchange_rate: string;
    };
    success_url: string;
    cancel_url: string;
    expires_at: string;
    created_at: string;
}

// ─── Helper ──────────────────────────────────────────────────

const API_URL =
    process.env.IRONIXPAY_API_URL || "https://sandbox.ironixpay.com";
const SECRET_KEY = process.env.IRONIXPAY_SECRET_KEY || "";

/**
 * Create a new IronixPay Checkout Session.
 *
 * @example
 * ```ts
 * const session = await createCheckoutSession({
 *   pricing_amount: "10.00",
 *   pricing_currency: "USDT",
 *   currency: "USDT",
 *   network: "TRON",
 *   success_url: "https://example.com/success",
 *   cancel_url: "https://example.com/cancel",
 * });
 * // Redirect user to session.url
 * ```
 */
export async function createCheckoutSession(
    params: CreateSessionParams
): Promise<CheckoutSession> {
    const res = await fetch(`${API_URL}/v1/checkout/sessions`, {
        method: "POST",
        headers: {
            Authorization: `Bearer ${SECRET_KEY}`,
            "Content-Type": "application/json",
        },
        body: JSON.stringify(params),
    });

    if (!res.ok) {
        const error = await res.json().catch(() => ({}));
        throw new Error(
            `IronixPay API error (${res.status}): ${error.message || res.statusText
            }`
        );
    }

    return res.json();
}

// ─── Webhook Verification ────────────────────────────────────

/**
 * Verify an IronixPay webhook signature (HMAC-SHA256).
 *
 * IronixPay sends two headers:
 *   - `X-Signature`: HMAC-SHA256 hex digest
 *   - `X-Timestamp`: Unix timestamp (seconds)
 *
 * The signed message is: `${timestamp}.${payload}`
 *
 * @param payload - Raw request body as string
 * @param signature - Value of the `X-Signature` header
 * @param timestamp - Value of the `X-Timestamp` header
 * @param secret - Your webhook secret
 * @returns true if the signature is valid
 */
export async function verifyWebhookSignature(
    payload: string,
    signature: string,
    timestamp: string,
    secret: string
): Promise<boolean> {
    // Reject if timestamp is too old (5 min window) to prevent replay attacks
    const now = Math.floor(Date.now() / 1000);
    if (Math.abs(now - parseInt(timestamp, 10)) > 300) {
        return false;
    }

    const encoder = new TextEncoder();
    const key = await crypto.subtle.importKey(
        "raw",
        encoder.encode(secret),
        { name: "HMAC", hash: "SHA-256" },
        false,
        ["sign"]
    );
    // Sign: timestamp.payload (matches backend: format!("{}.{}", timestamp, payload))
    const message = `${timestamp}.${payload}`;
    const sig = await crypto.subtle.sign("HMAC", key, encoder.encode(message));
    const computed = Array.from(new Uint8Array(sig))
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("");

    return computed === signature;
}
