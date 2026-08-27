# Errors

All errors return a consistent JSON structure. [View full error definitions in API Reference →](https://api.ironixpay.com/docs#tag/errors)

```json
{
  "error": {
    "type": "invalid_request_error",
    "code": "parameter_invalid",
    "message": "Amount must be at least 1 USDT",
    "param": "amount",
    "doc_url": "https://ironixpay.com/guide/errors#parameter_invalid"
  }
}
```

<!-- sync: backend/src/api/error.rs ApiErrorBody -->

## Error Types

| Type | Description |
|------|-------------|
| `invalid_request_error` | Bad request — missing/invalid parameters, auth failure |
| `api_error` | Internal server error (rare, please retry) |
| `idempotency_error` | Idempotency key reused with different body |

## Error Codes

| Code | HTTP | When |
|------|------|------|
| `authentication_failed` | 401 | Missing or invalid API key |
| `parameter_invalid` | 400 | Invalid request parameter (check `param` field) |
| `resource_missing` | 404 | Resource not found |
| `permission_denied` | 403 | Insufficient permissions |
| `conflict` | 409 | State conflict (e.g., session already completed) |
| `idempotency_conflict` | 409 | Same idempotency key, different request body |
| `session_expired` | 410 | Session has expired |
| `environment_mismatch` | 403 | API key environment does not match the target network |
| `service_unavailable` | 503 | Service temporarily unavailable (e.g., address pool exhausted) |
| `api_error` | 500 | Internal server error |

<!-- sync: backend/src/api/error.rs AppError::to_api_error -->

## Handling Errors

```javascript
try {
  const session = await fetch('/v1/checkout/sessions', { ... });
  const data = await session.json();

  if (!session.ok) {
    const { error } = data;
    console.error(`[${error.type}] ${error.code}: ${error.message}`);

    if (error.code === 'authentication_failed') {
      // Check your API key
    } else if (error.code === 'parameter_invalid') {
      // Fix the parameter indicated by error.param
    }
  }
} catch (err) {
  // Network error — safe to retry
}
```

## HTTP Status Codes

| Status | Meaning |
|--------|---------|
| `200` | Success |
| `201` | Created |
| `400` | Bad Request |
| `401` | Unauthorized |
| `403` | Forbidden |
| `404` | Not Found |
| `409` | Conflict |
| `410` | Gone |
| `500` | Internal Server Error |
| `503` | Service Unavailable |
