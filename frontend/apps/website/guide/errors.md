# 错误处理

所有错误以统一 JSON 格式返回。[在 API Reference 中查看完整错误定义 →](https://api.ironixpay.com/docs#tag/errors)

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

## Error Type

| type | 含义 |
|------|------|
| `invalid_request_error` | 请求有误 — 参数缺失、格式错误或认证失败 |
| `api_error` | 服务端内部错误（罕见，可重试） |
| `idempotency_error` | 同一 Idempotency-Key 搭配了不同的请求体 |

## Error Code

| code | HTTP | 触发原因 |
|------|------|----------|
| `authentication_failed` | 401 | API Key 缺失或无效 |
| `parameter_invalid` | 400 | 参数校验不通过（详见 `param` 字段） |
| `resource_missing` | 404 | 资源不存在 |
| `permission_denied` | 403 | 无权操作 |
| `conflict` | 409 | 状态冲突（如 Session 已完成） |
| `idempotency_conflict` | 409 | 同一 key，不同请求体 |
| `session_expired` | 410 | Session 已过期 |
| `environment_mismatch` | 403 | API Key 环境与目标网络不匹配 |
| `service_unavailable` | 503 | 服务暂不可用（如地址池耗尽） |
| `api_error` | 500 | 服务端内部错误 |

<!-- sync: backend/src/api/error.rs AppError::to_api_error -->

## 错误处理示例

```javascript
try {
  const session = await fetch('/v1/checkout/sessions', { ... });
  const data = await session.json();

  if (!session.ok) {
    const { error } = data;
    console.error(`[${error.type}] ${error.code}: ${error.message}`);

    if (error.code === 'authentication_failed') {
      // 检查 API Key
    } else if (error.code === 'parameter_invalid') {
      // 看 error.param 定位哪个字段出了问题
    }
  }
} catch (err) {
  // 网络错误 — 可安全重试
}
```

## HTTP 状态码速查

| 状态码 | 含义 |
|--------|------|
| `200` | 成功 |
| `201` | 已创建 |
| `400` | 请求有误 |
| `401` | 未认证 |
| `403` | 无权访问 |
| `404` | 未找到 |
| `409` | 冲突 |
| `410` | 已过期 |
| `500` | 服务端错误 |
| `503` | 服务不可用 |
