<?php
/**
 * Plugin Name: IronixPay Crypto Payment Gateway for WooCommerce
 * Plugin URI: https://ironixpay.com/en/use-cases/woocommerce
 * Description: Accept USDT & USDC stablecoin payments on 8 blockchains (TRON, BSC, Ethereum, Polygon, Arbitrum, Base, Optimism, Solana) via IronixPay. Supports fiat pricing with automatic crypto conversion.
 * Version: 1.2.0
 * Author: IronixPay
 * Author URI: https://ironixpay.com
 * License: GPLv2 or later
 * License URI: https://www.gnu.org/licenses/gpl-2.0.html
 * Requires at least: 5.8
 * Tested up to: 6.9
 * WC requires at least: 7.0
 * WC tested up to: 9.6
 * Requires PHP: 7.4
 * Text Domain: ironixpay-usdt-gateway
 */

defined('ABSPATH') || exit;

define('IRONIXPAY_VERSION', '1.2.0');
define('IRONIXPAY_PLUGIN_FILE', __FILE__);
define('IRONIXPAY_PLUGIN_DIR', plugin_dir_path(__FILE__));
define('IRONIXPAY_PLUGIN_URL', plugin_dir_url(__FILE__));

/**
 * Declare HPOS compatibility.
 * Must be at the top level so it registers BEFORE WooCommerce checks at plugins_loaded priority 10.
 */
add_action('before_woocommerce_init', function () {
    if (class_exists('\Automattic\WooCommerce\Utilities\FeaturesUtil')) {
        \Automattic\WooCommerce\Utilities\FeaturesUtil::declare_compatibility('custom_order_tables', IRONIXPAY_PLUGIN_FILE, true);
    }
});

/**
 * Register WooCommerce Blocks checkout integration.
 * Must be at the top level so it registers before woocommerce_blocks_loaded fires.
 */
add_action('woocommerce_blocks_loaded', function () {
    if (class_exists('Automattic\WooCommerce\Blocks\Payments\Integrations\AbstractPaymentMethodType')) {
        require_once IRONIXPAY_PLUGIN_DIR . 'includes/class-wc-gateway-ironixpay-blocks.php';

        add_action(
            'woocommerce_blocks_payment_method_type_registration',
            function ($payment_method_registry) {
                $payment_method_registry->register(new WC_Gateway_IronixPay_Blocks());
            }
        );
    }
});

/**
 * Check if WooCommerce is active and load the gateway.
 */
add_action('plugins_loaded', 'ironixpay_init', 11);

function ironixpay_init()
{
    if (!class_exists('WC_Payment_Gateway')) {
        add_action('admin_notices', function () {
            echo '<div class="error"><p><strong>IronixPay Gateway</strong> requires WooCommerce to be installed and activated.</p></div>';
        });
        return;
    }

    // Include gateway classes
    require_once IRONIXPAY_PLUGIN_DIR . 'includes/class-ironixpay-api.php';
    require_once IRONIXPAY_PLUGIN_DIR . 'includes/class-ironixpay-webhook.php';
    require_once IRONIXPAY_PLUGIN_DIR . 'includes/class-wc-gateway-ironixpay.php';

    // Register the gateway
    add_filter('woocommerce_payment_gateways', function ($gateways) {
        $gateways[] = 'WC_Gateway_IronixPay';
        return $gateways;
    });

    // Register webhook handler route
    add_action('woocommerce_api_ironixpay_webhook', function () {
        $gateway = new WC_Gateway_IronixPay();
        $webhook = new IronixPay_Webhook($gateway);
        $webhook->handle();
    });
}

/**
 * Add "Settings" link to the plugins page.
 */
add_filter('plugin_action_links_' . plugin_basename(__FILE__), function ($links) {
    $settings_url = admin_url('admin.php?page=wc-settings&tab=checkout&section=ironixpay');
    array_unshift($links, '<a href="' . esc_url($settings_url) . '">' . __('Settings', 'ironixpay-usdt-gateway') . '</a>');
    return $links;
});
