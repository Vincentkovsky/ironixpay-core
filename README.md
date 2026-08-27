# IronixPay

[![CI](https://github.com/Vincentkovsky/ironixpay-core/actions/workflows/ci.yml/badge.svg)](https://github.com/Vincentkovsky/ironixpay-core/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Open-source stablecoin payment infrastructure built with Rust and Vue.
IronixPay creates checkout sessions, detects confirmed on-chain payments,
delivers signed webhooks, sweeps funds, and processes merchant payouts across
TRON, Solana, and EVM networks.

> [!WARNING]
> IronixPay is beta software that signs blockchain transactions. Do not use it
> with real funds until you have completed an independent security review,
> configured production-grade key management, and tested recovery procedures.

## Why IronixPay

- Unified checkout API for USDT and USDC.
- TRON, Solana, Ethereum, BNB Chain, Polygon, Arbitrum, Base, and Optimism.
- Per-payment addresses derived from an HD wallet.
- Confirmation-aware indexing and idempotent payment processing.
- Transactional outbox for reliable webhook and background job delivery.
- Automated token sweeping, EVM gas funding, and TRON energy delegation.
- Durable outbound transaction journal for ambiguous broadcast recovery.
- Merchant dashboard, hosted checkout, JavaScript SDK, and WooCommerce plugin.
- PostgreSQL-backed ledger with explicit payment and payout state machines.

## Architecture

```text
Merchant / Integration
        |
        v
   Checkout API ---> PostgreSQL ledger ---> Transactional outbox
        |                    |                       |
        v                    v                       v
 Hosted checkout       Chain indexers          Signed webhooks
                             |
                             v
                    Payment processor
                             |
                  +----------+----------+
                  |                     |
                  v                     v
               Sweeper              Payout worker
                  |                     |
                  +----------+----------+
                             |
                             v
                    TRON / EVM / Solana
```

Start with the [system design](.docs/architecture/system-design/README.md),
[state machines](.docs/architecture/system-design/state-machines.md), and
[database schema](.docs/architecture/system-design/database-schema.md).

## Repository layout

```text
backend/                       Rust, Axum, SeaORM, PostgreSQL
frontend/apps/checkout/        Customer payment experience
frontend/apps/merchant-dashboard/
                               Merchant operations portal
frontend/apps/website/         Product and integration documentation
frontend/packages/api-client/  Typed API client
frontend/packages/sdk/         Embedded checkout SDK
integrations/woocommerce/      WooCommerce gateway
examples/                      Next.js and Telegram bot examples
.docs/architecture/            Architecture and decision records
```

The hosted service's production deployment, internal administration console,
customer data, and operating procedures are intentionally not part of this
repository.

## Local development

### Prerequisites

- Rust 1.93 or newer
- Node.js 22 or newer
- pnpm 10.29.2
- Docker with Docker Compose

### 1. Start PostgreSQL

```bash
docker compose up -d
```

### 2. Configure the backend

```bash
cp backend/.env.example backend/.env
```

Replace every value enclosed in angle brackets. Generate a unique encryption
key with:

```bash
openssl rand -hex 32
```

Use a newly generated BIP-39 mnemonic that is dedicated to local test networks.
Never reuse a wallet seed that has held real funds.

### 3. Start the backend

```bash
cd backend
cargo run
```

The API listens on `http://localhost:3000`. Readiness is available at
`http://localhost:3000/ready`.

### 4. Start a frontend

```bash
cd frontend
pnpm install --frozen-lockfile
pnpm --filter @ironix-pay/api-client build
pnpm --filter @ironix-pay/ui build
pnpm --filter merchant-dashboard dev
```

To run the customer checkout instead:

```bash
pnpm --filter @ironix-pay/checkout dev
```

## Verification

Backend:

```bash
cd backend
cargo check --workspace --all-targets
cargo test --lib
```

Frontend:

```bash
cd frontend
pnpm --filter @ironix-pay/api-client build
pnpm --filter @ironix-pay/ui build
pnpm --filter merchant-dashboard type-check
pnpm --filter @ironix-pay/checkout build
```

## Integrations

- [Embedded JavaScript SDK](frontend/packages/sdk/README.md)
- [Next.js starter](examples/nextjs-starter/README.md)
- [Telegram bot starter](examples/telegram-bot/README.md)
- [WooCommerce gateway](integrations/woocommerce/ironixpay-usdt-gateway/README.md)
- [Hosted documentation](https://ironixpay.com/guide/quickstart)

## Contributing and security

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request. Report
vulnerabilities privately according to [SECURITY.md](SECURITY.md); do not open
a public security issue.

## License

Unless a subdirectory states otherwise, the project is licensed under the
[Apache License 2.0](LICENSE). The WooCommerce plugin is distributed under
GPL-2.0-or-later as stated in its source headers and README. These licenses do
not grant permission to represent modified or hosted versions as the official
IronixPay service or to use IronixPay trademarks beyond customary attribution.
