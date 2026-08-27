---
title: 跨境电商加密货币支付 — IronixPay
description: 为在线商店接入 USDT 支付。跨境交易零拒付、低固定费率、8 链准实时结算。
---

# 跨境电商加密货币支付

全球销售，准实时结算 — 无需银行账户、无拒付、无跨境手续费，接受全球客户的 USDT 付款。

## 行业痛点

跨境电商商户面临**层层叠加的支付摩擦**：

- **跨境附加费** — 卡组织对国际交易额外收取 1–3%，加上基础处理费
- **汇率损耗** — 双重转换（买家货币 → 美元 → 卖家货币）损耗 2–4%
- **拒付责任** — 实物商品商户承担风险，争议可能持续数月
- **市场排斥** — 银行服务不足地区（非洲、东南亚、中亚）的客户根本无法刷卡
- **到账延迟** — 7–14 天的结算周期影响现金流

## IronixPay 如何解决

| 痛点 | IronixPay 方案 |
|---|---|
| 跨境附加费 | **[低固定费率](/pricing)** — 客户在东京还是圣保罗，费率一样 |
| 汇率损耗 | **USDT 锚定美元** — 无需转换，收到即是最终金额 |
| 拒付 | **零拒付** — 区块链转账不可逆 |
| 市场排斥 | **全球覆盖** — 任何拥有加密钱包的人都可以付款 |
| 到账延迟 | **分钟级结算** — 资金数分钟内到达商户余额 |

## 典型用法

### 客户结账

<FlowChart>
  <Step icon="cart" title="客户将商品加入购物车">进入结算页面</Step>
  <Step icon="api" title="后端创建 Checkout Session">amount: 49.99 USDT</Step>
  <Step icon="redirect" title="客户跳转至 IronixPay 收银台">或通过 SDK 嵌入页面内</Step>
  <Step icon="send" title="客户从任意钱包或交易所发送 USDT" />
  <Step icon="scan" title="IronixPay 检测到链上转账" />
  <Step icon="webhook" title="Webhook 触发 → 系统标记订单为「已支付」" />
  <Step icon="check" title="客户跳转至成功页面">商户发货</Step>
</FlowChart>

## 电商关键功能

- **两种集成方式** — 跳转至 IronixPay 收银台，或通过 `@ironix-pay/sdk` 嵌入页面
- **现成插件** — [WooCommerce 插件](/use-cases/woocommerce) 零代码 WordPress 集成
- **订单追踪** — 附加 `client_reference_id`（订单 ID），Webhook 中原样返回用于对账
- **自动归集** — 所有收款自动汇集至金库钱包
- **超额/不足支付处理** — 内置[异常管理](/guide/exceptions)，处理金额偏差
- **多链支持** — 客户可选择偏好的链（TRON、Solana、BSC、ETH、Polygon 等）

## 最受益的电商细分领域

| 细分领域 | 加密支付的优势 |
|---|---|
| **数字商品** | 即时支付后即时交付，无结算延迟 |
| **奢侈品 / 高客单** | 避免 $1,000+ 订单上 3%+ 的处理费 |
| **Dropshipping** | 用 USDT 向供应商付款 — 进出同一货币 |
| **跨境 DTC** | 无需当地支付集成即可覆盖加密友好市场 |

## 已经在用 WooCommerce？

如果商店运行在 WordPress + WooCommerce 上，可以查看我们的 [WooCommerce 集成指南](/use-cases/woocommerce) — 安装插件、粘贴 API Key，5 分钟上线。

自建商城（Next.js、Vue 等），可以参考 [Next.js / React 指南](/use-cases/nextjs)或[前端集成文档](/guide/integration)。

## 开始使用

- [快速开始](/guide/quickstart) — 注册账户并获取 API 密钥
- [WooCommerce 插件](/use-cases/woocommerce) — WordPress 零代码集成
- [Next.js / React](/use-cases/nextjs) — 自建商城 SDK 集成
- [Webhooks 指南](/guide/webhooks) — 自动化订单确认
- [测试指南](/guide/testing) — 在沙盒环境测试完整购买流程
