# Payment Exceptions

When a payment doesn't match an active checkout session — whether it arrives late, goes to an idle address, or triggers a risk flag — IronixPay captures it as a **Payment Exception**. No funds are ever lost.

You can review and resolve exceptions in your [Dashboard](https://app.ironixpay.com) under the **Resolution Center**.

## Exception Types

| Type | When It Occurs |
|------|----------------|
| `session_expired` | Payment received **after** the session's 30-minute window has closed |
| `underpaid_expired` | Session expired with a **partial payment** — the customer paid less than the required amount before timeout |
| `no_active_session` | Payment sent to a receive address that has **no active session** bound to it |
| `session_already_completed` | Payment sent to an address whose session has **already been paid** in full |
| `risk_blocked` | Payment from an address flagged by **AML screening** (sanctions list or risk intelligence) |
| `dust_payment` | Payment below the **dust threshold** (< 0.1 USDT) — automatically ignored |
| `unknown` | An edge case that doesn't match any known pattern |

### Lifecycle

Every exception follows a simple lifecycle:

```
Pending → Processing → Resolved
                    ↘ Failed (retry available)
```

- **Pending** — Awaiting your review
- **Processing** — A resolution action has been initiated (e.g., broadcasting a transfer)
- **Resolved** — Action completed successfully
- **Failed** — Action failed (e.g., broadcast error). You can retry.

## Resolution Actions

There are three actions you can take to resolve an exception, depending on its type:

### Accept

**Credit the payment to the original session.**

The payment is recognized as valid, the session is marked as Paid, and funds are swept to your treasury. Use this when you're confident the customer intended to pay this session — they were just late, or the amount was partial.

::: tip When to use
A loyal customer sent payment 5 minutes after session expiry. You still want to fulfill their order.
:::

**Available for:** `session_expired`, `underpaid_expired`

### Attach

**Bind the payment to a different session.**

Use this when funds arrived at the wrong address. You select the target session by its ID, and the payment is credited to that session instead.

::: tip When to use
A customer copied an old address from their transaction history. You can attach the payment to their current open session.
:::

**Available for:** `no_active_session`, `session_already_completed`

### Transfer

**Send the funds to any TRON address** (refund or manual settlement).

This initiates an on-chain USDT transfer. A 2FA code is required for security. You can transfer the full amount or specify a partial amount.

::: warning
Transfer requires sufficient TRX for network fees on the source address. The system will attempt to rent energy first to minimize costs; if energy rental fails, it falls back to burning TRX.
:::

**Available for:** All exception types (except `dust_payment`)

## Action Matrix

Quick reference for which actions are available per exception type:

| Exception Type | Accept | Attach | Transfer |
|----------------|:------:|:------:|:--------:|
| `session_expired` | ✅ | — | ✅ |
| `underpaid_expired` | ✅ | — | ✅ |
| `no_active_session` | — | ✅ | ✅ |
| `session_already_completed` | — | ✅ | ✅ |
| `risk_blocked` | — | — | ✅ |
| `dust_payment` | — | — | — |
| `unknown` | — | — | ✅ |

::: info Why can't I accept or attach a `risk_blocked` payment?
AML compliance policy prohibits crediting funds from flagged addresses to any session. The only option is to refund (transfer) the funds back to the sender or another designated address.
:::

## Webhook Events

When an exception is created, IronixPay sends a webhook event:

```json
{
  "event_type": "payment.exception",
  "data": {
    "id": "exc_abc123...",
    "exception_type": "session_expired",
    "amount": "10.5",
    "currency": "USDT",
    "tx_hash": "abc123...",
    "session_id": "cs_def456..."
  }
}
```

## Best Practices

1. **Set up webhook listeners** for `payment.exception` events so your team gets alerted immediately
2. **Resolve promptly** — funds sit in the receive address until resolved
3. **Use Accept for legitimate late payments** rather than asking customers to repay
4. **Review risk_blocked exceptions carefully** — contact your compliance team before transferring funds from flagged addresses
