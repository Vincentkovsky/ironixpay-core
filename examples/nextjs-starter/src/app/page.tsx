"use client";

import { useState } from "react";

const NETWORKS = [
    { value: "TRON", label: "TRON" },
    { value: "BSC", label: "BNB Chain" },
    { value: "POLYGON", label: "Polygon" },
    { value: "ARBITRUM", label: "Arbitrum" },
    { value: "BASE", label: "Base" },
    { value: "OPTIMISM", label: "Optimism" },
    { value: "ETHEREUM", label: "Ethereum" },
] as const;

const PRODUCTS = [
    { name: "Starter Plan", price: 9.99, emoji: "🚀" },
    { name: "Pro Plan", price: 29.99, emoji: "⚡" },
    { name: "Enterprise Plan", price: 99.99, emoji: "🏢" },
];

export default function Home() {
    const [selectedProduct, setSelectedProduct] = useState(0);
    const [network, setNetwork] = useState("TRON");
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState("");

    const handleCheckout = async () => {
        setLoading(true);
        setError("");

        try {
            // Convert price to USDT micro-units (1 USDT = 1,000,000)
            const amount = Math.round(PRODUCTS[selectedProduct].price * 1_000_000);

            const res = await fetch("/api/checkout", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ amount, network }),
            });

            const data = await res.json();

            if (!res.ok) {
                throw new Error(data.error || "Failed to create checkout session");
            }

            // Redirect to IronixPay hosted checkout page
            window.location.href = data.url;
        } catch (err) {
            setError(err instanceof Error ? err.message : "Something went wrong");
        } finally {
            setLoading(false);
        }
    };

    return (
        <main
            style={{
                maxWidth: 640,
                margin: "0 auto",
                padding: "60px 24px",
            }}
        >
            {/* Header */}
            <div style={{ textAlign: "center", marginBottom: 48 }}>
                <h1
                    style={{
                        fontSize: 32,
                        fontWeight: 700,
                        margin: "0 0 8px",
                        color: "#fff",
                    }}
                >
                    IronixPay Demo Store
                </h1>
                <p style={{ color: "#888", fontSize: 16, margin: 0 }}>
                    Pay with USDT on 7 blockchains
                </p>
            </div>

            {/* Product Selection */}
            <div style={{ display: "flex", gap: 12, marginBottom: 32 }}>
                {PRODUCTS.map((product, i) => (
                    <button
                        key={i}
                        onClick={() => setSelectedProduct(i)}
                        style={{
                            flex: 1,
                            padding: "20px 16px",
                            border:
                                selectedProduct === i
                                    ? "2px solid #10b981"
                                    : "2px solid #2a2a2a",
                            borderRadius: 12,
                            background: selectedProduct === i ? "#10b981" + "15" : "#1a1a1a",
                            color: "#fff",
                            cursor: "pointer",
                            textAlign: "center",
                            transition: "all 0.2s",
                        }}
                    >
                        <div style={{ fontSize: 28, marginBottom: 8 }}>
                            {product.emoji}
                        </div>
                        <div style={{ fontSize: 14, fontWeight: 600 }}>{product.name}</div>
                        <div
                            style={{ fontSize: 20, fontWeight: 700, marginTop: 4, color: "#10b981" }}
                        >
                            ${product.price}
                        </div>
                    </button>
                ))}
            </div>

            {/* Network Selection */}
            <div style={{ marginBottom: 32 }}>
                <label
                    style={{
                        display: "block",
                        fontSize: 14,
                        fontWeight: 600,
                        marginBottom: 10,
                        color: "#999",
                    }}
                >
                    Select Network
                </label>
                <div
                    style={{
                        display: "grid",
                        gridTemplateColumns: "repeat(4, 1fr)",
                        gap: 8,
                    }}
                >
                    {NETWORKS.map((n) => (
                        <button
                            key={n.value}
                            onClick={() => setNetwork(n.value)}
                            style={{
                                padding: "10px 8px",
                                borderRadius: 8,
                                border:
                                    network === n.value
                                        ? "1.5px solid #10b981"
                                        : "1.5px solid #2a2a2a",
                                background:
                                    network === n.value ? "#10b98115" : "#1a1a1a",
                                color: network === n.value ? "#10b981" : "#aaa",
                                cursor: "pointer",
                                fontSize: 13,
                                fontWeight: network === n.value ? 600 : 400,
                                transition: "all 0.15s",
                                textAlign: "center",
                            }}
                        >
                            {n.label}
                        </button>
                    ))}
                </div>
                <p style={{ margin: "8px 0 0", fontSize: 12, color: "#555" }}>
                    ⚠ Sandbox mode only supports TRON. Other networks require a production API key.
                </p>
            </div>

            {/* Checkout Button */}
            <button
                onClick={handleCheckout}
                disabled={loading}
                style={{
                    width: "100%",
                    padding: "16px 24px",
                    borderRadius: 12,
                    border: "none",
                    background: loading
                        ? "#666"
                        : "linear-gradient(135deg, #10b981, #059669)",
                    color: "#fff",
                    fontSize: 18,
                    fontWeight: 700,
                    cursor: loading ? "not-allowed" : "pointer",
                    transition: "all 0.2s",
                }}
            >
                {loading ? "Creating session..." : `Pay $${PRODUCTS[selectedProduct].price} USDT`}
            </button>

            {/* Error */}
            {error && (
                <div
                    style={{
                        marginTop: 16,
                        padding: "12px 16px",
                        borderRadius: 8,
                        background: "#ef444420",
                        border: "1px solid #ef4444",
                        color: "#ef4444",
                        fontSize: 14,
                    }}
                >
                    {error}
                </div>
            )}

            {/* Footer */}
            <div
                style={{
                    marginTop: 48,
                    textAlign: "center",
                    color: "#555",
                    fontSize: 13,
                }}
            >
                <p>
                    Powered by{" "}
                    <a
                        href="https://ironixpay.com"
                        target="_blank"
                        rel="noopener"
                        style={{ color: "#10b981", textDecoration: "none" }}
                    >
                        IronixPay
                    </a>
                    {" "}— 1% fee, 7 chains, 1 API
                </p>
                <p style={{ marginTop: 8, fontSize: 12, color: "#444" }}>
                    This is a demo using the Sandbox environment. No real payments.
                </p>
            </div>
        </main>
    );
}
