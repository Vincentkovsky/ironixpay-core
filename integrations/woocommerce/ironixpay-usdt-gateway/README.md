# IronixPay Gateway for WooCommerce

Accept **USDT & USDC stablecoin payments** on 8 blockchains through your WooCommerce store. Supports fiat pricing in USD, EUR, CNY, GBP, JPY and more.

| Feature | Details |
|---------|---------|
| **Supported Chains** | TRON, BSC, Ethereum, Polygon, Arbitrum, Base, Optimism, Solana |
| **Currencies** | USDT (all 8 networks), USDC (all except TRON) |
| **Pricing** | Fiat (USD, CNY, EUR, GBP, JPY, KRW, SGD, HKD, TWD, RUB) or direct crypto (USDT/USDC) |
| **Integration** | Redirect mode — hosted checkout page |
| **Order Updates** | Automatic via Webhook |
| **Security** | HMAC-SHA256 webhook signature verification |

## Installation

1. Download or clone this plugin folder into `wp-content/plugins/ironixpay-gateway/`
2. Activate the plugin in **WordPress Admin → Plugins**
3. Go to **WooCommerce → Settings → Payments → IronixPay** to configure

## Configuration

### Step 1: Get API Credentials

1. Sign up at [ironixpay.com](https://app.ironixpay.com)
2. Go to **Dashboard → API Keys** and create a key
3. Go to **Dashboard → Webhook** and create an endpoint

### Step 2: Configure the Plugin

In **WooCommerce → Settings → Payments → IronixPay**:

| Setting | Description |
|---------|-------------|
| **Environment** | `Sandbox` for testing, `Production` for live payments |
| **API Key** | Your `sk_live_...` or `sk_test_...` key |
| **Webhook Secret** | Your webhook signing secret |
| **Supported Currencies** | Select USDT, USDC, or both |
| **Supported Networks** | Select which chains to offer customers at checkout |

### Step 3: Configure Webhook URL

In your **IronixPay Dashboard → Webhook settings**, set the URL to:

```
https://your-store.com/?wc-api=ironixpay_webhook
```

> **Important:** Replace `your-store.com` with your actual store domain.

## How It Works

```
Customer → Checkout → Select Currency + Network → Pay
    ↓
WooCommerce → IronixPay API → Create Session (fiat → crypto conversion)
    ↓
Redirect → Hosted Payment Page → Customer pays USDT/USDC
    ↓
Blockchain confirmed → Webhook → Order status → Processing
```

1. Customer selects "Pay with Crypto" and chooses a currency and blockchain at checkout
2. WooCommerce creates an IronixPay checkout session via API
3. The order total (in your store currency) is converted to the crypto amount
4. Customer is redirected to the hosted payment page
5. Customer sends USDT/USDC to the displayed address
6. IronixPay detects the payment on-chain
7. Webhook callback automatically updates the WooCommerce order to "Processing"

## Pricing

Products are priced in your store's WooCommerce currency. Supported fiat currencies: **USD, CNY, EUR, GBP, JPY, KRW, SGD, HKD, TWD, RUB**. IronixPay converts the order total to the selected cryptocurrency at the current market rate. If your store currency is USDT/USDC, settlement is 1:1. If your store uses an unsupported currency, the IronixPay payment option will not appear at checkout.

## Sandbox Testing

1. Set environment to **Sandbox**
2. Use your `sk_test_...` API key
3. Only **TRON (Nile testnet)** is available in sandbox mode
4. Place a test order and complete payment on the Nile testnet

## FAQ

**Q: What currencies can customers pay with?**
A: USDT (Tether) and USDC (Circle). You choose which to enable in plugin settings.

**Q: Can I price products in USD or EUR?**
A: Yes. Supported fiat currencies: USD, CNY, EUR, GBP, JPY, KRW, SGD, HKD, TWD, RUB. The API converts automatically. If your store uses a different currency, IronixPay won't appear at checkout.

**Q: Can customers choose which blockchain to pay on?**
A: Yes. You select which chains to support in plugin settings, and customers pick from those options at checkout.

**Q: How long until my order is updated?**
A: Typically within 1-2 minutes after on-chain confirmation (depends on the blockchain).

**Q: What if a customer overpays?**
A: The order is still marked as "Processing". The overpayment amount is noted in the order comments.

## Requirements

- WordPress 5.8+
- WooCommerce 7.0+
- PHP 7.4+
- An IronixPay account with API credentials

## Support

- Documentation: [ironixpay.com/guide](https://ironixpay.com/guide)
- Issues: [GitHub Issues](https://github.com/Vincentkovsky/ironixpay-core/issues)
- Email: support@ironixpay.com

## License

GPLv2 or later. See [LICENSE](https://www.gnu.org/licenses/gpl-2.0.html).
