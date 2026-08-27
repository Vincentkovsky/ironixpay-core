import Link from "next/link";

export default function CancelPage() {
    return (
        <main
            style={{
                maxWidth: 480,
                margin: "0 auto",
                padding: "100px 24px",
                textAlign: "center",
            }}
        >
            <div style={{ fontSize: 64, marginBottom: 24 }}>⏰</div>
            <h1
                style={{ fontSize: 28, fontWeight: 700, color: "#f59e0b", margin: "0 0 12px" }}
            >
                Payment Cancelled
            </h1>
            <p style={{ color: "#888", fontSize: 16, marginBottom: 32 }}>
                The checkout session has expired or was cancelled. No payment was
                processed.
            </p>
            <Link
                href="/"
                style={{
                    display: "inline-block",
                    padding: "12px 32px",
                    borderRadius: 8,
                    background: "#1a1a1a",
                    border: "1px solid #333",
                    color: "#fff",
                    textDecoration: "none",
                    fontSize: 14,
                    fontWeight: 600,
                }}
            >
                ← Try Again
            </Link>
        </main>
    );
}
