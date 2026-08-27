<?php
/**
 * IronixPay API Client
 *
 * Handles HTTP communication with the IronixPay REST API.
 * Uses WordPress HTTP API (wp_remote_post / wp_remote_get).
 */

defined('ABSPATH') || exit;

class IronixPay_API
{

    /** @var string API base URL */
    private $base_url;

    /** @var string API key (sk_live_... or sk_test_...) */
    private $api_key;

    /** @var bool Debug mode */
    private $debug;

    /** @var WC_Logger|null */
    private $logger;

    /**
     * @param string $base_url  API base URL (e.g. https://api.ironixpay.com)
     * @param string $api_key   Bearer token
     * @param bool   $debug     Enable request/response logging
     */
    public function __construct(string $base_url, string $api_key, bool $debug = false)
    {
        $this->base_url = rtrim($base_url, '/');
        $this->api_key = $api_key;
        $this->debug = $debug;
        $this->logger = $debug ? wc_get_logger() : null;
    }

    /**
     * Create a checkout session.
     *
     * POST /v1/checkout/sessions
     *
     * @param array $params {
     *     @type string $pricing_amount     Amount in pricing_currency units (e.g. "10.50")
     *     @type string $pricing_currency   Pricing/denomination currency (e.g. "USD", "USDT")
     *     @type string $currency           Settlement token: "USDT" or "USDC"
     *     @type string $network            PascalCase: "Tron", "Bsc", "Ethereum", etc.
     *     @type string $client_reference_id  WooCommerce order reference
     *     @type string $success_url          Redirect URL on success
     *     @type string $cancel_url           Redirect URL on cancel/expiry
     * }
     * @param string|null $idempotency_key  Optional idempotency key
     * @return array|WP_Error  Parsed SessionResponse or WP_Error
     */
    public function create_session(array $params, ?string $idempotency_key = null)
    {
        $headers = array(
            'Authorization' => 'Bearer ' . $this->api_key,
            'Content-Type' => 'application/json',
        );

        if ($idempotency_key) {
            $headers['Idempotency-Key'] = $idempotency_key;
        }

        return $this->request('POST', '/v1/checkout/sessions', $params, $headers);
    }

    /**
     * Retrieve a checkout session by ID.
     *
     * GET /v1/checkout/sessions/{id}
     *
     * @param string $session_id  Session ID (cs_xxx)
     * @return array|WP_Error  Parsed SessionResponse or WP_Error
     */
    public function get_session(string $session_id)
    {
        $headers = array(
            'Authorization' => 'Bearer ' . $this->api_key,
        );

        return $this->request('GET', '/v1/checkout/sessions/' . $session_id, null, $headers);
    }

    /**
     * Perform an HTTP request to the IronixPay API.
     *
     * @param string     $method   HTTP method (GET, POST)
     * @param string     $path     API path (e.g. /v1/checkout/sessions)
     * @param array|null $body     Request body (JSON-encoded for POST)
     * @param array      $headers  HTTP headers
     * @return array|WP_Error
     */
    private function request(string $method, string $path, ?array $body, array $headers)
    {
        $url = $this->base_url . $path;

        $args = array(
            'method' => $method,
            'headers' => $headers,
            'timeout' => 30,
        );

        if ($body !== null && $method === 'POST') {
            $args['body'] = wp_json_encode($body);
        }

        $this->log('info', sprintf('API %s %s', $method, $url), array('body' => $body));

        $response = wp_remote_request($url, $args);

        if (is_wp_error($response)) {
            $this->log('error', 'API request failed: ' . $response->get_error_message());
            return $response;
        }

        $status_code = wp_remote_retrieve_response_code($response);
        $raw_body = wp_remote_retrieve_body($response);
        $parsed = json_decode($raw_body, true);

        $this->log('info', sprintf('API response %d', $status_code), array('body' => $raw_body));

        // Check for API error response: { "error": { "type": ..., "code": ..., "message": ... } }
        if ($status_code < 200 || $status_code >= 300) {
            $error_message = 'IronixPay API error';
            $error_code = 'ironixpay_api_error';

            if (is_array($parsed) && isset($parsed['error'])) {
                $err = $parsed['error'];
                $error_message = isset($err['message']) ? $err['message'] : $error_message;
                $error_code = isset($err['code']) ? $err['code'] : $error_code;
            }

            return new WP_Error($error_code, $error_message, array(
                'status' => $status_code,
                'response' => $parsed,
            ));
        }

        return $parsed;
    }

    /**
     * Log a message to WooCommerce logger.
     *
     * @param string $level   Log level (info, error, debug)
     * @param string $message Log message
     * @param array  $context Additional context
     */
    private function log(string $level, string $message, array $context = array())
    {
        if (!$this->debug || !$this->logger) {
            return;
        }

        // Redact API key from logs
        $safe_message = str_replace($this->api_key, 'sk_***', $message);

        $this->logger->log($level, '[IronixPay] ' . $safe_message, array(
            'source' => 'ironixpay-usdt-gateway',
        ));
    }
}
