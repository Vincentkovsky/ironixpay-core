<script setup lang="ts">
import { computed } from 'vue'
import { useData } from 'vitepress'

const { lang } = useData()
const isZh = computed(() => lang.value === 'zh-CN')

const t = computed(() =>
  isZh.value
    ? {
        eyebrow: 'Enterprise access',
        title: '复杂业务，不该从注册页面开始',
        description:
          '告诉我们你的业务模型、资金规模和目标网络。我们会先给出支付架构、接入范围和费率建议，再决定下一步。',
        outcomes: ['支付与结算路径建议', '网络、资产和 API 范围', '预估费率与接入周期'],
        response: '1 个工作日内回复',
        cta: '获取企业接入方案',
        href: '/enterprise',
      }
    : {
        eyebrow: 'Enterprise access',
        title: 'Complex payment flows deserve a better starting point',
        description:
          'Share your business model, expected volume, and target rails. We will outline the payment architecture, integration scope, and commercial path before you commit.',
        outcomes: ['Payment and settlement architecture', 'Network, asset, and API scope', 'Indicative pricing and integration timeline'],
        response: 'Response within one business day',
        cta: 'Get an integration plan',
        href: '/en/enterprise',
      },
)
</script>

<template>
  <section class="ix-enterprise-access">
    <div class="ix-enterprise-access__inner">
      <div class="ix-enterprise-access__copy">
        <span class="ix-enterprise-access__eyebrow">{{ t.eyebrow }}</span>
        <h2>{{ t.title }}</h2>
        <p>{{ t.description }}</p>
      </div>

      <div class="ix-enterprise-access__response">
        <ol>
          <li v-for="(outcome, index) in t.outcomes" :key="outcome">
            <span>{{ String(index + 1).padStart(2, '0') }}</span>
            {{ outcome }}
          </li>
        </ol>
        <div class="ix-enterprise-access__action">
          <small>{{ t.response }}</small>
          <a
            :href="t.href"
            data-analytics-event="cta_click"
            data-analytics-name="enterprise_assessment"
            data-analytics-location="home_enterprise"
          >
            {{ t.cta }}
            <span aria-hidden="true">→</span>
          </a>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.ix-enterprise-access {
  --ix-enterprise-access-accent: #60a5fa;
  border-top: 1px solid rgba(96, 165, 250, 0.2);
  border-bottom: 1px solid rgba(96, 165, 250, 0.2);
  background: #0f172a;
  color: #f8fafc;
}

.ix-enterprise-access__inner {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(390px, 0.9fr);
  gap: 80px;
  max-width: 1140px;
  margin: 0 auto;
  padding: 76px 32px;
}

.ix-enterprise-access__copy {
  align-self: center;
}

.ix-enterprise-access__eyebrow {
  display: block;
  margin-bottom: 14px;
  color: var(--ix-enterprise-access-accent);
  font-family: 'Exo 2', sans-serif;
  font-size: 0.76rem;
  font-weight: 700;
  letter-spacing: 0.14em;
  text-transform: uppercase;
}

.ix-enterprise-access h2 {
  max-width: 620px;
  margin: 0 0 18px;
  color: #ffffff;
  font-family: 'Exo 2', sans-serif;
  font-size: clamp(2rem, 4vw, 3.1rem);
  font-weight: 800;
  letter-spacing: 0;
  line-height: 1.08;
}

.ix-enterprise-access__copy p {
  max-width: 640px;
  margin: 0;
  color: #cbd5e1;
  font-size: 1rem;
  line-height: 1.75;
}

.ix-enterprise-access__response {
  padding-left: 32px;
  border-left: 1px solid rgba(148, 163, 184, 0.24);
}

.ix-enterprise-access__response ol {
  padding: 0;
  margin: 0;
  list-style: none;
}

.ix-enterprise-access__response li {
  display: grid;
  grid-template-columns: 34px 1fr;
  gap: 12px;
  align-items: center;
  min-height: 48px;
  border-bottom: 1px solid rgba(148, 163, 184, 0.18);
  color: #e2e8f0;
  font-size: 0.9rem;
}

.ix-enterprise-access__response li span {
  color: var(--ix-enterprise-access-accent);
  font-family: 'Exo 2', sans-serif;
  font-size: 0.72rem;
  font-weight: 700;
}

.ix-enterprise-access__action {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-top: 24px;
}

.ix-enterprise-access__action small {
  color: #94a3b8;
  font-size: 0.74rem;
}

.ix-enterprise-access__action a {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  min-height: 48px;
  padding: 0 22px;
  border: 1px solid #3b82f6;
  border-radius: 12px;
  background: linear-gradient(135deg, #2563eb, #3b82f6);
  color: #ffffff;
  font-size: 0.88rem;
  font-weight: 700;
  text-decoration: none;
  box-shadow: 0 4px 20px rgba(37, 99, 235, 0.3);
  transition: transform 0.2s ease, box-shadow 0.2s ease;
}

.ix-enterprise-access__action a:hover {
  transform: translateY(-2px);
  box-shadow: 0 8px 30px rgba(37, 99, 235, 0.42);
}

.ix-enterprise-access__action a:focus-visible {
  outline: 3px solid rgba(147, 197, 253, 0.48);
  outline-offset: 3px;
}

@media (max-width: 860px) {
  .ix-enterprise-access__inner {
    grid-template-columns: 1fr;
    gap: 42px;
    padding: 60px 24px;
  }

  .ix-enterprise-access__response {
    padding: 0;
    border-left: 0;
  }
}

@media (max-width: 520px) {
  .ix-enterprise-access__action {
    align-items: stretch;
    flex-direction: column;
  }

  .ix-enterprise-access__action a {
    width: 100%;
  }
}

@media (prefers-reduced-motion: reduce) {
  .ix-enterprise-access__action a {
    transition: none;
  }

  .ix-enterprise-access__action a:hover {
    transform: none;
  }
}
</style>
