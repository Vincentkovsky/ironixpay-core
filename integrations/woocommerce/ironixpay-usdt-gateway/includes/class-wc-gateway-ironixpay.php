<?php
/**
 * IronixPay WooCommerce Payment Gateway
 *
 * Extends WC_Payment_Gateway to provide USDT/USDC cryptocurrency payment
 * via the IronixPay Redirect Checkout flow.
 */

defined('ABSPATH') || exit;

class WC_Gateway_IronixPay extends WC_Payment_Gateway
{

    /** @var string Environment: 'production' or 'sandbox' */
    private $environment;

    /** @var bool Debug mode */
    private $debug_mode;

    /**
     * Available networks with their display labels.
     * Keys must match Rust Network enum values (PascalCase).
     */
    const NETWORKS = array(
        'Tron' => 'TRON',
        'Bsc' => 'BSC (BNB Chain)',
        'Ethereum' => 'Ethereum',
        'Polygon' => 'Polygon',
        'Arbitrum' => 'Arbitrum',
        'Base' => 'Base',
        'Optimism' => 'Optimism',
        'Solana' => 'Solana',
    );

    /**
     * Networks that do NOT support USDC.
     */
    const USDC_UNSUPPORTED_NETWORKS = array('Tron');

    /**
     * Available settlement currencies.
     */
    const CURRENCIES = array(
        'USDT' => 'USDT (Tether)',
        'USDC' => 'USDC (Circle)',
    );

    /**
     * Fiat currencies supported by the IronixPay pricing API.
     * If the WooCommerce store currency is not in this list (and not USDT/USDC),
     * the gateway will be hidden at checkout.
     */
    const SUPPORTED_PRICING_CURRENCIES = array(
        'USD', 'CNY', 'EUR', 'GBP', 'JPY', 'KRW', 'SGD', 'HKD', 'TWD', 'RUB',
        'USDT', 'USDC',
    );

    public function __construct()
    {
        $this->id = 'ironixpay';
        $this->icon = IRONIXPAY_PLUGIN_URL . 'assets/images/ironixpay-logo.svg';
        $this->has_fields = true; // Need to show network/currency selector at checkout
        $this->method_title = __('IronixPay - Crypto Payment', 'ironixpay-usdt-gateway');
        $this->method_description = __('Accept USDT & USDC payments on 8 blockchains via IronixPay. Supports fiat pricing (USD, EUR, CNY, GBP, JPY, KRW, SGD, HKD, TWD, RUB) with automatic crypto conversion. Customers are redirected to a hosted checkout page.', 'ironixpay-usdt-gateway');
        $this->supports = array('products');

        // Load settings
        $this->init_form_fields();
        $this->init_settings();

        $this->title = $this->get_option('title', __('Pay with Crypto', 'ironixpay-usdt-gateway'));
        $this->description = $this->get_option('description', __('Pay securely with USDT or USDC stablecoin on your preferred blockchain.', 'ironixpay-usdt-gateway'));
        $this->environment = $this->get_option('environment', 'sandbox');
        $this->debug_mode = 'yes' === $this->get_option('debug', 'no');

        // Save settings hook
        add_action('woocommerce_update_options_payment_gateways_' . $this->id, array($this, 'process_admin_options'));
    }

    /**
     * Admin settings form fields.
     */
    public function init_form_fields()
    {
        $this->form_fields = array(
            'enabled' => array(
                'title' => __('Enable/Disable', 'ironixpay-usdt-gateway'),
                'type' => 'checkbox',
                'label' => __('Enable IronixPay crypto payments', 'ironixpay-usdt-gateway'),
                'default' => 'no',
            ),
            'title' => array(
                'title' => __('Title', 'ironixpay-usdt-gateway'),
                'type' => 'text',
                'description' => __('Payment method title displayed at checkout.', 'ironixpay-usdt-gateway'),
                'default' => __('Pay with Crypto', 'ironixpay-usdt-gateway'),
                'desc_tip' => true,
            ),
            'description' => array(
                'title' => __('Description', 'ironixpay-usdt-gateway'),
                'type' => 'textarea',
                'description' => __('Payment method description displayed at checkout.', 'ironixpay-usdt-gateway'),
                'default' => __('Pay securely with USDT or USDC stablecoin on your preferred blockchain.', 'ironixpay-usdt-gateway'),
                'desc_tip' => true,
            ),
            'environment' => array(
                'title' => __('Environment', 'ironixpay-usdt-gateway'),
                'type' => 'select',
                'description' => __('Select Sandbox for testing, Production for live payments.', 'ironixpay-usdt-gateway'),
                'default' => 'sandbox',
                'options' => array(
                    'sandbox' => __('Sandbox (Test Mode)', 'ironixpay-usdt-gateway'),
                    'production' => __('Production (Live)', 'ironixpay-usdt-gateway'),
                ),
                'desc_tip' => true,
            ),
            'api_key_live' => array(
                'title' => __('Live API Key', 'ironixpay-usdt-gateway'),
                'type' => 'password',
                'description' => __('Your production API key (sk_live_...). Get it from the IronixPay Dashboard.', 'ironixpay-usdt-gateway'),
                'default' => '',
                'desc_tip' => true,
            ),
            'api_key_test' => array(
                'title' => __('Test API Key', 'ironixpay-usdt-gateway'),
                'type' => 'password',
                'description' => __('Your sandbox API key (sk_test_...). Get it from the IronixPay Dashboard.', 'ironixpay-usdt-gateway'),
                'default' => '',
                'desc_tip' => true,
            ),
            'webhook_secret_live' => array(
                'title' => __('Live Webhook Secret', 'ironixpay-usdt-gateway'),
                'type' => 'password',
                'description' => __('Your production webhook signing secret.', 'ironixpay-usdt-gateway'),
                'default' => '',
                'desc_tip' => true,
            ),
            'webhook_secret_test' => array(
                'title' => __('Test Webhook Secret', 'ironixpay-usdt-gateway'),
                'type' => 'password',
                'description' => __('Your sandbox webhook signing secret.', 'ironixpay-usdt-gateway'),
                'default' => '',
                'desc_tip' => true,
            ),
            'webhook_url_info' => array(
                'title' => __('Webhook URL', 'ironixpay-usdt-gateway'),
                'type' => 'title',
                'description' => sprintf(
                    /* translators: %s is the webhook URL */
                    __('Configure this URL in your IronixPay Dashboard → Webhook settings: %s', 'ironixpay-usdt-gateway'),
                    '<br><code>' . esc_url(home_url('/?wc-api=ironixpay_webhook')) . '</code>'
                ),
            ),
            'supported_currencies' => array(
                'title' => __('Supported Currencies', 'ironixpay-usdt-gateway'),
                'type' => 'multiselect',
                'description' => __('Select which stablecoins to accept. USDC is not available on TRON or in Sandbox mode.', 'ironixpay-usdt-gateway'),
                'default' => array('USDT'),
                'options' => self::CURRENCIES,
                'desc_tip' => true,
                'css' => 'min-height: 60px;',
            ),
            'supported_networks' => array(
                'title' => __('Supported Networks', 'ironixpay-usdt-gateway'),
                'type' => 'multiselect',
                'description' => __('Select which blockchain networks to offer at checkout. Customers can choose from these options. Note: Sandbox mode only supports TRON (Nile testnet). USDC is not available on TRON.', 'ironixpay-usdt-gateway'),
                'default' => array('Tron'),
                'options' => self::NETWORKS,
                'desc_tip' => true,
                'css' => 'min-height: 120px;',
            ),
            'debug' => array(
                'title' => __('Debug Log', 'ironixpay-usdt-gateway'),
                'type' => 'checkbox',
                'label' => __('Enable debug logging', 'ironixpay-usdt-gateway'),
                'description' => sprintf(
                    /* translators: %s is the log file path */
                    __('Log API requests and responses. View logs at %s.', 'ironixpay-usdt-gateway'),
                    '<a href="' . admin_url('admin.php?page=wc-status&tab=logs') . '">WooCommerce → Status → Logs</a>'
                ),
                'default' => 'no',
            ),
        );
    }

    /**
     * Only show this gateway if the store currency is supported by IronixPay.
     *
     * @return bool
     */
    public function is_available()
    {
        if (!parent::is_available()) {
            return false;
        }

        $store_currency = get_woocommerce_currency();
        if (!in_array($store_currency, self::SUPPORTED_PRICING_CURRENCIES, true)) {
            return false;
        }

        return true;
    }

    /**
     * Render payment fields at checkout (currency + network selector).
     */
    public function payment_fields()
    {
        // Display description
        if ($this->description) {
            echo '<p>' . wp_kses_post($this->description) . '</p>';
        }

        $currencies = $this->get_supported_currencies();
        $networks = $this->get_supported_networks();

        // --- Currency selector ---
        if (count($currencies) === 1) {
            // Single currency: hidden input
            $currency_key = array_keys($currencies)[0];
            echo '<input type="hidden" name="ironixpay_currency" value="' . esc_attr($currency_key) . '">';
        } else {
            // Multiple currencies: show selector
            echo '<div class="ironixpay-currency-selector" style="margin-bottom: 12px;">';
            echo '<label for="ironixpay_currency">' . esc_html__('Select Currency', 'ironixpay-usdt-gateway') . ' <span class="required">*</span></label>';
            echo '<select name="ironixpay_currency" id="ironixpay_currency" class="select" required>';
            echo '<option value="">' . esc_html__('— Choose a currency —', 'ironixpay-usdt-gateway') . '</option>';
            foreach ($currencies as $value => $label) {
                echo '<option value="' . esc_attr($value) . '">' . esc_html($label) . '</option>';
            }
            echo '</select>';
            echo '<p class="ironixpay-currency-note" style="font-size:12px;color:#6b7280;margin:4px 0 0;">'
                . esc_html__('Note: USDC is not available on TRON network.', 'ironixpay-usdt-gateway')
                . '</p>';
            echo '</div>';
        }

        // --- Network selector ---
        if (count($networks) === 1) {
            $key = array_keys($networks)[0];
            echo '<input type="hidden" name="ironixpay_network" value="' . esc_attr($key) . '">';
            echo '<p class="ironixpay-single-network">'
                . wp_kses(
                    sprintf(
                        /* translators: %s is the network name */
                        __('Payment will be processed on %s', 'ironixpay-usdt-gateway'),
                        '<strong>' . esc_html($networks[$key]) . '</strong>'
                    ),
                    array('strong' => array())
                )
                . '</p>';
            return;
        }

        // Multiple networks: show selector
        echo '<div class="ironixpay-network-selector">';
        echo '<label for="ironixpay_network">' . esc_html__('Select Blockchain Network', 'ironixpay-usdt-gateway') . ' <span class="required">*</span></label>';
        echo '<select name="ironixpay_network" id="ironixpay_network" class="select" required>';
        echo '<option value="">' . esc_html__('— Choose a network —', 'ironixpay-usdt-gateway') . '</option>';

        foreach ($networks as $value => $label) {
            // Mark TRON with note if USDC is also enabled
            $suffix = '';
            if (in_array($value, self::USDC_UNSUPPORTED_NETWORKS) && count($currencies) > 1) {
                $suffix = ' (USDT only)';
            }
            echo '<option value="' . esc_attr($value) . '">' . esc_html($label . $suffix) . '</option>';
        }

        echo '</select>';
        echo '</div>';
    }

    /**
     * Validate checkout fields.
     *
     * @return bool
     */
    public function validate_fields()
    {
        $currency = $this->get_posted_currency();
        $network = $this->get_posted_network();

        // Validate currency
        if (empty($currency)) {
            wc_add_notice(__('Please select a payment currency.', 'ironixpay-usdt-gateway'), 'error');
            return false;
        }

        $supported_currencies = $this->get_supported_currencies();
        if (!isset($supported_currencies[$currency])) {
            wc_add_notice(__('The selected payment currency is not available.', 'ironixpay-usdt-gateway'), 'error');
            return false;
        }

        // Validate network
        if (empty($network)) {
            wc_add_notice(__('Please select a payment network.', 'ironixpay-usdt-gateway'), 'error');
            return false;
        }

        $supported = $this->get_supported_networks();
        if (!isset($supported[$network])) {
            wc_add_notice(__('The selected payment network is not available.', 'ironixpay-usdt-gateway'), 'error');
            return false;
        }

        // Validate USDC + TRON incompatibility
        if ($currency === 'USDC' && in_array($network, self::USDC_UNSUPPORTED_NETWORKS)) {
            wc_add_notice(__('USDC is not supported on the TRON network. Please choose a different network or use USDT.', 'ironixpay-usdt-gateway'), 'error');
            return false;
        }

        // Validate USDC + Sandbox incompatibility
        if ($currency === 'USDC' && $this->environment === 'sandbox') {
            wc_add_notice(__('USDC is not available in Sandbox mode. Please use USDT for testing.', 'ironixpay-usdt-gateway'), 'error');
            return false;
        }

        return true;
    }

    /**
     * Get the selected currency from checkout form.
     *
     * @return string
     */
    private function get_posted_currency()
    {
        // phpcs:ignore WordPress.Security.NonceVerification.Missing -- Nonce verified by WooCommerce checkout handler
        if (!empty($_POST['ironixpay_currency'])) {
            // phpcs:ignore WordPress.Security.NonceVerification.Missing
            return sanitize_text_field(wp_unslash($_POST['ironixpay_currency']));
        }

        // Fallback: Block checkout REST API
        $value = $this->get_block_payment_data('ironixpay_currency');
        if (!empty($value)) {
            return $value;
        }

        // If only one currency configured, use it as default
        $currencies = $this->get_supported_currencies();
        if (count($currencies) === 1) {
            return array_keys($currencies)[0];
        }

        return '';
    }

    /**
     * Get the selected network from either Classic or Block checkout.
     *
     * @return string
     */
    private function get_posted_network()
    {
        // phpcs:ignore WordPress.Security.NonceVerification.Missing -- Nonce verified by WooCommerce checkout handler
        if (!empty($_POST['ironixpay_network'])) {
            // phpcs:ignore WordPress.Security.NonceVerification.Missing
            return sanitize_text_field(wp_unslash($_POST['ironixpay_network']));
        }

        // Fallback: Block checkout REST API
        return $this->get_block_payment_data('ironixpay_network');
    }

    /**
     * Extract a value from Block checkout's payment_data in the JSON request body.
     *
     * Uses a static cache because php://input can only be read once per request.
     *
     * @param string $key  The payment_data key to extract
     * @return string      The sanitized value, or empty string if not found
     */
    private function get_block_payment_data(string $key): string
    {
        static $payment_data = null;

        if ($payment_data === null) {
            $payment_data = array();
            $raw_body = file_get_contents('php://input');
            if (!empty($raw_body)) {
                $body = json_decode($raw_body, true);
                if (isset($body['payment_data']) && is_array($body['payment_data'])) {
                    foreach ($body['payment_data'] as $item) {
                        if (isset($item['key'], $item['value'])) {
                            $payment_data[$item['key']] = sanitize_text_field($item['value']);
                        }
                    }
                }
            }
        }

        return isset($payment_data[$key]) ? $payment_data[$key] : '';
    }

    /**
     * Process the payment: create an IronixPay session and redirect.
     *
     * @param int $order_id WooCommerce order ID
     * @return array
     */
    public function process_payment($order_id)
    {
        $order = wc_get_order($order_id);
        $network = $this->get_posted_network();
        $currency = $this->get_posted_currency();

        if (empty($network)) {
            wc_add_notice(__('Please select a payment network.', 'ironixpay-usdt-gateway'), 'error');
            return array('result' => 'failure');
        }

        if (empty($currency)) {
            $currency = 'USDT'; // Safe fallback
        }

        // Format order total as a decimal string.
        // Use WooCommerce's configured decimal count (e.g. 2 for USD, 0 for JPY).
        $decimals = wc_get_price_decimals();
        $amount_str = number_format($order->get_total(), $decimals, '.', '');

        // Minimum amount validation is handled by the IronixPay API,
        // which knows the actual crypto amount after fiat conversion.

        // Validate store currency is supported for pricing
        $store_currency = get_woocommerce_currency();
        if (!in_array($store_currency, self::SUPPORTED_PRICING_CURRENCIES, true)) {
            wc_add_notice(
                sprintf(
                    /* translators: %s is the unsupported currency code */
                    __('IronixPay does not support %s as a pricing currency.', 'ironixpay-usdt-gateway'),
                    $store_currency
                ),
                'error'
            );
            return array('result' => 'failure');
        }

        // Build success/cancel URLs
        $success_url = $this->get_return_url($order);
        $cancel_url = $order->get_cancel_order_url_raw();

        // Create IronixPay session
        $api = $this->get_api_client();

        $params = array(
            'pricing_amount' => $amount_str,
            'pricing_currency' => get_woocommerce_currency(),
            'currency' => $currency,
            'network' => $network,
            'client_reference_id' => $this->get_client_reference_id($order),
            'success_url' => $success_url,
            'cancel_url' => $cancel_url,
        );

        // Use order ID as idempotency key (deterministic, safe for retries)
        $idempotency_key = 'wc_order_' . $order->get_id() . '_' . $order->get_order_key();

        $result = $api->create_session($params, $idempotency_key);

        if (is_wp_error($result)) {
            $this->log('error', 'Failed to create session: ' . $result->get_error_message());
            wc_add_notice(
                sprintf(
                    /* translators: %s is the error message */
                    __('Payment error: %s', 'ironixpay-usdt-gateway'),
                    $result->get_error_message()
                ),
                'error'
            );
            return array('result' => 'failure');
        }

        // Store session metadata on the order (HPOS-compatible)
        $order->update_meta_data('_ironixpay_session_id', sanitize_text_field($result['id']));
        $order->update_meta_data('_ironixpay_pay_address', sanitize_text_field($result['pay_address']));
        $order->update_meta_data('_ironixpay_network', sanitize_text_field($network));
        $order->update_meta_data('_ironixpay_currency', sanitize_text_field($currency));
        $order->update_meta_data('_ironixpay_session_url', esc_url_raw($result['url']));

        // Add order note
        $order->add_order_note(sprintf(
            /* translators: 1: session ID, 2: network name, 3: pay address, 4: currency */
            __('IronixPay session created: %1$s on %2$s (%4$s). Pay address: %3$s', 'ironixpay-usdt-gateway'),
            $result['id'],
            $network,
            $result['pay_address'],
            $currency
        ));

        // Mark as pending payment
        $order->update_status('pending', sprintf(
            /* translators: %s is the currency */
            __('Awaiting %s payment via IronixPay.', 'ironixpay-usdt-gateway'),
            $currency
        ));
        $order->save();

        // Empty the cart
        WC()->cart->empty_cart();

        // Redirect to hosted checkout page
        return array(
            'result' => 'success',
            'redirect' => $result['url'],
        );
    }

    // ========== Helper Methods ==========

    /**
     * Get the API client instance.
     *
     * @return IronixPay_API
     */
    public function get_api_client(): IronixPay_API
    {
        return new IronixPay_API(
            $this->get_api_base_url(),
            $this->get_api_key(),
            $this->debug_mode
        );
    }

    /**
     * Get the active API key based on environment.
     *
     * @return string
     */
    public function get_api_key(): string
    {
        return $this->environment === 'production'
            ? $this->get_option('api_key_live', '')
            : $this->get_option('api_key_test', '');
    }

    /**
     * Get the active webhook secret based on environment.
     *
     * @return string
     */
    public function get_webhook_secret(): string
    {
        return $this->environment === 'production'
            ? $this->get_option('webhook_secret_live', '')
            : $this->get_option('webhook_secret_test', '');
    }

    /**
     * Get the API base URL based on environment.
     *
     * @return string
     */
    public function get_api_base_url(): string
    {
        return $this->environment === 'production'
            ? 'https://api.ironixpay.com'
            : 'https://sandbox.ironixpay.com';
    }

    /**
     * Get the current environment.
     *
     * @return string 'production' or 'sandbox'
     */
    public function get_environment(): string
    {
        return $this->environment;
    }

    /**
     * Get the supported currencies as configured by the merchant.
     *
     * @return array Associative array: currency_code => display_label
     */
    public function get_supported_currencies(): array
    {
        // Sandbox only supports USDT
        if ($this->environment === 'sandbox') {
            return array('USDT' => self::CURRENCIES['USDT']);
        }

        $configured = $this->get_option('supported_currencies', array('USDT'));

        if (!is_array($configured) || empty($configured)) {
            return array('USDT' => self::CURRENCIES['USDT']);
        }

        $result = array();
        foreach ($configured as $key) {
            if (isset(self::CURRENCIES[$key])) {
                $result[$key] = self::CURRENCIES[$key];
            }
        }

        return empty($result) ? array('USDT' => self::CURRENCIES['USDT']) : $result;
    }

    /**
     * Get the supported networks as configured by the merchant.
     *
     * @return array  Associative array: enum_value => display_label
     */
    public function get_supported_networks(): array
    {
        // Sandbox only supports TRON (Nile testnet)
        if ($this->environment === 'sandbox') {
            return array('Tron' => self::NETWORKS['Tron']);
        }

        $configured = $this->get_option('supported_networks', array('Tron'));

        if (!is_array($configured) || empty($configured)) {
            return array('Tron' => self::NETWORKS['Tron']);
        }

        $result = array();
        foreach ($configured as $key) {
            if (isset(self::NETWORKS[$key])) {
                $result[$key] = self::NETWORKS[$key];
            }
        }

        return empty($result) ? array('Tron' => self::NETWORKS['Tron']) : $result;
    }

    /**
     * Get networks that support a specific currency.
     *
     * @param string $currency  Currency code (USDT or USDC)
     * @return array
     */
    public function get_networks_for_currency(string $currency): array
    {
        $networks = $this->get_supported_networks();

        if ($currency === 'USDC') {
            // Filter out networks that don't support USDC
            foreach (self::USDC_UNSUPPORTED_NETWORKS as $unsupported) {
                unset($networks[$unsupported]);
            }
        }

        return $networks;
    }

    /**
     * Generate a client_reference_id for the order.
     *
     * @param WC_Order $order
     * @return string
     */
    private function get_client_reference_id(WC_Order $order): string
    {
        return 'wc_order_' . $order->get_id();
    }

    /**
     * Log a message (when debug mode is enabled).
     *
     * @param string $level   Log level
     * @param string $message Message
     */
    public function log(string $level, string $message)
    {
        if ($this->debug_mode) {
            $logger = wc_get_logger();
            $logger->log($level, '[IronixPay] ' . $message, array('source' => 'ironixpay-usdt-gateway'));
        }
    }
}
