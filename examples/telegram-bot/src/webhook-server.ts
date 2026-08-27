// IronixPay Webhook Server
// Receives payment notifications and notifies users via Telegram

import express from "express";
import type { Bot } from "grammy";
import { verifyWebhookSignature } from "./ironixpay.js";

const PORT = parseInt(process.env.WEBHOOK_PORT || "3000", 10);
const WEBHOOK_SECRET = process.env.IRONIXPAY_WEBHOOK_SECRET || "";

/**
 * Start an Express server to receive IronixPay webhook notifications.
 *
 * Bot and pendingOrders are passed as parameters to avoid circular imports
 * (index.ts → webhook-server.ts → index.ts).
 */
export function startWebhookServer(
    bot: Bot,
    pendingOrders: Map<string, { chatId: number; productId: string }>
) {
    const app = express();

    // Parse raw body for signature verification
    app.use("/webhooks", express.text({ type: "application/json" }));

    // Health check
    app.get("/health", (_req, res) => {
        res.json({ status: "ok" });
    });

    // IronixPay webhook endpoint
    app.post("/webhooks/ironixpay", async (req, res) => {
        try {
            const body = req.body as string;
            const signature = (req.headers["x-signature"] as string) || "";
            const timestamp = (req.headers["x-timestamp"] as string) || "";

            // Verify signature
            if (WEBHOOK_SECRET) {
                const isValid = verifyWebhookSignature(
                    body,
                    signature,
                    timestamp,
                    WEBHOOK_SECRET
                );
                if (!isValid) {
                    console.warn("⚠️ Webhook signature verification failed");
                    res.status(401).json({ error: "Invalid signature" });
                    return;
                }
            }

            // Parse event
            const event = JSON.parse(body);
            console.log("📨 Webhook received:", event.event_type, event.data?.id);

            switch (event.event_type) {
                case "session.completed": {
                    const { client_reference_id, amount_received } = event.data;
                    const order = pendingOrders.get(client_reference_id);

                    if (order) {
                        const amount = (
                            parseInt(amount_received, 10) / 1_000_000
                        ).toFixed(2);

                        await bot.api.sendMessage(
                            order.chatId,
                            `✅ Payment confirmed!\n\n` +
                            `💰 Received: $${amount} USDT\n` +
                            `📦 Order: ${client_reference_id}\n\n` +
                            `Thank you for your purchase! 🎉`
                        );

                        // Clean up
                        pendingOrders.delete(client_reference_id);
                        console.log(`✅ Order ${client_reference_id} fulfilled`);
                    }
                    break;
                }

                case "session.expired": {
                    const { client_reference_id } = event.data;
                    const order = pendingOrders.get(client_reference_id);

                    if (order) {
                        await bot.api.sendMessage(
                            order.chatId,
                            `⏰ Payment session expired.\n\n` +
                            `Use /buy to create a new order.`
                        );

                        pendingOrders.delete(client_reference_id);
                        console.log(`⏰ Order ${client_reference_id} expired`);
                    }
                    break;
                }

                default:
                    console.log("Unhandled event type:", event.event_type);
            }

            res.json({ received: true });
        } catch (error) {
            console.error("Webhook error:", error);
            res.status(500).json({ error: "Webhook processing failed" });
        }
    });

    app.listen(PORT, () => {
        console.log(`🌐 Webhook server listening on port ${PORT}`);
        console.log(`   Webhook URL: POST /webhooks/ironixpay`);
    });
}
