import { NextResponse } from "next/server";
import { verifyWebhookSignature } from "@/lib/ironixpay";

/**
 * IronixPay Webhook Handler
 *
 * Receives payment notifications from IronixPay.
 * Configure your webhook URL in the IronixPay dashboard:
 *   https://yourdomain.com/api/webhooks/ironixpay
 *
 * Events:
 *   - session.completed — Payment confirmed on-chain
 *   - session.expired   — Session timed out
 */
export async function POST(request: Request) {
    try {
        const body = await request.text();
        const signature = request.headers.get("x-signature") || "";
        const timestamp = request.headers.get("x-timestamp") || "";
        const secret = process.env.IRONIXPAY_WEBHOOK_SECRET || "";

        // Verify HMAC-SHA256 signature
        if (secret) {
            const isValid = await verifyWebhookSignature(
                body,
                signature,
                timestamp,
                secret
            );
            if (!isValid) {
                console.warn("Webhook signature verification failed");
                return NextResponse.json(
                    { error: "Invalid signature" },
                    { status: 401 }
                );
            }
        }

        // Webhook payload structure:
        // { id, event_type, created, data: { ...session fields } }
        const event = JSON.parse(body);
        console.log("Webhook received:", event.event_type, event.data?.id);

        // Handle different event types
        switch (event.event_type) {
            case "session.completed":
                // Payment confirmed — fulfill the order
                console.log("✅ Payment confirmed:", {
                    sessionId: event.data.id,
                    amount: event.data.amount_received,
                    network: event.data.network,
                    clientReferenceId: event.data.client_reference_id,
                });
                // TODO: Update your database, send confirmation email, etc.
                break;

            case "session.expired":
                // Session expired — mark order as cancelled
                console.log("⏰ Session expired:", event.data.id);
                // TODO: Cancel the pending order
                break;

            default:
                console.log("Unhandled event type:", event.event_type);
        }

        return NextResponse.json({ received: true });
    } catch (error) {
        console.error("Webhook error:", error);
        return NextResponse.json(
            { error: "Webhook processing failed" },
            { status: 500 }
        );
    }
}
