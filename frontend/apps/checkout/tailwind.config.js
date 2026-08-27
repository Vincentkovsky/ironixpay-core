/** @type {import('tailwindcss').Config} */
export default {
    content: [
        "./index.html",
        "./src/**/*.{vue,js,ts,jsx,tsx}",
    ],
    theme: {
        extend: {
            colors: {
                brand: {
                    blue: '#2563eb', // IronixPay brand blue
                    dark: '#1e293b', // Example - adjust to design
                    success: '#10b981',
                    warning: '#f59e0b',
                    error: '#ef4444',
                }
            },
            fontFamily: {
                sans: ['Inter', 'sans-serif'],
            }
        },
    },
    plugins: [],
}
