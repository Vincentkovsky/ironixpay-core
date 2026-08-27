---
title: PSP 与聚合平台 — 子商户模式 — IronixPay
description: 一个 API Key 管理多个子商户，独立对账、独立数据隔离。适用于支付聚合平台、电商 Marketplace、SaaS 代收款。
---

# PSP 与聚合平台 — 子商户模式

一个商户账户，多个子商户 — 每笔交易归属清晰，对账自动化，数据零串扰。

## 行业痛点

支付聚合平台（PSP）和多商户 Marketplace 面临**多租户管理难题**：

- **对账混乱** — 所有入金混在同一个账户，无法按子商户拆分
- **重复注册** — 每个下游需要独立注册、管理密钥，运营成本线性增长
- **数据隔离差** — 子商户之间可以看到彼此的交易数据
- **自建成本高** — 自行实现多租户要处理地址分配、费用分摊、Webhook 路由
- **合规报告难** — 无法按子商户维度导出交易明细

## IronixPay 如何解决

| 痛点 | IronixPay 方案 |
|---|---|
| 对账混乱 | **子商户自动归属** — 每笔交易绑定 `sub_merchant_code`，Dashboard 和 API 均可按子商户筛选 |
| 重复注册 | **一个 API Key 管全部** — 通过 `X-Sub-Merchant-Code` 请求头切换上下文 |
| 数据隔离 | **独立地址池** — 每个子商户派生独立的 HD 地址，链上和数据库均隔离 |
| 自建成本 | **开箱即用** — CRUD API 创建子商户，几行代码即完成集成 |
| 合规报告 | **按子商户导出** — Billing 页面支持按子商户筛选并导出 CSV |

## 典型用法

### 聚合平台集成流程

<FlowChart>
  <Step icon="api" title="PSP 平台注册 IronixPay 商户账户" />
  <Step icon="chain" title="通过 API 创建子商户">POST /v1/sub-merchants</Step>
  <Step icon="order" title="为子商户创建 Checkout Session">Header: X-Sub-Merchant-Code</Step>
  <Step icon="webhook" title="客户付款 → Webhook 触发">payload 包含 sub_merchant_code</Step>
  <Step icon="check" title="PSP 按 sub_merchant_code 分账对账" />
</FlowChart>

## 关键功能

- **子商户 CRUD** — `POST / GET / PATCH /v1/sub-merchants`，创建、查询、更新子商户，支持 `active` / `suspended` 状态管理
- **上下文切换** — 请求头 `X-Sub-Merchant-Code` 指定子商户，无需额外密钥
- **独立 HD 地址** — 每个子商户自动分配独立的 HD 派生地址池，跨 8 条链
- **混合视图** — Dashboard 支持「全部」「仅自身」「指定子商户」三种查看模式
- **统一计费** — 手续费从 PSP 母账户余额统一扣除，子商户无需独立余额
- **Webhook 归属** — `session.completed` 等事件 payload 自动携带 `sub_merchant_code`
- **CSV 导出** — Sessions / Billing 页面可按子商户维度导出，满足对账和合规需求

## 最受益的平台类型

| 平台类型 | 子商户模式的优势 |
|---|---|
| **支付聚合 (PSP)** | 一套 API 接入，下游商户零感知，统一管理 |
| **电商 Marketplace** | 每个卖家一个子商户，收款自动归属，平台统一结算 |
| **SaaS 代收款** | 为客户代收，按 `sub_merchant_code` 划分到账明细 |
| **代理商网络** | 代理商管辖多个终端商户，分层查看和导出 |

## API 速查

### 创建子商户

```bash
curl -X POST https://api.ironixpay.com/v1/sub-merchants \
  -H "Authorization: Bearer $IRONIXPAY_SECRET_KEY" \
  -H "Content-Type: application/json" \
  -d '{"sub_merchant_code": "shop_tokyo", "display_name": "Tokyo Branch"}'
```

### 为子商户创建支付

```bash
curl -X POST https://api.ironixpay.com/v1/checkout/sessions \
  -H "Authorization: Bearer $IRONIXPAY_SECRET_KEY" \
  -H "X-Sub-Merchant-Code: shop_tokyo" \
  -H "Content-Type: application/json" \
  -d '{
    "pricing_amount": "50",
    "pricing_currency": "USDT",
    "currency": "USDT",
    "network": "TRON",
    "success_url": "https://example.com/success",
    "cancel_url": "https://example.com/cancel"
  }'
```

### 查询子商户列表

```bash
curl https://api.ironixpay.com/v1/sub-merchants \
  -H "Authorization: Bearer $IRONIXPAY_SECRET_KEY"
```

## 开始使用

- [快速开始](/guide/quickstart) — 注册账户并获取 API 密钥
- [Checkout Sessions](/guide/checkout) — 了解支付会话生命周期
- [Webhooks 指南](/guide/webhooks) — 自动化支付通知和对账
- [API 参考](https://api.ironixpay.com/docs) — 查看完整的子商户 API 文档
- [测试指南](/guide/testing) — 在沙盒环境测试子商户流程
