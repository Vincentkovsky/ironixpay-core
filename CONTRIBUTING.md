# Contributing to IronixPay

Thank you for helping improve IronixPay. Contributions to correctness,
security, documentation, integrations, and developer experience are welcome.

## Before you start

- Use a GitHub issue for significant behavioral changes.
- Never test with production wallets, funded seeds, or real customer data.
- Report security issues privately as described in `SECURITY.md`.
- Keep changes focused and follow the patterns already used in the codebase.

## Development checks

Backend:

```bash
cd backend
cargo fmt --check
cargo check --workspace --all-targets
cargo test --lib
```

Frontend:

```bash
cd frontend
pnpm install --frozen-lockfile
pnpm --filter @ironix-pay/api-client build
pnpm --filter @ironix-pay/ui build
pnpm --filter merchant-dashboard type-check
pnpm --filter @ironix-pay/checkout build
```

## Pull requests

- Use conventional commit prefixes such as `feat:`, `fix:`, `docs:`, and
  `refactor:`.
- Add focused tests for behavioral changes.
- Update public documentation when a contract or workflow changes.
- Do not commit generated builds, environment files, credentials, wallet
  material, or production configuration.

By submitting a contribution, you agree that it is licensed under the Apache
License 2.0.
