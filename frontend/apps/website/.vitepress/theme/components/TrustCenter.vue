<script setup lang="ts">
import { computed } from 'vue'
import { useData } from 'vitepress'

const { lang } = useData()
const isZh = computed(() => lang.value === 'zh-CN')

const t = computed(() =>
  isZh.value
    ? {
        title: 'Trust Center',
        subtitle: '了解 IronixPay 如何保护您的资产和数据。',
        heroDesc:
          'IronixPay 是企业级稳定币支付基础设施。所有的交易在链上实时结算并自动归集，确保资金流转的透明与高效。以下是我们在安全和合规方面的核心能力。',
        secTitle: '安全能力',
        cards: [
          {
            icon: 'aml',
            title: '合规筛查',
            desc: '内置 OFAC / EU / UN 制裁名单 + GoPlus Security API 双重风控。每笔入金和出金地址自动审查，高风险交易实时拦截。',
          },

          {
            icon: 'isolation',
            title: '环境隔离',
            desc: '生产环境和沙盒环境完全隔离 — 独立数据库、独立 RPC 节点、独立钱包。沙盒测试不会影响生产数据。',
          },
          {
            icon: 'hdwallet',
            title: '专属收款地址',
            desc: '每个商户拥有基于 HD 钱包独立派生的专属收款账户。用户支付的资金将通过 Sweeper 引擎自动、安全地归集至主金库。',
          },
          {
            icon: 'webhook',
            title: 'Webhook 可靠性',
            desc: 'HMAC-SHA256 签名验证 + 7 级指数退避重试机制（最长 24h），确保支付事件零丢失。商户可随时在控制台查看完整投递日志。',
          },
        ],
        compTitle: '合规进展',
        compDesc: '我们正在构建符合行业标准的合规体系。',
        compliance: [
          { name: 'SOC 2 Type II', status: 'planned', label: 'Planning' },
          { name: 'ISO 27001', status: 'planned', label: 'Planning' },
          { name: 'OFAC / 制裁筛查', status: 'active', label: 'Active' },
          { name: 'GoPlus 链上风控', status: 'active', label: 'Active' },
        ],
        contactTitle: '有安全疑问？',
        contactDesc: '如果你在安全方面有任何问题或发现潜在漏洞，请联系我们。',
        contactCta: '联系安全团队',
      }
    : {
        title: 'Trust Center',
        subtitle: 'How IronixPay protects your assets and data.',
        heroDesc:
          'IronixPay is enterprise-grade stablecoin payment infrastructure. Every transaction is settled on-chain and swept automatically, ensuring transparent and efficient fund flows. Here is how we approach security and compliance.',
        secTitle: 'Security Capabilities',
        cards: [
          {
            icon: 'aml',
            title: 'AML / Sanctions Screening',
            desc: 'Built-in OFAC / EU / UN sanctions lists + GoPlus Security API dual-layer screening. Every inbound and outbound address is automatically checked. High-risk transactions are blocked in real-time.',
          },

          {
            icon: 'isolation',
            title: 'Environment Isolation',
            desc: 'Production and Sandbox are fully isolated — separate databases, RPC nodes, and wallets. Sandbox testing can never affect production data.',
          },
          {
            icon: 'hdwallet',
            title: 'Dedicated Addresses',
            desc: 'Each merchant is assigned a dedicated deposit address derived securely via HD wallets. Incoming funds are automatically swept to the treasury via the Sweeper service.',
          },
          {
            icon: 'webhook',
            title: 'Webhook Reliability',
            desc: 'HMAC-SHA256 signature verification + 7-level exponential backoff retry (up to 24h), ensuring zero event loss. Full delivery logs available in the merchant dashboard.',
          },
        ],
        compTitle: 'Compliance Roadmap',
        compDesc: 'We are building toward industry-standard compliance certifications.',
        compliance: [
          { name: 'SOC 2 Type II', status: 'planned', label: 'Planning' },
          { name: 'ISO 27001', status: 'planned', label: 'Planning' },
          { name: 'OFAC / Sanctions Screening', status: 'active', label: 'Active' },
          { name: 'GoPlus On-chain Risk', status: 'active', label: 'Active' },
        ],
        contactTitle: 'Security Questions?',
        contactDesc: 'If you have security concerns or want to report a vulnerability, reach out.',
        contactCta: 'Contact Security Team',
      },
)
</script>

<template>
  <div class="ix-trust">
    <!-- Hero -->
    <div class="ix-trust__hero">
      <div class="ix-trust__hero-inner">
        <div class="ix-trust__shield-icon">
          <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
            <polyline points="9 12 11 14 15 10" />
          </svg>
        </div>
        <h1 class="ix-trust__title">{{ t.title }}</h1>
        <p class="ix-trust__subtitle">{{ t.subtitle }}</p>
        <p class="ix-trust__hero-desc">{{ t.heroDesc }}</p>
      </div>
    </div>

    <!-- Security Cards -->
    <div class="ix-trust__section">
      <h2 class="ix-trust__section-heading">{{ t.secTitle }}</h2>
      <div class="ix-trust__grid">
        <div v-for="card in t.cards" :key="card.icon" class="ix-trust__card">
          <div class="ix-trust__card-icon">
            <!-- AML -->
            <svg v-if="card.icon === 'aml'" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>
            </svg>

            <!-- Isolation -->
            <svg v-else-if="card.icon === 'isolation'" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
              <rect x="2" y="2" width="20" height="8" rx="2" ry="2"/><rect x="2" y="14" width="20" height="8" rx="2" ry="2"/><line x1="6" y1="6" x2="6.01" y2="6"/><line x1="6" y1="18" x2="6.01" y2="18"/>
            </svg>
            <!-- HD wallet -->
            <svg v-else-if="card.icon === 'hdwallet'" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
              <rect x="2" y="4" width="20" height="16" rx="2"/><path d="M16 2v4"/><path d="M8 2v4"/><path d="M2 10h20"/><path d="M10 14h4"/>
            </svg>
            <!-- Webhook -->
            <svg v-else-if="card.icon === 'webhook'" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/>
            </svg>
          </div>
          <h3 class="ix-trust__card-title">{{ card.title }}</h3>
          <p class="ix-trust__card-desc">{{ card.desc }}</p>
        </div>
      </div>
    </div>

    <!-- Compliance Roadmap -->
    <div class="ix-trust__compliance">
      <h2 class="ix-trust__section-heading">{{ t.compTitle }}</h2>
      <p class="ix-trust__comp-desc">{{ t.compDesc }}</p>
      <div class="ix-trust__comp-grid">
        <div v-for="item in t.compliance" :key="item.name" class="ix-trust__comp-item">
          <span class="ix-trust__comp-name">{{ item.name }}</span>
          <span
            class="ix-trust__comp-badge"
            :class="{
              'ix-trust__comp-badge--active': item.status === 'active',
              'ix-trust__comp-badge--planned': item.status === 'planned',
            }"
          >
            {{ item.label }}
          </span>
        </div>
      </div>
    </div>

    <!-- Contact -->
    <div class="ix-trust__contact">
      <h2 class="ix-trust__contact-title">{{ t.contactTitle }}</h2>
      <p class="ix-trust__contact-desc">{{ t.contactDesc }}</p>
      <a href="mailto:security@ironixpay.com" class="ix-trust__contact-btn">
        {{ t.contactCta }}
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M4 10L10 4m0 0H5m5 0v5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
      </a>
    </div>
  </div>
</template>

<style scoped>
.ix-trust {
  max-width: 900px;
  margin: 0 auto;
  padding: 48px 24px 80px;
}

/* ─── Hero ─── */
.ix-trust__hero {
  text-align: center;
  margin-bottom: 56px;
}

.ix-trust__shield-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 64px;
  height: 64px;
  border-radius: 16px;
  background: linear-gradient(135deg, rgba(34, 197, 94, 0.1), rgba(34, 197, 94, 0.05));
  color: #22c55e;
  margin-bottom: 20px;
}

.ix-trust__title {
  font-family: 'Exo 2', sans-serif;
  font-size: 2.4rem;
  font-weight: 800;
  letter-spacing: -0.03em;
  color: #0f172a;
  margin: 0 0 10px;
}

.ix-trust__subtitle {
  font-size: 1.1rem;
  color: #64748b;
  margin: 0 0 20px;
}

.ix-trust__hero-desc {
  max-width: 640px;
  margin: 0 auto;
  font-size: 0.92rem;
  color: #475569;
  line-height: 1.7;
}

/* ─── Section heading ─── */
.ix-trust__section-heading {
  font-family: 'Exo 2', sans-serif;
  font-size: 1.4rem;
  font-weight: 700;
  color: #0f172a;
  text-align: center;
  margin: 0 0 28px;
  padding: 0;
  border: none;
}

/* ─── Security Cards ─── */
.ix-trust__section {
  margin-bottom: 56px;
}

.ix-trust__grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 20px;
}

.ix-trust__card {
  padding: 28px;
  border-radius: 14px;
  border: 1px solid #e2e8f0;
  background: #fff;
  transition: box-shadow 0.2s ease;
}

.ix-trust__card:hover {
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.05);
}

.ix-trust__card-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 42px;
  height: 42px;
  border-radius: 10px;
  background: #f1f5f9;
  color: #334155;
  margin-bottom: 14px;
}

.ix-trust__card-title {
  font-size: 1rem;
  font-weight: 700;
  color: #0f172a;
  margin: 0 0 8px;
  padding: 0;
  border: none;
}

.ix-trust__card-desc {
  font-size: 0.86rem;
  color: #475569;
  line-height: 1.65;
  margin: 0;
}

/* ─── Compliance Roadmap ─── */
.ix-trust__compliance {
  margin-bottom: 56px;
  text-align: center;
}

.ix-trust__comp-desc {
  font-size: 0.9rem;
  color: #64748b;
  margin: -16px 0 24px;
}

.ix-trust__comp-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 12px;
  max-width: 520px;
  margin: 0 auto;
}

.ix-trust__comp-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 20px;
  border-radius: 10px;
  border: 1px solid #e2e8f0;
  background: #fff;
}

.ix-trust__comp-name {
  font-size: 0.88rem;
  font-weight: 600;
  color: #0f172a;
}

.ix-trust__comp-badge {
  font-size: 0.72rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  padding: 3px 10px;
  border-radius: 100px;
}

.ix-trust__comp-badge--active {
  background: rgba(34, 197, 94, 0.1);
  color: #16a34a;
}

.ix-trust__comp-badge--planned {
  background: rgba(245, 158, 11, 0.1);
  color: #d97706;
}

/* ─── Contact ─── */
.ix-trust__contact {
  text-align: center;
  padding: 40px;
  border-radius: 16px;
  background: #f8fafc;
  border: 1px solid #e2e8f0;
}

.ix-trust__contact-title {
  font-family: 'Exo 2', sans-serif;
  font-size: 1.2rem;
  font-weight: 700;
  color: #0f172a;
  margin: 0 0 8px;
  padding: 0;
  border: none;
}

.ix-trust__contact-desc {
  font-size: 0.88rem;
  color: #64748b;
  margin: 0 0 20px;
}

.ix-trust__contact-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 12px 24px;
  border-radius: 10px;
  background: #0f172a;
  color: #fff;
  font-size: 0.88rem;
  font-weight: 600;
  text-decoration: none;
  transition: all 0.2s ease;
}

.ix-trust__contact-btn:hover {
  background: #1e293b;
  transform: translateY(-1px);
}

/* ═══ Dark mode ═══ */
.dark .ix-trust__title { color: #f1f5f9; }
.dark .ix-trust__subtitle { color: #94a3b8; }
.dark .ix-trust__hero-desc { color: #94a3b8; }
.dark .ix-trust__section-heading { color: #f1f5f9; }

.dark .ix-trust__card {
  background: #111827;
  border-color: rgba(255, 255, 255, 0.08);
}
.dark .ix-trust__card-icon {
  background: rgba(255, 255, 255, 0.06);
  color: #94a3b8;
}
.dark .ix-trust__card-title { color: #f1f5f9; }
.dark .ix-trust__card-desc { color: #94a3b8; }

.dark .ix-trust__comp-desc { color: #94a3b8; }
.dark .ix-trust__comp-item {
  background: #111827;
  border-color: rgba(255, 255, 255, 0.08);
}
.dark .ix-trust__comp-name { color: #f1f5f9; }

.dark .ix-trust__contact {
  background: #111827;
  border-color: rgba(255, 255, 255, 0.08);
}
.dark .ix-trust__contact-title { color: #f1f5f9; }
.dark .ix-trust__contact-desc { color: #94a3b8; }
.dark .ix-trust__contact-btn {
  background: #e2e8f0;
  color: #0f172a;
}
.dark .ix-trust__contact-btn:hover {
  background: #f1f5f9;
}

/* ═══ Responsive ═══ */
@media (max-width: 640px) {
  .ix-trust__grid {
    grid-template-columns: 1fr;
  }
  .ix-trust__comp-grid {
    grid-template-columns: 1fr;
  }
  .ix-trust__title {
    font-size: 1.8rem;
  }
}

/* ═══ VitePress overrides ═══ */
:deep(.vp-doc) .ix-trust h1,
:deep(.vp-doc) .ix-trust h2,
:deep(.vp-doc) .ix-trust h3 {
  border: none;
  margin-top: 0;
  padding-top: 0;
}

:deep(.vp-doc) .ix-trust a {
  color: inherit;
  text-decoration: none;
  font-weight: inherit;
}
</style>
