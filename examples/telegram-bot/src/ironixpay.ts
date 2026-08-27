// IronixPay API Helper for Telegram Bot
// Docs: https://ironixpay.com/guide/quickstart

import crypto from "node:crypto";

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
    /** Redirect URL after successful payment (for web) */
    success_url: string;
    /** Redirect URL when session expires */
    cancel_url: string;
    /** Your internal order ID */
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

// ─── Config ──────────────────────────────────────────────────

const API_URL =
    process.env.IRONIXPAY_API_URL || "https://sandbox.ironixpay.com";
const SECRET_KEY = process.env.IRONIXPAY_SECRET_KEY || "";

// ─── Create Checkout Session ─────────────────────────────────

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

// ─── Webhook Signature Verification ─────────────────────────

/**
 * Verify an IronixPay webhook signature (HMAC-SHA256).
 *
 * IronixPay sends two headers:
 *   - `X-Signature`: HMAC-SHA256 hex digest
 *   - `X-Timestamp`: Unix timestamp (seconds)
 *
 * The signed message is: `${timestamp}.${payload}`
 */
export function verifyWebhookSignature(
    payload: string,
    signature: string,
    timestamp: string,
    secret: string
): boolean {
    // Reject if timestamp is too old (5 min window)
    const now = Math.floor(Date.now() / 1000);
    if (Math.abs(now - parseInt(timestamp, 10)) > 300) {
        return false;
    }

    const message = `${timestamp}.${payload}`;
    const computed = crypto
        .createHmac("sha256", secret)
        .update(message)
        .digest("hex");

    // Guard: timingSafeEqual throws if buffer lengths differ.
    // A malformed signature (wrong length or invalid hex) would crash.
    if (computed.length !== signature.length) {
        return false;
    }

    try {
        return crypto.timingSafeEqual(
            Buffer.from(computed, "hex"),
            Buffer.from(signature, "hex")
        );
    } catch {
        return false;
    }
}
