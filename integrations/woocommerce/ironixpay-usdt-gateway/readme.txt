=== IronixPay Crypto Payment Gateway for WooCommerce ===
Contributors: ironixpay
Tags: woocommerce, payment gateway, crypto, usdt, usdc
Requires at least: 5.8
Tested up to: 6.9
Requires PHP: 7.4
WC requires at least: 7.0
WC tested up to: 9.6
Stable tag: 1.2.0
License: GPLv2 or later
License URI: https://www.gnu.org/licenses/gpl-2.0.html

Accept USDT & USDC stablecoin payments on 8 blockchain networks via IronixPay. Supports fiat pricing (USD, EUR, CNY, GBP, JPY and more) with automatic crypto conversion.

== Description ==

**IronixPay Crypto Payment Gateway for WooCommerce** lets your store accept USDT (Tether) and USDC (Circle) stablecoin payments across 8 blockchain networks. Price your products in supported fiat currencies (USD, EUR, CNY, GBP, JPY, KRW, SGD, HKD, TWD, RUB) — customers pay the equivalent in crypto. No volatility, no chargebacks.

= Supported Currencies =

* USDT (Tether) — Available on all 8 networks
* USDC (Circle) — Available on BSC, Ethereum, Polygon, Arbitrum, Base, Optimism, Solana

= Supported Networks =

* TRON (TRC-20) — USDT only
* BSC / BNB Chain (BEP-20)
* Ethereum (ERC-20)
* Polygon (ERC-20)
* Arbitrum (ERC-20)
* Base (ERC-20)
* Optimism (ERC-20)
* Solana (SPL Token)

= How It Works =

1. Customer selects "Pay with Crypto" at checkout and chooses a currency and blockchain network.
2. Customer is redirected to a secure IronixPay hosted checkout page.
3. IronixPay monitors the blockchain for payment and confirms automatically.
4. Webhook notifies your store — order status updates to "Processing".
5. Funds are swept to your treasury wallet.

= Key Features =

* **Multi-currency** — Accept both USDT and USDC stablecoins.
* **Multi-chain support** — 8 networks, one plugin.
* **Fiat pricing** — Price products in USD, EUR, CNY, GBP, JPY, KRW, SGD, HKD, TWD, or RUB. Automatic conversion to crypto at checkout.
* **Hosted checkout** — No sensitive data touches your server.
* **Automatic payment detection** — On-chain monitoring with confirmation.
* **Webhook-driven** — HMAC-SHA256 signed callbacks for secure order updates.
* **WooCommerce Block Checkout** — Full support for the new block-based checkout.
* **HPOS compatible** — Works with WooCommerce High-Performance Order Storage.
* **Sandbox mode** — Test with TRON Nile testnet before going live.
* **Idempotent processing** — Safe against duplicate webhook deliveries.
* **AML integration** — Blocked payments are flagged and put on hold.
* **Overpayment handling** — Automatic detection with order notes.

= Requirements =

* WordPress 5.8+
* WooCommerce 7.0+
* PHP 7.4+
* An IronixPay merchant account ([sign up at ironixpay.com](https://ironixpay.com))

== Installation ==

1. Upload the `ironixpay-usdt-gateway` folder to `/wp-content/plugins/`, or install via **Plugins → Add New → Upload Plugin**.
2. Activate the plugin through the **Plugins** menu.
3. Go to **WooCommerce → Settings → Payments → IronixPay - Crypto Payment**.
4. Enable the gateway, select your environment (Sandbox or Production).
5. Enter your API Key and Webhook Secret from the [IronixPay Dashboard](https://dashboard.ironixpay.com).
6. Copy the displayed **Webhook URL** and paste it into your IronixPay Dashboard → Webhook settings.
7. Select which currencies (USDT, USDC, or both) and blockchain networks to offer at checkout.
8. Save changes.

== Frequently Asked Questions ==

= What currencies can I accept? =

USDT (Tether) and USDC (Circle). You can enable one or both in the plugin settings. USDC is available on all networks except TRON (where only USDT is supported).

= What currency are products priced in? =

Products are priced in your store's default currency (e.g., USD, EUR). The order total is sent to IronixPay's API which handles the conversion. If your store currency is already USDT/USDC, it's a 1:1 settlement.

= Can I price products in fiat currencies? =

Yes! The following fiat currencies are supported: USD, CNY, EUR, GBP, JPY, KRW, SGD, HKD, TWD, and RUB. The order total is sent to the IronixPay API, which converts it to the selected cryptocurrency at the current market rate. If your store uses a different currency, the IronixPay payment option will not appear at checkout.

= Is there a minimum payment amount? =

Yes, the minimum payment is approximately 1 USDT/USDC equivalent. The exact minimum is validated by the IronixPay API at checkout.

= What happens if the customer overpays? =

Overpayments are detected automatically. The order is marked as paid with a note indicating the overpaid amount. Excess funds can be managed via the IronixPay Dashboard.

= What happens if the payment session expires? =

If no payment is received within the time limit, the order is marked as "Failed" with an appropriate note. Any partial payments are noted for resolution via the IronixPay Resolution Center.

= How does the webhook verification work? =

Each webhook request includes an `X-Signature` header (HMAC-SHA256) and an `X-Timestamp` header. The plugin verifies the signature using your Webhook Secret and rejects requests with timestamps older than 5 minutes (anti-replay protection).

= Does this plugin support the new WooCommerce Block Checkout? =

Yes! The plugin fully supports both the classic shortcode checkout and the new block-based checkout introduced in WooCommerce 8.3+.

= Can I test before going live? =

Yes. Set the environment to "Sandbox" and use your test API key. Sandbox mode uses the TRON Nile testnet. Note: USDC is not available in Sandbox mode.

== Screenshots ==

1. Payment method selection at checkout with currency and network cards.
2. IronixPay gateway settings in WooCommerce.
3. Order received page after successful payment redirect.

== External services ==

This plugin connects to the IronixPay API to create cryptocurrency payment checkout sessions. It is needed to process USDT and USDC stablecoin payments on behalf of the merchant.

When a customer chooses to pay with crypto at checkout, the plugin sends the order total, currency, selected cryptocurrency, blockchain network, and order reference ID to the IronixPay API. The customer is then redirected to a hosted payment page on IronixPay. No customer personal data (name, email, address) is sent.

IronixPay also sends signed webhook callbacks to your store when a payment is confirmed, expired, or flagged.

API endpoints used:
* Production: https://api.ironixpay.com
* Sandbox: https://sandbox.ironixpay.com

This service is provided by IronixPay: [Terms of Service](https://ironixpay.com/en/terms), [Privacy Policy](https://ironixpay.com/en/privacy).

== Changelog ==

= 1.2.0 =
* Added Solana (SPL Token) as the 8th supported blockchain network.
* Solana supports both USDT and USDC.
* Clarified fiat pricing support — 10 fiat currencies (USD, CNY, EUR, GBP, JPY, KRW, SGD, HKD, TWD, RUB) with automatic conversion.
* Improved minimum amount validation (now handled server-side for accuracy after fiat conversion).
* Added "External services" disclosure section for WordPress.org compliance.
* Updated plugin descriptions and FAQ.

= 1.1.0 =
* Added USDC (Circle) support alongside USDT.
* New "Supported Currencies" setting — choose USDT, USDC, or both.
* Dynamic network filtering — TRON automatically hidden when USDC is selected.
* Updated checkout UI with currency toggle buttons (Block checkout).
* Sandbox USDC validation (graceful error message).
* Webhook order notes now display correct currency.
* Plugin renamed to "IronixPay Crypto Payment Gateway for WooCommerce".

= 1.0.0 =
* Initial release.
* Multi-chain USDT payments (7 networks).
* WooCommerce Block Checkout support.
* HPOS compatibility.
* Webhook signature verification with anti-replay.
* Sandbox/Production environment switching.
* Network card selector with chain icons.

== Upgrade Notice ==

= 1.2.0 =
Adds Solana network support (USDT & USDC). Update and enable Solana in WooCommerce → Settings → Payments → IronixPay.

= 1.1.0 =
Adds USDC support. Update and enable USDC in WooCommerce → Settings → Payments → IronixPay.

= 1.0.0 =
Initial release. Install and configure to start accepting USDT payments.
