# Webhooks

IronixPay 通过 Webhook 实时推送支付状态变更。

## 事件类型

### Checkout Session 事件

| 事件 | 说明 |
|------|------|
| `session.completed` | 全额到账 — payload 中 `status` 为 `Paid` 或 `Overpaid` |
| `session.expired` | Session 超时 — 若有部分到账，转入 Resolution Center |
| `session.resolved` | Resolution 操作完成（Accept / Attach） |
| `session.blocked` | AML 命中风险，资金被冻结 |

### Payout 事件

| 事件 | 说明 |
|------|------|
| `payout.completed` | 出金成功 — 链上交易已确认 |
| `payout.failed` | 出金失败 — 金额已原路退回余额 |

<!-- sync: backend/src/entity/webhook_events.rs EVENT_SESSION_* / EVENT_PAYOUT_* -->

## Payload 结构

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

::: tip 金额字段
所有金额字段（`amount`、`amount_received`、`fee_amount`、`net_amount`）均为**人类可读的 String 类型**（如 `"10.5"` = 10.5 USDT/USDC），无需额外换算。
:::

::: details Checkout Session 完整字段说明

| 字段 | 类型 | 说明 |
|------|------|------|
| `object` | string | 固定值 `"checkout_session"` |
| `id` | string | Session ID（`cs_` 前缀） |
| `merchant_id` | string | 商户 ID |
| `amount` | string | 预期金额（如 `"10.5"`） |
| `amount_received` | string | 实收金额（如 `"10.5"`） |
| `fee_amount` | string? | 平台手续费（如 `"1.05"`）。仅在 `session.completed` / `session.resolved` 时出现 |
| `net_amount` | string? | 扣除手续费后商户实收金额（如 `"9.45"`）。仅在 `session.completed` / `session.resolved` 时出现 |
| `currency` | string | 币种（`"USDT"` 或 `"USDC"`） |
| `token_contract` | string | 代币合约地址（可用于验证真实性） |
| `network` | string | 网络（`"TRON"`、`"BSC"` 等） |
| `livemode` | boolean | `true` = 生产，`false` = 沙盒 |
| `status` | string | `"Paid"` / `"Overpaid"` / `"Expired"` |
| `pay_address` | string | 收款地址 |
| `client_reference_id` | string? | 商户自定义订单号 |
| `pricing` | object | 定价快照（见下方） |
| `pricing.currency` | string | 定价币种（`"USDT"` / `"USD"` 等） |
| `pricing.amount` | string | 原始定价金额（如 `"10.50"`） |
| `pricing.exchange_rate` | string | 创建时汇率（加密直接定价为 `"1.00000000"`） |
| `created_at` | number | Session 创建时间（Unix 时间戳） |
| `paid_at` | number? | 支付完成时间（过期时为空） |
| `tx_count` | number | 关联交易笔数 |
| `transactions` | array | 完整交易明细（见下方） |

**transactions[] 字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `tx_hash` | string | 链上交易哈希 |
| `amount` | string | 该笔交易金额（如 `"10.5"`） |
| `confirmations` | number | 确认数 |
| `from_address` | string | 付款来源地址（可用于退款） |
| `detected_at` | number | 链上检测时间（Unix 时间戳） |

:::

::: details Payout 完整字段说明

| 字段 | 类型 | 说明 |
|------|------|------|
| `object` | string | 固定值 `"payout"` |
| `id` | string | Payout ID（`po_` 前缀） |
| `merchant_id` | string | 商户 ID |
| `livemode` | boolean | `true` = 生产，`false` = 沙盒 |
| `status` | string | `"Completed"` / `"Failed"` |
| `amount` | string | 原始请求金额（如 `"5"`） |
| `fee` | string | 手续费（如 `"1.5"`） |
| `net_amount` | string | 实际出金金额 = amount - fee |
| `currency` | string | 币种（`"USDT"` 或 `"USDC"`） |
| `network` | string | 网络 |
| `to_address` | string | 目标地址 |
| `tx_hash` | string? | 链上交易哈希（失败时为空） |
| `idempotency_key` | string? | 幂等键 |
| `description` | string? | 备注说明 |
| `metadata` | object? | 自定义元数据 |
| `error_reason` | string? | 失败原因（成功时为空） |
| `created_at` | number | 创建时间（Unix 时间戳） |
| `completed_at` | number? | 完成时间（未完成时为空） |

:::

## 签名验证

每个 Webhook 请求带两个 Header：

```http
X-Signature: <hex-encoded HMAC-SHA256>
X-Timestamp: <Unix timestamp>
```

### 验证算法

```
签名原文 = "{X-Timestamp}.{请求体 JSON}"
期望签名 = HMAC-SHA256(Webhook Secret, 签名原文)
```

1. 检查 `X-Timestamp` 距当前时间不超过 **5 分钟**（防重放攻击）
2. 拼接 `timestamp.body` 作为签名原文
3. 使用 `whsec_...` 密钥计算 HMAC-SHA256
4. 将计算结果（hex 编码）与 `X-Signature` 做**常量时间比较**

### 验签示例

::: code-group

```javascript [Node.js]
const crypto = require('crypto');

function verifyWebhook(payload, signature, timestamp, secret) {
  // 1. 防重放：时间戳 5 分钟有效期
  const now = Math.floor(Date.now() / 1000);
  if (Math.abs(now - parseInt(timestamp)) > 300) {
    throw new Error('Timestamp expired — possible replay attack');
  }

  // 2. 构造签名原文
  const message = `${timestamp}.${payload}`;

  // 3. 计算期望签名
  const expected = crypto
    .createHmac('sha256', secret)
    .update(message)
    .digest('hex');

  // 4. Constant-time 比较
  return crypto.timingSafeEqual(
    Buffer.from(signature),
    Buffer.from(expected)
  );
}

// Express 中间件用法
app.post('/webhook', express.raw({ type: 'application/json' }), (req, res) => {
  const signature = req.headers['x-signature'];
  const timestamp = req.headers['x-timestamp'];
  const payload = req.body.toString();

  if (!verifyWebhook(payload, signature, timestamp, WEBHOOK_SECRET)) {
    return res.status(401).send('Invalid signature');
  }

  const event = JSON.parse(payload);
  // 处理事件...
  res.status(200).send('OK');
});
```

```python [Python]
import hmac
import hashlib
import time

def verify_webhook(payload: str, signature: str, timestamp: str, secret: str) -> bool:
    # 1. 防重放：时间戳 5 分钟有效期
    now = int(time.time())
    if abs(now - int(timestamp)) > 300:
        raise ValueError("Timestamp expired — possible replay attack")

    # 2. 构造签名原文
    message = f"{timestamp}.{payload}"

    # 3. 计算期望签名
    expected = hmac.new(
        secret.encode("utf-8"),
        message.encode("utf-8"),
        hashlib.sha256,
    ).hexdigest()

    # 4. Constant-time 比较
    return hmac.compare_digest(signature, expected)


# Flask 用法
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
    # 处理事件...
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
	// 1. 防重放：时间戳 5 分钟有效期
	ts, err := strconv.ParseInt(timestamp, 10, 64)
	if err != nil {
		return fmt.Errorf("invalid timestamp")
	}
	if math.Abs(float64(time.Now().Unix()-ts)) > 300 {
		return fmt.Errorf("timestamp expired — possible replay attack")
	}

	// 2. 构造签名原文
	message := timestamp + "." + payload

	// 3. 计算期望签名
	mac := hmac.New(sha256.New, []byte(secret))
	mac.Write([]byte(message))
	expected := hex.EncodeToString(mac.Sum(nil))

	// 4. Constant-time 比较
	if !hmac.Equal([]byte(signature), []byte(expected)) {
		return fmt.Errorf("invalid signature")
	}
	return nil
}

// HTTP handler 用法
func webhookHandler(w http.ResponseWriter, r *http.Request) {
	signature := r.Header.Get("X-Signature")
	timestamp := r.Header.Get("X-Timestamp")

	body, _ := io.ReadAll(r.Body)
	payload := string(body)

	if err := verifyWebhook(payload, signature, timestamp, webhookSecret); err != nil {
		http.Error(w, "Invalid signature", http.StatusUnauthorized)
		return
	}

	// 处理事件...
	w.WriteHeader(http.StatusOK)
}
```

```php [PHP]
<?php

function verifyWebhook(string $payload, string $signature, string $timestamp, string $secret): bool
{
    // 1. 防重放：时间戳 5 分钟有效期
    if (abs(time() - intval($timestamp)) > 300) {
        throw new Exception('Timestamp expired — possible replay attack');
    }

    // 2. 构造签名原文
    $message = $timestamp . '.' . $payload;

    // 3. 计算期望签名
    $expected = hash_hmac('sha256', $message, $secret);

    // 4. Constant-time 比较
    return hash_equals($expected, $signature);
}

// Laravel 用法
Route::post('/webhook', function (Request $request) {
    $payload   = $request->getContent();
    $signature = $request->header('X-Signature', '');
    $timestamp = $request->header('X-Timestamp', '');

    if (!verifyWebhook($payload, $signature, $timestamp, config('services.ironixpay.webhook_secret'))) {
        abort(401, 'Invalid signature');
    }

    $event = json_decode($payload, true);
    // 处理事件...
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
        // 1. 防重放：时间戳 5 分钟有效期
        long now = System.currentTimeMillis() / 1000;
        if (Math.abs(now - Long.parseLong(timestamp)) > 300) {
            throw new SecurityException("Timestamp expired — possible replay attack");
        }

        // 2. 构造签名原文
        String message = timestamp + "." + payload;

        // 3. 计算期望签名
        Mac mac = Mac.getInstance("HmacSHA256");
        mac.init(new SecretKeySpec(secret.getBytes(StandardCharsets.UTF_8), "HmacSHA256"));
        byte[] hash = mac.doFinal(message.getBytes(StandardCharsets.UTF_8));
        String expected = bytesToHex(hash);

        // 4. Constant-time 比较
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

// Spring Boot Controller 用法
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

        // 处理事件...
        return ResponseEntity.ok("OK");
    }
}
```

:::

::: warning 重放攻击防护
务必检查 `X-Timestamp` 距当前时间不超过 **5 分钟**，否则拒绝该请求。
:::

## 配置 Webhook

在 [Merchant Dashboard](https://app.ironixpay.com) 中设置：

> [在 API Reference 中查看 Webhook 端点 →](https://api.ironixpay.com/docs#tag/webhooks)

1. 进入 **Settings → Webhooks**
2. 填写端点 URL（生产环境必须 HTTPS）
3. 复制 **Webhook Secret**（`whsec_...`）
4. 勾选要订阅的事件

## 重试策略

投递失败后按指数退避重试：

| 次数 | 间隔 |
|------|------|
| 1 | 立即 |
| 2 | 15 秒 |
| 3 | 1 分钟 |
| 4 | 5 分钟 |
| 5 | 1 小时 |
| 6 | 6 小时 |
| 7 | 24 小时 |

连续 7 次失败后标记为 `giving_up`。可在 Dashboard 查看失败记录并手动重发。

## 最佳实践

1. **快速返回 2xx** — 收到后立即响应 `200`，业务逻辑异步处理
2. **幂等处理** — 用 `id` 去重，同一事件可能推送多次
3. **先验签名** — 处理 payload 前务必验证 `X-Signature`
4. **强制 HTTPS** — 生产端点必须 HTTPS（`localhost` 除外）
