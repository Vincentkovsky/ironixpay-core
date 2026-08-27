# Authentication

IronixPay uses **API keys** to authenticate requests. Include your key in the `Authorization` header:

```http
Authorization: Bearer sk_test_abc123...
```

## API Key Types

| Key Prefix | Environment | Description |
|------------|-------------|-------------|
| `sk_live_` | Production  | Real funds on TRON, Solana, BSC, ETH, Polygon, Arbitrum, Optimism, Base |
| `sk_test_` | Sandbox     | TRON Nile testnet, no real funds |

::: tip
Use test keys during development. Sandbox sessions behave identically to production but use testnet tokens.
:::

::: warning Data Isolation
Test and production data are completely separate. Test API keys cannot access production sessions, and vice versa.
:::

## Managing API Keys

Generate and manage API keys in your [Merchant Dashboard](https://app.ironixpay.com).

- You can have multiple active keys per environment
- Rotate keys without downtime by creating a new key before revoking the old one
- Never expose your secret key in client-side code

## Security Best Practices

1. **Keep keys secret** — Store keys in environment variables, never commit them to version control
2. **Use test keys for development** — Only use `sk_live_` keys in production
3. **Rotate regularly** — Rotate keys periodically and after any suspected compromise
4. **Restrict access** — Limit who on your team can view and manage API keys
