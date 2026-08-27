<?php
/**
 * IronixPay Block Checkout Integration
 *
 * Server-side registration for WooCommerce Blocks checkout.
 * Extends AbstractPaymentMethodType to provide settings data to the
 * client-side JS component and register the frontend script.
 */

defined('ABSPATH') || exit;

use Automattic\WooCommerce\Blocks\Payments\Integrations\AbstractPaymentMethodType;

final class WC_Gateway_IronixPay_Blocks extends AbstractPaymentMethodType
{

    /**
     * Must match the 'name' used in registerPaymentMethod() on the JS side.
     * @var string
     */
    protected $name = 'ironixpay';

    /** @var WC_Gateway_IronixPay|null */
    private $gateway = null;

    /**
     * Called on every request. Load settings here.
     */
    public function initialize()
    {
        $this->settings = get_option('woocommerce_ironixpay_settings', array());
    }

    /**
     * Whether the gateway is enabled.
     * @return bool
     */
    public function is_active()
    {
        return filter_var($this->get_setting('enabled', false), FILTER_VALIDATE_BOOLEAN);
    }

    /**
     * Register the frontend JS script.
     * @return string[]
     */
    public function get_payment_method_script_handles()
    {
        $asset_path = IRONIXPAY_PLUGIN_DIR . 'assets/js/blocks-checkout.js';
        $asset_url = IRONIXPAY_PLUGIN_URL . 'assets/js/blocks-checkout.js';
        $version = file_exists($asset_path) ? filemtime($asset_path) : IRONIXPAY_VERSION;

        wp_register_script(
            'ironixpay-blocks-checkout',
            $asset_url,
            array(),
            $version,
            true
        );

        return array('ironixpay-blocks-checkout');
    }

    /**
     * Admin script handles (reuse the same).
     * @return string[]
     */
    public function get_payment_method_script_handles_for_admin()
    {
        return $this->get_payment_method_script_handles();
    }

    /**
     * Data passed to the JS side via wc.wcSettings.getSetting('ironixpay_data').
     * @return array
     */
    public function get_payment_method_data()
    {
        $gateway = $this->get_gateway();

        // Build network compatibility map: which currencies each network supports
        $usdc_unsupported = WC_Gateway_IronixPay::USDC_UNSUPPORTED_NETWORKS;

        return array(
            'title' => $this->get_setting('title', __('Pay with Crypto', 'ironixpay-usdt-gateway')),
            'description' => $this->get_setting('description', __('Pay securely with USDT or USDC stablecoin on your preferred blockchain.', 'ironixpay-usdt-gateway')),
            'supports' => $this->get_supported_features(),
            'logo_url' => IRONIXPAY_PLUGIN_URL . 'assets/images/ironixpay-logo.svg',
            'supported_networks' => $gateway ? $gateway->get_supported_networks() : array('Tron' => 'TRON'),
            'supported_currencies' => $gateway ? $gateway->get_supported_currencies() : array('USDT' => 'USDT (Tether)'),
            'usdc_unsupported_networks' => $usdc_unsupported,
            'environment' => $gateway ? $gateway->get_environment() : 'sandbox',
            'network_icons' => array(
                'Tron' => IRONIXPAY_PLUGIN_URL . 'assets/images/networks/tron.svg',
                'Bsc' => IRONIXPAY_PLUGIN_URL . 'assets/images/networks/bsc.svg',
                'Ethereum' => IRONIXPAY_PLUGIN_URL . 'assets/images/networks/ethereum.svg',
                'Polygon' => IRONIXPAY_PLUGIN_URL . 'assets/images/networks/polygon.svg',
                'Arbitrum' => IRONIXPAY_PLUGIN_URL . 'assets/images/networks/arb.svg',
                'Base' => IRONIXPAY_PLUGIN_URL . 'assets/images/networks/base.svg',
                'Optimism' => IRONIXPAY_PLUGIN_URL . 'assets/images/networks/op.svg',
                'Solana' => IRONIXPAY_PLUGIN_URL . 'assets/images/networks/solana.svg',
            ),
        );
    }

    /**
     * Get the gateway instance lazily.
     * @return WC_Gateway_IronixPay|null
     */
    private function get_gateway()
    {
        if ($this->gateway === null) {
            $gateways = WC()->payment_gateways()->payment_gateways();
            if (isset($gateways['ironixpay'])) {
                $this->gateway = $gateways['ironixpay'];
            }
        }
        return $this->gateway;
    }
}
