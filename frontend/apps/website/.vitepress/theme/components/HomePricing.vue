<script setup lang="ts">
import { computed } from 'vue'
import { useData } from 'vitepress'

const { lang } = useData()
const isZh = computed(() => lang.value === 'zh-CN')

const t = computed(() =>
  isZh.value
    ? {
        eyebrow: '定价',
        title: '0.5% 成功收款费率',
        desc: '没有月费、设置费或隐藏成本。TRON/ETH 最低 1 USDT，BSC、Solana 与 L2 链最低 0.1 USDT。',
        primary: '查看完整定价',
        primaryHref: '/pricing',
        secondary: '开始使用',
        secondaryHref: 'https://app.ironixpay.com',
        stats: [
          ['0.5%', '成功收款手续费'],
          ['0 USDT', '月费与设置费'],
          ['0.5 USDT', 'BSC / L2 / Solana 出金'],
        ],
      }
    : {
        eyebrow: 'Pricing',
        title: '0.5% per successful payment',
        desc: 'No monthly fees, setup fees, or hidden costs. Min. 1 USDT on TRON/ETH, 0.1 USDT on BSC, Solana, and L2 chains.',
        primary: 'See Full Pricing',
        primaryHref: '/en/pricing',
        secondary: 'Get Started',
        secondaryHref: 'https://app.ironixpay.com',
        stats: [
          ['0.5%', 'successful payment fee'],
          ['0 USDT', 'monthly and setup fees'],
          ['0.5 USDT', 'BSC / L2 / Solana withdrawal'],
        ],
      },
)
</script>

<template>
  <section class="ix-home-pricing">
    <div class="ix-home-pricing__inner">
      <div class="ix-home-pricing__copy">
        <span class="ix-home-pricing__eyebrow">{{ t.eyebrow }}</span>
        <h2 class="ix-home-pricing__title">{{ t.title }}</h2>
        <p class="ix-home-pricing__desc">{{ t.desc }}</p>
      </div>

      <div class="ix-home-pricing__panel">
        <div class="ix-home-pricing__stats">
          <div v-for="item in t.stats" :key="item[1]" class="ix-home-pricing__stat">
            <strong>{{ item[0] }}</strong>
            <span>{{ item[1] }}</span>
          </div>
        </div>
        <div class="ix-home-pricing__actions">
          <a
            :href="t.primaryHref"
            class="ix-home-pricing__btn ix-home-pricing__btn--primary"
            data-analytics-event="cta_click"
            data-analytics-name="pricing_details"
            data-analytics-location="home_pricing"
          >
            {{ t.primary }}
          </a>
          <a
            :href="t.secondaryHref"
            target="_blank"
            class="ix-home-pricing__btn ix-home-pricing__btn--ghost"
            data-analytics-event="cta_click"
            data-analytics-name="create_account"
            data-analytics-location="home_pricing"
          >
            {{ t.secondary }}
          </a>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.ix-home-pricing {
  padding: 24px 24px 88px;
}

.ix-home-pricing__inner {
  max-width: 1100px;
  margin: 0 auto;
  display: grid;
  grid-template-columns: minmax(0, 0.9fr) minmax(360px, 1.1fr);
  gap: 32px;
  align-items: center;
  border: 1px solid rgba(37, 99, 235, 0.16);
  border-radius: 18px;
  background:
    linear-gradient(135deg, rgba(37, 99, 235, 0.08), transparent 42%),
    #ffffff;
  padding: 34px;
  box-shadow: 0 18px 50px rgba(15, 23, 42, 0.06);
}

.ix-home-pricing__eyebrow {
  display: inline-block;
  font-family: 'Exo 2', sans-serif;
  font-size: 0.75rem;
  font-weight: 700;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: #2563eb;
  margin-bottom: 12px;
}

.ix-home-pricing__title {
  font-family: 'Exo 2', sans-serif;
  font-size: clamp(2rem, 4vw, 3.35rem);
  line-height: 1;
  font-weight: 850;
  color: #0f172a;
  margin: 0 0 14px;
}

.ix-home-pricing__desc {
  font-size: 1rem;
  line-height: 1.75;
  color: #64748b;
  margin: 0;
}

.ix-home-pricing__panel {
  display: grid;
  gap: 22px;
}

.ix-home-pricing__stats {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.ix-home-pricing__stat {
  min-height: 116px;
  padding: 20px 18px;
  border-radius: 14px;
  background: rgba(248, 250, 252, 0.88);
  border: 1px solid #e2e8f0;
}

.ix-home-pricing__stat strong {
  display: block;
  font-family: 'Exo 2', sans-serif;
  font-size: 1.7rem;
  line-height: 1.1;
  color: #0f172a;
  margin-bottom: 10px;
}

.ix-home-pricing__stat span {
  color: #64748b;
  font-size: 0.88rem;
  line-height: 1.45;
}

.ix-home-pricing__actions {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
}

.ix-home-pricing__btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-height: 46px;
  padding: 0 18px;
  border-radius: 10px;
  font-weight: 700;
  font-size: 0.95rem;
  text-decoration: none;
  transition: transform 0.2s ease, border-color 0.2s ease, background 0.2s ease;
}

.ix-home-pricing__btn:hover {
  transform: translateY(-1px);
}

.ix-home-pricing__btn--primary {
  color: #ffffff;
  background: #2563eb;
  border: 1px solid #2563eb;
}

.ix-home-pricing__btn--ghost {
  color: #1e293b;
  background: #ffffff;
  border: 1px solid #dbe3ef;
}

.dark .ix-home-pricing__inner {
  background:
    linear-gradient(135deg, rgba(59, 130, 246, 0.16), transparent 45%),
    #0f172a;
  border-color: rgba(96, 165, 250, 0.22);
}

.dark .ix-home-pricing__title,
.dark .ix-home-pricing__stat strong {
  color: #f8fafc;
}

.dark .ix-home-pricing__desc,
.dark .ix-home-pricing__stat span {
  color: #94a3b8;
}

.dark .ix-home-pricing__stat {
  background: rgba(15, 23, 42, 0.78);
  border-color: rgba(148, 163, 184, 0.18);
}

.dark .ix-home-pricing__btn--ghost {
  color: #e2e8f0;
  background: rgba(15, 23, 42, 0.7);
  border-color: rgba(148, 163, 184, 0.24);
}

@media (max-width: 860px) {
  .ix-home-pricing__inner {
    grid-template-columns: 1fr;
    padding: 26px;
  }

  .ix-home-pricing__stats {
    grid-template-columns: 1fr;
  }

  .ix-home-pricing__stat {
    min-height: auto;
  }
}
</style>
