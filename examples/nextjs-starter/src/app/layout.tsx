import type { Metadata } from "next";

export const metadata: Metadata = {
    title: "IronixPay Next.js Starter",
    description: "Accept USDT payments with IronixPay — Next.js starter template",
};

export default function RootLayout({
    children,
}: {
    children: React.ReactNode;
}) {
    return (
        <html lang="en">
            <body
                style={{
                    margin: 0,
                    fontFamily:
                        '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
                    backgroundColor: "#0f0f0f",
                    color: "#e5e5e5",
                    minHeight: "100vh",
                }}
            >
                {children}
            </body>
        </html>
    );
}
