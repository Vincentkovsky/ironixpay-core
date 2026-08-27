// Telegram Bot — IronixPay USDT Payment Demo
// Framework: grammY (https://grammy.dev)

import { Bot, InlineKeyboard } from "grammy";
import { createCheckoutSession, type Network } from "./ironixpay.js";
import { startWebhookServer } from "./webhook-server.js";

// ─── Config ──────────────────────────────────────────────────

const BOT_TOKEN = process.env.BOT_TOKEN;
if (!BOT_TOKEN) {
    console.error("❌ BOT_TOKEN is required. Get it from @BotFather.");
    process.exit(1);
}

const PUBLIC_URL = process.env.PUBLIC_URL || "http://localhost:3000";
const API_URL = process.env.IRONIXPAY_API_URL || "https://sandbox.ironixpay.com";

// Auto-detect: sandbox only supports TRON, production supports all 7 networks
const IS_PRODUCTION = !API_URL.includes("sandbox");

const ALL_NETWORKS: { value: Network; label: string }[] = [
    { value: "TRON", label: "TRON" },
    { value: "BSC", label: "BNB Chain" },
    { value: "POLYGON", label: "Polygon" },
    { value: "ARBITRUM", label: "Arbitrum" },
    { value: "BASE", label: "Base" },
    { value: "OPTIMISM", label: "Optimism" },
    { value: "ETHEREUM", label: "Ethereum" },
];

const AVAILABLE_NETWORKS = IS_PRODUCTION
    ? ALL_NETWORKS
    : ALL_NETWORKS.filter((n) => n.value === "TRON");

// ─── Products ────────────────────────────────────────────────

const PRODUCTS = [
    { id: "starter", name: "⚡ Starter Plan", price: "9.99", description: "Basic features for individuals" },
    { id: "pro", name: "🚀 Pro Plan", price: "29.99", description: "Advanced features for teams" },
    { id: "enterprise", name: "🏢 Enterprise", price: "99.99", description: "Full access for organizations" },
] as const;

// In-memory order store (use a real database in production!)
// Maps orderId → { chatId, productId }
export const pendingOrders = new Map<string, { chatId: number; productId: string }>();

// ─── Bot Setup ───────────────────────────────────────────────

export const bot = new Bot(BOT_TOKEN);

// /start — Welcome message
bot.command("start", async (ctx) => {
    const mode = IS_PRODUCTION ? "🟢 Production" : "🟡 Sandbox";
    await ctx.reply(
        `👋 Welcome to the IronixPay Demo Store!\n\n` +
        `Mode: ${mode}\n` +
        `Networks: ${AVAILABLE_NETWORKS.map((n) => n.label).join(", ")}\n\n` +
        `Use /buy to browse products and pay with USDT.\n` +
        `Use /help for more info.`,
    );
});

// /help
bot.command("help", async (ctx) => {
    await ctx.reply(
        "🔹 /buy — Browse products\n" +
        "🔹 /help — Show this message\n\n" +
        "Payments are processed via IronixPay.\n" +
        `Supported: USDT on ${AVAILABLE_NETWORKS.map((n) => n.label).join(", ")}.`,
    );
});

// /buy — Show product list with inline buttons
bot.command("buy", async (ctx) => {
    const keyboard = new InlineKeyboard();

    for (const product of PRODUCTS) {
        const priceStr = product.price;
        keyboard.text(
            `${product.name} — $${priceStr}`,
            `buy:${product.id}`
        ).row();
    }

    await ctx.reply("🛒 Choose a product:", { reply_markup: keyboard });
});

// Handle product selection → show confirmation (sandbox) or network selection (production)
bot.callbackQuery(/^buy:(.+)$/, async (ctx) => {
    const productId = ctx.match[1];
    const product = PRODUCTS.find((p) => p.id === productId);

    if (!product) {
        await ctx.answerCallbackQuery({ text: "Product not found" });
        return;
    }

    await ctx.answerCallbackQuery();

    const priceStr = product.price;

    if (IS_PRODUCTION && AVAILABLE_NETWORKS.length > 1) {
        // Production: let user pick a network first
        const keyboard = new InlineKeyboard();
        for (const net of AVAILABLE_NETWORKS) {
            keyboard.text(`⛓ ${net.label}`, `net:${product.id}:${net.value}`).row();
        }
        keyboard.text("❌ Cancel", "cancel");

        await ctx.editMessageText(
            `${product.name}\n` +
            `💰 Price: $${priceStr} USDT\n\n` +
            `Select a network:`,
            { reply_markup: keyboard },
        );
    } else {
        // Sandbox: go straight to confirm (TRON only)
        const keyboard = new InlineKeyboard()
            .text(`💳 Pay $${priceStr} USDT`, `pay:${product.id}:TRON`)
            .row()
            .text("❌ Cancel", "cancel");

        await ctx.editMessageText(
            `${product.name}\n\n` +
            `${product.description}\n\n` +
            `💰 Price: $${priceStr} USDT\n` +
            `⛓ Network: TRON`,
            { reply_markup: keyboard },
        );
    }
});

// Handle network selection (production only)
bot.callbackQuery(/^net:(.+):(.+)$/, async (ctx) => {
    const productId = ctx.match[1];
    const network = ctx.match[2] as Network;
    const product = PRODUCTS.find((p) => p.id === productId);
    const netLabel = AVAILABLE_NETWORKS.find((n) => n.value === network)?.label || network;

    if (!product) {
        await ctx.answerCallbackQuery({ text: "Product not found" });
        return;
    }

    await ctx.answerCallbackQuery();

    const priceStr = product.price;

    const keyboard = new InlineKeyboard()
        .text(`💳 Pay $${priceStr} USDT`, `pay:${product.id}:${network}`)
        .row()
        .text("◀️ Change Network", `buy:${product.id}`)
        .text("❌ Cancel", "cancel");

    await ctx.editMessageText(
        `${product.name}\n\n` +
        `${product.description}\n\n` +
        `💰 Price: $${priceStr} USDT\n` +
        `⛓ Network: ${netLabel}`,
        { reply_markup: keyboard },
    );
});

// Handle payment initiation — format: pay:{productId}:{network}
bot.callbackQuery(/^pay:(.+):(.+)$/, async (ctx) => {
    const productId = ctx.match[1];
    const network = ctx.match[2] as Network;
    const product = PRODUCTS.find((p) => p.id === productId);
    const netLabel = AVAILABLE_NETWORKS.find((n) => n.value === network)?.label || network;

    if (!product) {
        await ctx.answerCallbackQuery({ text: "Product not found" });
        return;
    }

    await ctx.answerCallbackQuery({ text: "Creating payment..." });

    try {
        const orderId = `tg_${ctx.from.id}_${Date.now()}`;

        const session = await createCheckoutSession({
            pricing_amount: product.price,
            pricing_currency: "USDT",
            currency: "USDT",
            network,
            success_url: `${PUBLIC_URL}/success`,
            cancel_url: `${PUBLIC_URL}/cancel`,
            client_reference_id: orderId,
        });

        // Store the order so we can notify the user on webhook
        pendingOrders.set(orderId, {
            chatId: ctx.chat!.id,
            productId: product.id,
        });

        const priceStr = product.price;

        const keyboard = new InlineKeyboard()
            .url("💳 Pay Now", session.url)
            .row()
            .text("🔄 Check Status", `status:${orderId}`);

        await ctx.editMessageText(
            `✅ Payment created!\n\n` +
            `🛒 ${product.name}\n` +
            `💰 Amount: $${priceStr} USDT\n` +
            `⛓ Network: ${netLabel}\n\n` +
            `Click the button below to pay:`,
            { reply_markup: keyboard },
        );
    } catch (error) {
        console.error("Payment creation failed:", error);
        await ctx.editMessageText(
            "❌ Failed to create payment. Please try again with /buy.",
        );
    }
});

// Handle cancel
bot.callbackQuery("cancel", async (ctx) => {
    await ctx.answerCallbackQuery();
    await ctx.editMessageText("❌ Order cancelled. Use /buy to start again.");
});

// Handle status check
bot.callbackQuery(/^status:(.+)$/, async (ctx) => {
    const orderId = ctx.match[1];
    const order = pendingOrders.get(orderId);

    if (order) {
        await ctx.answerCallbackQuery({
            text: "⏳ Payment pending — complete it via the Pay Now button",
            show_alert: true,
        });
    } else {
        await ctx.answerCallbackQuery({
            text: "✅ Payment completed or expired",
            show_alert: true,
        });
    }
});

// ─── Start ───────────────────────────────────────────────────

async function main() {
    // Start the webhook server first (receives IronixPay payment notifications)
    // Pass bot and pendingOrders to avoid circular imports
    startWebhookServer(bot, pendingOrders);

    // Start the bot
    const mode = IS_PRODUCTION ? "production" : "sandbox";
    console.log(`🤖 Bot starting in ${mode} mode (${AVAILABLE_NETWORKS.length} networks)...`);
    await bot.start({
        onStart: () => console.log("🤖 Bot is running!"),
    });
}

main().catch(console.error);
