# Security Policy

IronixPay processes payment data and signs blockchain transactions. Treat every
deployment as security-sensitive and complete an independent review before
using it with real funds.

## Reporting a vulnerability

Do not open a public GitHub issue for a suspected vulnerability.

Email `support@ironixpay.com` with the subject `Security: <short summary>` and
include:

- the affected component and version or commit;
- reproduction steps or a proof of concept;
- the expected impact;
- any suggested mitigation.

Please avoid accessing other users' data, moving funds, degrading the hosted
service, or publishing details before a fix is available. We will acknowledge a
report as soon as practical and coordinate disclosure with the reporter.

## Supported versions

Until the first stable release, only the latest commit on `main` receives
security fixes.

## Deployment responsibility

The public repository contains development defaults only. Operators are
responsible for key management, RPC trust, database security, network
isolation, monitoring, backups, compliance, and incident response.
