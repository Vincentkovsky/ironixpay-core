# Webhooks

IronixPay sends webhook events to notify your server of payment state changes in real time.

## Event Types

### Checkout Session Events

| Event | Description |
|-------|-------------|
| `session.completed` | Payment received in full — status is `Paid` (exact) or `Overpaid` |
| `session.expired` | Session expired — may have partial payment, routed to Resolution Center |
| `session.resolved` | Resolution action completed (Accept / Attach) |
| `session.blocked` | AML risk detected, funds blocked |

### Payout Events

| Event | Description |
|-------|-------------|
| `payout.completed` | Payout confirmed on-chain |
| `payout.failed` | Payout failed — amount refunded to merchant balance |

<!-- sync: backend/src/entity/webhook_events.rs EVENT_SESSION_* / EVENT_PAYOUT_* -->

## Webhook Payload

### Checkout Session

```json
{
  "id": "evt_abc123...",
  "event_type": "session.completed",
  "created": 1739246160,
  "data": {
    "object": "checkout_session",
    "id": "cs_abc123def456",
    "merchant_id": "mer_abc123",
    "amount": "10.5",
    "amount_received": "10.5",
    "fee_amount": "1.05",
    "net_amount": "9.45",
    "currency": "USDT",
    "token_contract": "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
    "network": "TRON",
    "livemode": false,
    "status": "Paid",
    "pay_address": "TQFEyGNzHZAJmebJUvsoZvJghHm2yNhXAD",
    "client_reference_id": "order_20250227_001",
    "pricing": {
      "currency": "USDT",
      "amount": "10.5",
      "exchange_rate": "1.00000000"
    },
    "created_at": 1739246100,
    "paid_at": 1739246160,
    "tx_count": 1,
    "transactions": [
      {
        "tx_hash": "7c72c45f...",
        "amount": "10.5",
        "confirmations": 20,
        "from_address": "TNPeeaaFB7K9cmo4uQpcU32zGK8G1NYqeL",
        "detected_at": 1739246140
      }
    ]
  }
}
```

### Payout

```json
{
  "id": "evt_xyz789...",
  "event_type": "payout.completed",
  "created": 1739246200,
  "data": {
    "object": "payout",
    "id": "po_abc123def456",
    "merchant_id": "mer_abc123",
    "livemode": true,
    "status": "Completed",
    "amount": "5",
    "fee": "1.5",
    "net_amount": "3.5",
    "currency": "USDT",
    "network": "TRON",
    "to_address": "TNPeeaaFB7K9cmo4uQpcU32zGK8G1NYqeL",
    "tx_hash": "7c72c45f...",
    "idempotency_key": "order_123",
    "description": "Vendor payment #1234",
    "metadata": { "internal_ref": "inv-2025-001" },
    "created_at": 1739246100,
    "completed_at": 1739246200
  }
}
```

::: tip Amount Fields
All amount fields (`amount`, `amount_received`, `fee_amount`, `net_amount`) are **human-readable String** type (e.g., `"10.5"` = 10.5 USDT/USDC). No additional conversion is needed.
:::

::: details Checkout Session — Full Field Reference

| Field | Type | Description |
|-------|------|-------------|
| `object` | string | Always `"checkout_session"` |
| `id` | string | Session ID (`cs_` prefix) |
| `merchant_id` | string | Merchant ID |
| `amount` | string | Expected amount (e.g., `"10.5"`) |
| `amount_received` | string | Received amount (e.g., `"10.5"`) |
| `fee_amount` | string? | Platform fee deducted (e.g., `"1.05"`). Only present for `session.completed` / `session.resolved` |
| `net_amount` | string? | Net amount credited to merchant after fee (e.g., `"9.45"`). Only present for `session.completed` / `session.resolved` |
| `currency` | string | Currency (`"USDT"` or `"USDC"`) |
| `token_contract` | string | Token contract address (for verification) |
| `network` | string | Network (`"TRON"`, `"BSC"`, etc.) |
| `livemode` | boolean | `true` = production, `false` = sandbox |
| `status` | string | `"Paid"` / `"Overpaid"` / `"Expired"` |
| `pay_address` | string | Payment address |
| `client_reference_id` | string? | Merchant's custom order reference |
| `created_at` | number | Session creation time (Unix timestamp) |
| `paid_at` | number? | Payment completion time (null for expired) |
| `tx_count` | number | Number of credited transactions |
| `transactions` | array | Full transaction details (see below) |

**transactions[] fields:**

| Field | Type | Description |
|-------|------|-------------|
| `tx_hash` | string | On-chain transaction hash |
| `amount` | string | Transaction amount (e.g., `"10.5"`) |
| `confirmations` | number | Confirmation count |
| `from_address` | string | Sender address (useful for refunds) |
| `detected_at` | number | On-chain detection time (Unix timestamp) |

:::

::: details Payout — Full Field Reference

| Field | Type | Description |
|-------|------|-------------|
| `object` | string | Always `"payout"` |
| `id` | string | Payout ID (`po_` prefix) |
| `merchant_id` | string | Merchant ID |
| `livemode` | boolean | `true` = production, `false` = sandbox |
| `status` | string | `"Completed"` / `"Failed"` |
| `amount` | string | Requested amount (e.g., `"5"`) |
| `fee` | string | Fee charged (e.g., `"1.5"`) |
| `net_amount` | string | Net amount sent = amount - fee |
| `currency` | string | Currency (`"USDT"` or `"USDC"`) |
| `network` | string | Network |
| `to_address` | string | Destination address |
| `tx_hash` | string? | On-chain tx hash (null if failed) |
| `idempotency_key` | string? | Idempotency key |
| `description` | string? | Optional description |
| `metadata` | object? | Custom metadata |
| `error_reason` | string? | Failure reason (null if succeeded) |
| `created_at` | number | Creation time (Unix timestamp) |
| `completed_at` | number? | Completion time (null if not completed) |

:::

## Signature Verification

Every webhook includes two headers:

```http
X-Signature: <hex-encoded HMAC-SHA256>
X-Timestamp: <unix-timestamp>
```

### Algorithm

```
signed_message = "{X-Timestamp}.{request_body_json}"
expected_sig   = HMAC-SHA256(webhook_secret, signed_message)
```

1. Check that `X-Timestamp` is within **5 minutes** of your server time (replay protection)
2. Concatenate `timestamp.body` as the signed message
3. Compute HMAC-SHA256 using your `whsec_...` secret
4. Compare the hex-encoded result against `X-Signature` using a **constant-time** comparison

### Verification Examples

::: code-group

```javascript [Node.js]
const crypto = require('crypto');

function verifyWebhook(payload, signature, timestamp, secret) {
  // 1. Replay protection: reject timestamps older than 5 minutes
  const now = Math.floor(Date.now() / 1000);
  if (Math.abs(now - parseInt(timestamp)) > 300) {
    throw new Error('Timestamp too old — possible replay attack');
  }

  // 2. Reconstruct the signed message
  const message = `${timestamp}.${payload}`;

  // 3. Compute expected signature
  const expected = crypto
    .createHmac('sha256', secret)
    .update(message)
    .digest('hex');

  // 4. Constant-time comparison
  return crypto.timingSafeEqual(
    Buffer.from(signature),
    Buffer.from(expected)
  );
}

// Express middleware example
app.post('/webhook', express.raw({ type: 'application/json' }), (req, res) => {
  const signature = req.headers['x-signature'];
  const timestamp = req.headers['x-timestamp'];
  const payload = req.body.toString();

  if (!verifyWebhook(payload, signature, timestamp, WEBHOOK_SECRET)) {
    return res.status(401).send('Invalid signature');
  }

  const event = JSON.parse(payload);
  // Handle event...
  res.status(200).send('OK');
});
```

```python [Python]
import hmac
import hashlib
import time

def verify_webhook(payload: str, signature: str, timestamp: str, secret: str) -> bool:
    # 1. Replay protection: reject timestamps older than 5 minutes
    now = int(time.time())
    if abs(now - int(timestamp)) > 300:
        raise ValueError("Timestamp too old — possible replay attack")

    # 2. Reconstruct the signed message
    message = f"{timestamp}.{payload}"

    # 3. Compute expected signature
    expected = hmac.new(
        secret.encode("utf-8"),
        message.encode("utf-8"),
        hashlib.sha256,
    ).hexdigest()

    # 4. Constant-time comparison
    return hmac.compare_digest(signature, expected)


# Flask example
from flask import Flask, request, abort

app = Flask(__name__)

@app.route("/webhook", methods=["POST"])
def webhook():
    signature = request.headers.get("X-Signature", "")
    timestamp = request.headers.get("X-Timestamp", "")
    payload = request.get_data(as_text=True)

    if not verify_webhook(payload, signature, timestamp, WEBHOOK_SECRET):
        abort(401)

    event = request.get_json()
    # Handle event...
    return "OK", 200
```

```go [Go]
package main

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"math"
	"net/http"
	"strconv"
	"time"
)

func verifyWebhook(payload, signature, timestamp, secret string) error {
	// 1. Replay protection: reject timestamps older than 5 minutes
	ts, err := strconv.ParseInt(timestamp, 10, 64)
	if err != nil {
		return fmt.Errorf("invalid timestamp")
	}
	if math.Abs(float64(time.Now().Unix()-ts)) > 300 {
		return fmt.Errorf("timestamp too old — possible replay attack")
	}

	// 2. Reconstruct the signed message
	message := timestamp + "." + payload

	// 3. Compute expected signature
	mac := hmac.New(sha256.New, []byte(secret))
	mac.Write([]byte(message))
	expected := hex.EncodeToString(mac.Sum(nil))

	// 4. Constant-time comparison
	if !hmac.Equal([]byte(signature), []byte(expected)) {
		return fmt.Errorf("invalid signature")
	}
	return nil
}

// HTTP handler example
func webhookHandler(w http.ResponseWriter, r *http.Request) {
	signature := r.Header.Get("X-Signature")
	timestamp := r.Header.Get("X-Timestamp")

	body, _ := io.ReadAll(r.Body)
	payload := string(body)

	if err := verifyWebhook(payload, signature, timestamp, webhookSecret); err != nil {
		http.Error(w, "Invalid signature", http.StatusUnauthorized)
		return
	}

	// Handle event...
	w.WriteHeader(http.StatusOK)
}
```

```php [PHP]
<?php

function verifyWebhook(string $payload, string $signature, string $timestamp, string $secret): bool
{
    // 1. Replay protection: reject timestamps older than 5 minutes
    if (abs(time() - intval($timestamp)) > 300) {
        throw new Exception('Timestamp too old — possible replay attack');
    }

    // 2. Reconstruct the signed message
    $message = $timestamp . '.' . $payload;

    // 3. Compute expected signature
    $expected = hash_hmac('sha256', $message, $secret);

    // 4. Constant-time comparison
    return hash_equals($expected, $signature);
}

// Laravel example
Route::post('/webhook', function (Request $request) {
    $payload   = $request->getContent();
    $signature = $request->header('X-Signature', '');
    $timestamp = $request->header('X-Timestamp', '');

    if (!verifyWebhook($payload, $signature, $timestamp, config('services.ironixpay.webhook_secret'))) {
        abort(401, 'Invalid signature');
    }

    $event = json_decode($payload, true);
    // Handle event...
    return response('OK', 200);
});
```

```java [Java]
import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;

public class WebhookVerifier {

    public static boolean verifyWebhook(String payload, String signature, String timestamp, String secret)
            throws Exception {
        // 1. Replay protection: reject timestamps older than 5 minutes
        long now = System.currentTimeMillis() / 1000;
        if (Math.abs(now - Long.parseLong(timestamp)) > 300) {
            throw new SecurityException("Timestamp too old — possible replay attack");
        }

        // 2. Reconstruct the signed message
        String message = timestamp + "." + payload;

        // 3. Compute expected signature
        Mac mac = Mac.getInstance("HmacSHA256");
        mac.init(new SecretKeySpec(secret.getBytes(StandardCharsets.UTF_8), "HmacSHA256"));
        byte[] hash = mac.doFinal(message.getBytes(StandardCharsets.UTF_8));
        String expected = bytesToHex(hash);

        // 4. Constant-time comparison
        return MessageDigest.isEqual(
            expected.getBytes(StandardCharsets.UTF_8),
            signature.getBytes(StandardCharsets.UTF_8)
        );
    }

    private static String bytesToHex(byte[] bytes) {
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) sb.append(String.format("%02x", b));
        return sb.toString();
    }
}

// Spring Boot Controller example
@RestController
public class WebhookController {

    @PostMapping("/webhook")
    public ResponseEntity<String> handleWebhook(
            @RequestBody String payload,
            @RequestHeader("X-Signature") String signature,
            @RequestHeader("X-Timestamp") String timestamp) {

        if (!WebhookVerifier.verifyWebhook(payload, signature, timestamp, webhookSecret)) {
            return ResponseEntity.status(401).body("Invalid signature");
        }

        // Handle event...
        return ResponseEntity.ok("OK");
    }
}
```

:::

::: warning Replay Attack Protection
Always check `X-Timestamp` against your server's current time. If the difference exceeds **5 minutes**, reject the request.
:::

## Configuring Webhooks

Set up your webhook endpoint in the [Merchant Dashboard](https://app.ironixpay.com):

> [View Webhook endpoints in API Reference →](https://api.ironixpay.com/docs#tag/webhooks)

1. Navigate to **Settings → Webhooks**
2. Enter your endpoint URL (must be HTTPS in production)
3. Copy the generated **webhook secret** (`whsec_...`)
4. Select the events you want to receive

## Retry Policy

Failed deliveries are retried with exponential backoff:

| Attempt | Delay |
|---------|-------|
| 1 | Immediate |
| 2 | 15 seconds |
| 3 | 1 minute |
| 4 | 5 minutes |
| 5 | 1 hour |
| 6 | 6 hours |
| 7 | 24 hours |

After 7 failed attempts, the event is marked as `giving_up`. You can view failed deliveries in the Dashboard and manually retry them.

## Best Practices

1. **Return 2xx quickly** — Process webhook logic asynchronously. Return `200` immediately to avoid timeouts.
2. **Handle duplicates** — Use the `id` field to deduplicate events. Webhooks may be delivered more than once.
3. **Verify signatures** — Always verify the `X-Signature` header before trusting the payload.
4. **Use HTTPS** — Webhook endpoints must use HTTPS in production (HTTP is only allowed for `localhost`).
