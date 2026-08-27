<?php
/**
 * IronixPay Webhook Handler
 *
 * Receives and processes webhook callbacks from IronixPay.
 * Verifies HMAC-SHA256 signatures and updates WooCommerce order status.
 */

defined('ABSPATH') || exit;

class IronixPay_Webhook
{

    /** @var WC_Gateway_IronixPay */
    private $gateway;

    /** @var int Timestamp tolerance in seconds (5 minutes) */
    const TIMESTAMP_TOLERANCE = 300;

    /**
     * @param WC_Gateway_IronixPay $gateway  Gateway instance for config access
     */
    public function __construct(WC_Gateway_IronixPay $gateway)
    {
        $this->gateway = $gateway;
    }

    /**
     * Main webhook handler entry point.
     * Called via: /?wc-api=ironixpay_webhook
     */
    public function handle()
    {
        $raw_payload = file_get_contents('php://input');

        if (empty($raw_payload)) {
            $this->respond(400, 'Empty payload');
            return;
        }

        // 1. Verify signature
        $signature = isset($_SERVER['HTTP_X_SIGNATURE']) ? sanitize_text_field(wp_unslash($_SERVER['HTTP_X_SIGNATURE'])) : '';
        $timestamp = isset($_SERVER['HTTP_X_TIMESTAMP']) ? sanitize_text_field(wp_unslash($_SERVER['HTTP_X_TIMESTAMP'])) : '';

        if (empty($signature) || empty($timestamp)) {
            $this->respond(401, 'Missing signature or timestamp headers');
            return;
        }

        // 2. Anti-replay: check timestamp within tolerance
        $ts_int = intval($timestamp);
        if (abs(time() - $ts_int) > self::TIMESTAMP_TOLERANCE) {
            $this->gateway->log('warning', 'Webhook rejected: timestamp too old/future: ' . $timestamp);
            $this->respond(403, 'Timestamp expired');
            return;
        }

        // 3. Verify HMAC-SHA256 signature
        $secret = $this->gateway->get_webhook_secret();
        if (empty($secret)) {
            $this->gateway->log('error', 'Webhook secret not configured');
            $this->respond(500, 'Webhook secret not configured');
            return;
        }

        $expected = hash_hmac('sha256', $timestamp . '.' . $raw_payload, $secret);
        if (!hash_equals($expected, $signature)) {
            $this->gateway->log('warning', 'Webhook rejected: invalid signature');
            $this->respond(403, 'Invalid signature');
            return;
        }

        // 4. Parse payload
        $payload = json_decode($raw_payload, true);
        if (!is_array($payload) || !isset($payload['event_type']) || !isset($payload['data'])) {
            $this->respond(400, 'Invalid payload structure');
            return;
        }

        $event_id = isset($payload['id']) ? sanitize_text_field($payload['id']) : '';
        $event_type = sanitize_text_field($payload['event_type']);
        $data = $payload['data'];

        $this->gateway->log('info', sprintf('Webhook received: %s (event: %s)', $event_type, $event_id));

        // 5. Find the WooCommerce order
        $order = $this->find_order($data);
        if (!$order) {
            $this->gateway->log('warning', 'Webhook: order not found for payload');
            // Return 200 to prevent IronixPay from retrying (order may have been deleted)
            $this->respond(200, 'Order not found, acknowledged');
            return;
        }

        // 6. Idempotency check: prevent double processing
        $processed_key = '_ironixpay_webhook_processed_' . $event_id;
        if ($order->get_meta($processed_key)) {
            $this->gateway->log('info', sprintf('Webhook already processed: %s', $event_id));
            $this->respond(200, 'Already processed');
            return;
        }

        // 7. Process the event
        $this->process_event($order, $event_type, $data, $event_id);

        // 8. Mark as processed
        $order->update_meta_data($processed_key, current_time('mysql'));
        $order->save();

        $this->respond(200, 'OK');
    }

    /**
     * Find the WooCommerce order from webhook data.
     *
     * Priority:
     * 1. data.client_reference_id → parse "wc_order_{id}" → wc_get_order()
     * 2. data.session_id → search by _ironixpay_session_id meta
     *
     * @param array $data  Webhook data object
     * @return WC_Order|null
     */
    private function find_order(array $data): ?WC_Order
    {
        // Method 1: client_reference_id
        if (!empty($data['client_reference_id'])) {
            $ref = $data['client_reference_id'];

            // Parse "wc_order_123" format
            if (preg_match('/^wc_order_(\d+)$/', $ref, $matches)) {
                $order = wc_get_order(intval($matches[1]));
                if ($order instanceof WC_Order) {
                    return $order;
                }
            }
        }

        // Method 2: session ID meta lookup
        // Note: SessionEventPayload uses 'id' as the session ID field (cs_xxx)
        if (!empty($data['id'])) {
            $session_id = sanitize_text_field($data['id']);

            // HPOS-compatible order query
            $orders = wc_get_orders(array(
                'meta_key' => '_ironixpay_session_id',
                'meta_value' => $session_id,
                'limit' => 1,
                'return' => 'objects',
            ));

            if (!empty($orders)) {
                return $orders[0];
            }
        }

        return null;
    }

    /**
     * Process a webhook event and update the order.
     *
     * @param WC_Order $order      WooCommerce order
     * @param string   $event_type Event type (e.g. session.completed)
     * @param array    $data       Event data
     * @param string   $event_id   Unique event ID (evt_xxx)
     */
    private function process_event(WC_Order $order, string $event_type, array $data, string $event_id)
    {
        $status = isset($data['status']) ? $data['status'] : '';

        switch ($event_type) {
            case 'session.completed':
                $this->handle_completed($order, $data, $status);
                break;

            case 'session.expired':
                $this->handle_expired($order, $data);
                break;

            case 'session.blocked':
                $this->handle_blocked($order, $data);
                break;

            case 'session.resolved':
                $this->handle_resolved($order, $data);
                break;

            default:
                $this->gateway->log('info', sprintf('Ignoring unknown event type: %s', $event_type));
                break;
        }
    }

    /**
     * Handle session.completed (Paid or Overpaid).
     */
    private function handle_completed(WC_Order $order, array $data, string $status)
    {
        // Don't update if order is already processing or completed
        if (in_array($order->get_status(), array('processing', 'completed'), true)) {
            $this->gateway->log('info', 'Order already processing/completed, skipping');
            return;
        }

        // Build transaction note
        $note_parts = array();
        $note_parts[] = sprintf(__('Payment confirmed via IronixPay.', 'ironixpay-usdt-gateway'));

        if (!empty($data['network'])) {
            /* translators: %s is the blockchain network name */
            $note_parts[] = sprintf(__('Network: %s', 'ironixpay-usdt-gateway'), $data['network']);
        }

        if (!empty($data['pay_address'])) {
            /* translators: %s is the payment address */
            $note_parts[] = sprintf(__('Pay address: %s', 'ironixpay-usdt-gateway'), $data['pay_address']);
        }

        // Include transaction hashes
        if (!empty($data['transactions']) && is_array($data['transactions'])) {
            $tx_hashes = array();
            foreach ($data['transactions'] as $tx) {
                if (!empty($tx['tx_hash'])) {
                    $tx_hashes[] = $tx['tx_hash'];
                }
            }
            if (!empty($tx_hashes)) {
                /* translators: %s is the transaction hash(es) */
                $note_parts[] = sprintf(__('Transaction(s): %s', 'ironixpay-usdt-gateway'), implode(', ', $tx_hashes));
            }
        }

        // Note overpayment
        // Note: amounts in SessionEventPayload are standard-unit strings (e.g. "10.50")
        if ($status === 'Overpaid') {
            $expected = isset($data['amount']) ? floatval($data['amount']) : 0;
            $received = isset($data['amount_received']) ? floatval($data['amount_received']) : 0;
            $overpaid = $received - $expected;
            $currency = !empty($data['currency']) ? sanitize_text_field($data['currency']) : 'USDT';
            $note_parts[] = sprintf(
                /* translators: 1: overpaid amount, 2: currency */
                __('⚠️ Overpaid by %1$s %2$s.', 'ironixpay-usdt-gateway'),
                number_format($overpaid, 6),
                $currency
            );
        }

        $order->add_order_note(implode("\n", $note_parts));

        // Pass transaction ID to payment_complete: prefer first tx_hash, fallback to session ID
        $transaction_id = '';
        if (!empty($data['transactions']) && is_array($data['transactions']) && !empty($data['transactions'][0]['tx_hash'])) {
            $transaction_id = $data['transactions'][0]['tx_hash'];
        } elseif (!empty($data['id'])) {
            $transaction_id = $data['id'];
        }
        $order->payment_complete($transaction_id);

        $this->gateway->log('info', sprintf('Order #%d marked as processing (status: %s)', $order->get_id(), $status));
    }

    /**
     * Handle session.expired.
     */
    private function handle_expired(WC_Order $order, array $data)
    {
        // Don't downgrade if already paid
        if (in_array($order->get_status(), array('processing', 'completed'), true)) {
            $this->gateway->log('info', 'Order already paid, ignoring expired event');
            return;
        }

        $note = __('IronixPay payment session expired. No payment was received within the time limit.', 'ironixpay-usdt-gateway');

        // Note partial payment if any
        // Note: amounts are standard-unit strings (e.g. "5.25")
        $received = isset($data['amount_received']) ? floatval($data['amount_received']) : 0;
        if ($received > 0) {
            $currency = !empty($data['currency']) ? sanitize_text_field($data['currency']) : 'USDT';
            $note .= "\n" . sprintf(
                /* translators: 1: partial amount, 2: currency */
                __('Partial payment received: %1$s %2$s. This will be handled via the IronixPay Resolution Center.', 'ironixpay-usdt-gateway'),
                number_format($received, 6),
                $currency
            );
        }

        $order->update_status('failed', $note);
        $order->save();

        $this->gateway->log('info', sprintf('Order #%d marked as failed (expired)', $order->get_id()));
    }

    /**
     * Handle session.blocked (AML risk).
     */
    private function handle_blocked(WC_Order $order, array $data)
    {
        $order->update_status('on-hold', __('⚠️ IronixPay: Payment blocked by AML risk check. The funds are held for review. Please check the IronixPay Dashboard for details.', 'ironixpay-usdt-gateway'));
        $order->save();

        $this->gateway->log('warning', sprintf('Order #%d put on hold (AML blocked)', $order->get_id()));
    }

    /**
     * Handle session.resolved (manual resolution via Dashboard).
     */
    private function handle_resolved(WC_Order $order, array $data)
    {
        // Don't update if already processing or completed
        if (in_array($order->get_status(), array('processing', 'completed'), true)) {
            return;
        }

        $status = isset($data['status']) ? $data['status'] : '';
        $note = __('IronixPay: Payment resolved via Resolution Center.', 'ironixpay-usdt-gateway');

        if (!empty($status)) {
            /* translators: %s is the session resolution status */
            $note .= ' ' . sprintf(__('Session status: %s', 'ironixpay-usdt-gateway'), $status);
        }

        $order->add_order_note($note);

        // Only mark as paid for Paid/Overpaid resolutions
        if (in_array($status, array('Paid', 'Overpaid'), true)) {
            $transaction_id = !empty($data['id']) ? $data['id'] : '';
            $order->payment_complete($transaction_id);
            $this->gateway->log('info', sprintf('Order #%d resolved as paid', $order->get_id()));
        } elseif ($status === 'Refunded') {
            $order->update_status('cancelled', __('IronixPay: Payment refunded via Resolution Center.', 'ironixpay-usdt-gateway'));
            $order->save();
            $this->gateway->log('info', sprintf('Order #%d resolved as refunded, marked cancelled', $order->get_id()));
        } else {
            // Unknown resolution status — log but don't change order status
            $this->gateway->log('warning', sprintf('Order #%d resolved with unknown status: %s', $order->get_id(), $status));
        }
    }

    /**
     * Send a JSON response and exit.
     *
     * @param int    $status_code HTTP status code
     * @param string $message     Response message
     */
    private function respond(int $status_code, string $message)
    {
        status_header($status_code);
        header('Content-Type: application/json');
        echo wp_json_encode(array('message' => $message));
        exit;
    }
}
