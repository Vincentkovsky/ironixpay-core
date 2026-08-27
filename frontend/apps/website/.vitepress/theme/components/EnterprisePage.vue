<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { useData } from 'vitepress'
import { trackEvent } from '../analytics'

const { lang } = useData()
const isZh = computed(() => lang.value === 'zh-CN')
const step = ref(1)
const submitting = ref(false)
const submitted = ref(false)
const errorMessage = ref('')

const form = reactive({
  businessType: '',
  monthlyVolume: '',
  networks: [] as string[],
  integrationNeeds: [] as string[],
  companyName: '',
  companyWebsite: '',
  contactEmail: '',
  telegram: '',
  message: '',
  faxNumber: '',
})

const t = computed(() =>
  isZh.value
    ? {
        eyebrow: '企业接入',
        title: '先把支付路径设计清楚，再决定怎么接入',
        description:
          '适用于交易规模较大、资金路径复杂，或需要 Payouts、子商户和多网络支持的团队。我们会基于你的实际业务给出一份初步接入建议。',
        responseLabel: '你将获得',
        responseItems: [
          ['支付架构', '收款、归集和出款路径建议'],
          ['接入范围', '适用网络、资产、API 与风控能力'],
          ['商业建议', '预估费率、接入周期和下一步'],
        ],
        responseTime: '1 个工作日内，由熟悉支付架构的团队成员回复。',
        stepOne: '业务需求',
        stepTwo: '联系方式',
        formTitleOne: '描述你的支付场景',
        formDescOne: '这些信息帮助我们判断资金路径和接入范围。',
        formTitleTwo: '把初步方案发给谁？',
        formDescTwo: '工作邮箱必填，Telegram 选填。我们不会进行批量营销。',
        businessType: '业务类型',
        businessPlaceholder: '选择最接近的一项',
        businessOptions: [
          ['ecommerce', '跨境电商'],
          ['saas_digital', 'SaaS / 数字产品'],
          ['forex_financial', '外汇 / 金融服务'],
          ['psp_marketplace', 'PSP / 平台型业务'],
          ['other', '其他'],
        ],
        monthlyVolume: '预计每月稳定币交易量',
        volumePlaceholder: '选择大致区间',
        volumeOptions: [
          ['under_50k', '低于 5 万美元'],
          ['50k_250k', '5 万–25 万美元'],
          ['250k_1m', '25 万–100 万美元'],
          ['above_1m', '100 万美元以上'],
          ['not_sure', '仍在评估'],
        ],
        networks: '目标网络',
        networkOptions: [
          ['tron', 'TRON'],
          ['solana', 'Solana'],
          ['ethereum', 'Ethereum'],
          ['bsc', 'BSC'],
          ['l2', 'Polygon / Arbitrum / Optimism / Base'],
          ['not_sure', '暂未确定'],
        ],
        needs: '需要的能力',
        needOptions: [
          ['checkout', '托管收银台'],
          ['payment_api', 'Payment API / SDK'],
          ['payouts', 'Payout API'],
          ['sub_merchants', '子商户 / 平台接入'],
          ['other', '其他或需要架构建议'],
        ],
        continue: '继续填写联系方式',
        companyName: '公司或项目名称',
        companyWebsite: '公司网站',
        optional: '选填',
        workEmail: '工作邮箱',
        telegram: 'Telegram',
        context: '还有哪些背景值得我们提前了解？',
        contextPlaceholder: '例如目标上线时间、当前支付方式、结算地区或特殊技术要求',
        back: '返回修改需求',
        submit: '提交接入需求',
        submitting: '正在提交…',
        privacyPrefix: '提交即表示你同意我们按照',
        privacyLink: '隐私政策',
        privacySuffix: '处理这些信息，仅用于评估和回复本次需求。',
        requiredError: '请完成所有必填项，并至少选择一个目标网络和接入能力。',
        genericError: '暂时无法提交。请稍后重试，或发送邮件至 support@ironixpay.com。',
        successEyebrow: '需求已收到',
        successTitle: '我们会先做判断，再联系你',
        successDesc:
          '团队会评估你的业务类型、交易规模和目标网络，并在 1 个工作日内通过工作邮箱或 Telegram 回复初步方案。',
        successNext: '回复会包含支付路径、建议接入范围和下一步安排。',
        returnHome: '返回首页',
      }
    : {
        eyebrow: 'Enterprise access',
        title: 'Map the money flow before you integrate',
        description:
          'For teams with meaningful volume, complex settlement paths, or requirements spanning payouts, sub-merchants, and multiple networks. We will turn your operating context into an initial integration plan.',
        responseLabel: 'What you will receive',
        responseItems: [
          ['Payment architecture', 'A recommended collection, sweep, and payout flow'],
          ['Integration scope', 'Suitable networks, assets, APIs, and risk controls'],
          ['Commercial outline', 'Indicative pricing, timeline, and next steps'],
        ],
        responseTime: 'A team member familiar with payment architecture replies within one business day.',
        stepOne: 'Business fit',
        stepTwo: 'Contact details',
        formTitleOne: 'Describe your payment operation',
        formDescOne: 'This helps us reason about the money flow and integration scope.',
        formTitleTwo: 'Where should we send the plan?',
        formDescTwo: 'Work email is required. Telegram is optional. No sales blasts.',
        businessType: 'Business type',
        businessPlaceholder: 'Choose the closest match',
        businessOptions: [
          ['ecommerce', 'Cross-border e-commerce'],
          ['saas_digital', 'SaaS / digital products'],
          ['forex_financial', 'Forex / financial services'],
          ['psp_marketplace', 'PSP / marketplace'],
          ['other', 'Other'],
        ],
        monthlyVolume: 'Expected monthly stablecoin volume',
        volumePlaceholder: 'Select an approximate range',
        volumeOptions: [
          ['under_50k', 'Under $50k'],
          ['50k_250k', '$50k–$250k'],
          ['250k_1m', '$250k–$1m'],
          ['above_1m', '$1m+'],
          ['not_sure', 'Still evaluating'],
        ],
        networks: 'Target networks',
        networkOptions: [
          ['tron', 'TRON'],
          ['solana', 'Solana'],
          ['ethereum', 'Ethereum'],
          ['bsc', 'BSC'],
          ['l2', 'Polygon / Arbitrum / Optimism / Base'],
          ['not_sure', 'Not decided yet'],
        ],
        needs: 'Capabilities needed',
        needOptions: [
          ['checkout', 'Hosted Checkout'],
          ['payment_api', 'Payment API / SDK'],
          ['payouts', 'Payout API'],
          ['sub_merchants', 'Sub-merchant / platform integration'],
          ['other', 'Other / architecture guidance'],
        ],
        continue: 'Continue to contact details',
        companyName: 'Company or project name',
        companyWebsite: 'Company website',
        optional: 'Optional',
        workEmail: 'Work email',
        telegram: 'Telegram',
        context: 'Anything else we should understand upfront?',
        contextPlaceholder: 'Target launch date, current payment setup, settlement regions, or technical constraints',
        back: 'Back to business fit',
        submit: 'Submit integration request',
        submitting: 'Submitting…',
        privacyPrefix: 'By submitting, you agree that we may process this information under our',
        privacyLink: 'Privacy Policy',
        privacySuffix: 'solely to evaluate and respond to this request.',
        requiredError: 'Complete all required fields and select at least one network and capability.',
        genericError: 'Unable to submit right now. Try again later or email support@ironixpay.com.',
        successEyebrow: 'Request received',
        successTitle: 'We will assess the operation before we reach out',
        successDesc:
          'The team will review your business model, expected volume, and target networks, then reply by work email or Telegram within one business day.',
        successNext: 'The reply will cover the payment flow, recommended integration scope, and next steps.',
        returnHome: 'Return home',
      },
)

const canContinue = computed(
  () =>
    form.businessType &&
    form.monthlyVolume &&
    form.networks.length > 0 &&
    form.integrationNeeds.length > 0,
)

function continueToContact() {
  if (!canContinue.value) {
    errorMessage.value = t.value.requiredError
    return
  }
  errorMessage.value = ''
  step.value = 2
  window.requestAnimationFrame(() => {
    document.querySelector<HTMLElement>('.ix-enterprise-form')?.focus()
  })
}

async function submitLead() {
  errorMessage.value = ''
  if (!canContinue.value || !form.companyName.trim() || !form.contactEmail.trim()) {
    errorMessage.value = t.value.requiredError
    return
  }

  submitting.value = true
  try {
    const apiBase = import.meta.env.VITE_API_BASE_URL || 'https://api.ironixpay.com'
    const response = await fetch(`${apiBase}/api/public/enterprise-leads`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        company_name: form.companyName,
        company_website: form.companyWebsite || null,
        contact_email: form.contactEmail,
        telegram: form.telegram || null,
        business_type: form.businessType,
        monthly_volume: form.monthlyVolume,
        networks: form.networks,
        integration_needs: form.integrationNeeds,
        message: form.message || null,
        locale: isZh.value ? 'zh' : 'en',
        fax_number: form.faxNumber,
      }),
    })

    if (!response.ok) {
      const payload = await response.json().catch(() => null)
      throw new Error(payload?.error?.message || t.value.genericError)
    }

    trackEvent('generate_lead', {
      form_name: 'enterprise_access',
      business_type: form.businessType,
      monthly_volume: form.monthlyVolume,
    })
    submitted.value = true
    window.scrollTo({ top: 0, behavior: 'smooth' })
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : t.value.genericError
  } finally {
    submitting.value = false
  }
}
</script>

<template>
  <main class="ix-enterprise-page">
    <section class="ix-enterprise-hero">
      <div class="ix-enterprise-hero__inner">
        <div>
          <span class="ix-enterprise-eyebrow">{{ t.eyebrow }}</span>
          <h1>{{ t.title }}</h1>
          <p>{{ t.description }}</p>
        </div>

        <div class="ix-enterprise-brief" aria-label="Response outline">
          <span>{{ t.responseLabel }}</span>
          <ol>
            <li v-for="(item, index) in t.responseItems" :key="item[0]">
              <b>{{ String(index + 1).padStart(2, '0') }}</b>
              <div>
                <strong>{{ item[0] }}</strong>
                <small>{{ item[1] }}</small>
              </div>
            </li>
          </ol>
          <p>{{ t.responseTime }}</p>
        </div>
      </div>
    </section>

    <section class="ix-enterprise-intake">
      <div v-if="!submitted" class="ix-enterprise-intake__inner">
        <aside class="ix-enterprise-intake__aside">
          <span>{{ isZh ? '接入评估' : 'Integration assessment' }}</span>
          <h2>{{ step === 1 ? t.formTitleOne : t.formTitleTwo }}</h2>
          <p>{{ step === 1 ? t.formDescOne : t.formDescTwo }}</p>

        </aside>

        <form class="ix-enterprise-form" tabindex="-1" @submit.prevent="submitLead">
          <div class="ix-enterprise-progress" aria-label="Form progress">
            <div
              :class="{ active: step === 1, complete: step > 1 }"
              :aria-current="step === 1 ? 'step' : undefined"
            >
              <b>01</b>
              <span>{{ t.stepOne }}</span>
            </div>
            <div :class="{ active: step === 2 }" :aria-current="step === 2 ? 'step' : undefined">
              <b>02</b>
              <span>{{ t.stepTwo }}</span>
            </div>
          </div>

          <div v-if="step === 1" class="ix-enterprise-form__step">
            <div class="ix-field-grid">
              <label class="ix-field">
                <span>{{ t.businessType }}</span>
                <select v-model="form.businessType" required>
                  <option value="" disabled>{{ t.businessPlaceholder }}</option>
                  <option v-for="option in t.businessOptions" :key="option[0]" :value="option[0]">
                    {{ option[1] }}
                  </option>
                </select>
              </label>

              <label class="ix-field">
                <span>{{ t.monthlyVolume }}</span>
                <select v-model="form.monthlyVolume" required>
                  <option value="" disabled>{{ t.volumePlaceholder }}</option>
                  <option v-for="option in t.volumeOptions" :key="option[0]" :value="option[0]">
                    {{ option[1] }}
                  </option>
                </select>
              </label>
            </div>

            <fieldset class="ix-choice-group">
              <legend>{{ t.networks }}</legend>
              <div class="ix-choice-grid">
                <label v-for="option in t.networkOptions" :key="option[0]" class="ix-choice">
                  <input v-model="form.networks" type="checkbox" :value="option[0]" />
                  <span>{{ option[1] }}</span>
                </label>
              </div>
            </fieldset>

            <fieldset class="ix-choice-group">
              <legend>{{ t.needs }}</legend>
              <div class="ix-choice-grid ix-choice-grid--needs">
                <label v-for="option in t.needOptions" :key="option[0]" class="ix-choice">
                  <input v-model="form.integrationNeeds" type="checkbox" :value="option[0]" />
                  <span>{{ option[1] }}</span>
                </label>
              </div>
            </fieldset>

            <p v-if="errorMessage" class="ix-form-error" role="alert">{{ errorMessage }}</p>
            <div class="ix-form-actions ix-form-actions--end">
              <button type="button" class="ix-button ix-button--primary" @click="continueToContact">
                {{ t.continue }}
                <span aria-hidden="true">→</span>
              </button>
            </div>
          </div>

          <div v-else class="ix-enterprise-form__step">
            <div class="ix-field-grid">
              <label class="ix-field">
                <span>{{ t.companyName }}</span>
                <input v-model="form.companyName" type="text" autocomplete="organization" maxlength="120" required />
              </label>

              <label class="ix-field">
                <span>{{ t.companyWebsite }} <small>{{ t.optional }}</small></span>
                <input v-model="form.companyWebsite" type="url" inputmode="url" autocomplete="url" maxlength="300" placeholder="https://" />
              </label>

              <label class="ix-field">
                <span>{{ t.workEmail }}</span>
                <input v-model="form.contactEmail" type="email" inputmode="email" autocomplete="email" maxlength="254" required />
              </label>

              <label class="ix-field">
                <span>{{ t.telegram }} <small>{{ t.optional }}</small></span>
                <input v-model="form.telegram" type="text" autocomplete="off" maxlength="100" placeholder="@username" />
              </label>
            </div>

            <label class="ix-field">
              <span>{{ t.context }} <small>{{ t.optional }}</small></span>
              <textarea v-model="form.message" rows="5" maxlength="1000" :placeholder="t.contextPlaceholder" />
              <small class="ix-field__count">{{ form.message.length }} / 1000</small>
            </label>

            <label class="ix-honeypot" aria-hidden="true">
              Fax number
              <input v-model="form.faxNumber" type="text" tabindex="-1" autocomplete="off" />
            </label>

            <p class="ix-form-privacy">
              {{ t.privacyPrefix }}
              <a :href="isZh ? '/privacy' : '/en/privacy'">{{ t.privacyLink }}</a>
              {{ t.privacySuffix }}
            </p>
            <p v-if="errorMessage" class="ix-form-error" role="alert">{{ errorMessage }}</p>

            <div class="ix-form-actions">
              <button type="button" class="ix-button ix-button--secondary" @click="step = 1">
                {{ t.back }}
              </button>
              <button type="submit" class="ix-button ix-button--primary" :disabled="submitting">
                {{ submitting ? t.submitting : t.submit }}
                <span v-if="!submitting" aria-hidden="true">→</span>
              </button>
            </div>
          </div>
        </form>
      </div>

      <div v-else class="ix-enterprise-success" role="status">
        <span class="ix-enterprise-success__mark" aria-hidden="true">✓</span>
        <span class="ix-enterprise-eyebrow">{{ t.successEyebrow }}</span>
        <h2>{{ t.successTitle }}</h2>
        <p>{{ t.successDesc }}</p>
        <small>{{ t.successNext }}</small>
        <a :href="isZh ? '/' : '/en/'">{{ t.returnHome }}</a>
      </div>
    </section>
  </main>
</template>

<style scoped>
.ix-enterprise-page {
  --ix-enterprise-accent: #2563eb;
  --ix-enterprise-accent-strong: #1d4ed8;
  --ix-enterprise-accent-soft: #eff6ff;
  --ix-enterprise-ink: #0f172a;
  --ix-enterprise-muted: #64748b;
  --ix-enterprise-border: #dbe3ef;
  background: #ffffff;
  color: var(--ix-enterprise-ink);
}

.ix-enterprise-hero {
  border-bottom: 1px solid #e2e8f0;
  background: #f8fafc;
}

.ix-enterprise-hero__inner {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(380px, 0.72fr);
  gap: 72px;
  align-items: center;
  max-width: 1140px;
  margin: 0 auto;
  padding: 88px 32px 80px;
}

.ix-enterprise-eyebrow {
  display: block;
  margin-bottom: 14px;
  color: var(--ix-enterprise-accent);
  font-family: 'Exo 2', sans-serif;
  font-size: 0.75rem;
  font-weight: 700;
  letter-spacing: 0.14em;
  text-transform: uppercase;
}

.ix-enterprise-hero h1 {
  max-width: 620px;
  margin: 0 0 20px;
  color: var(--ix-enterprise-ink);
  font-family: 'Exo 2', sans-serif;
  font-size: clamp(2.4rem, 4vw, 3.2rem);
  font-weight: 800;
  letter-spacing: 0;
  line-height: 1.12;
}

.ix-enterprise-hero__inner > div:first-child > p {
  max-width: 650px;
  margin: 0;
  color: #475569;
  font-size: 1rem;
  line-height: 1.7;
}

.ix-enterprise-brief {
  padding: 24px 26px;
  border: 1px solid rgba(15, 23, 42, 0.08);
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.88);
  box-shadow: 0 24px 54px rgba(15, 23, 42, 0.08);
}

.ix-enterprise-brief > span,
.ix-enterprise-intake__aside > span {
  color: #64748b;
  font-size: 0.72rem;
  font-weight: 750;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.ix-enterprise-brief ol {
  padding: 0;
  margin: 14px 0 18px;
  list-style: none;
}

.ix-enterprise-brief li {
  display: grid;
  grid-template-columns: 34px 1fr;
  gap: 12px;
  padding: 13px 0;
  border-bottom: 1px solid #e2e8f0;
}

.ix-enterprise-brief li b {
  color: #3b82f6;
  font-family: 'Exo 2', sans-serif;
  font-size: 0.72rem;
}

.ix-enterprise-brief li strong,
.ix-enterprise-brief li small {
  display: block;
}

.ix-enterprise-brief li strong {
  margin-bottom: 2px;
  color: #243044;
  font-size: 0.88rem;
}

.ix-enterprise-brief li small {
  color: #6a7689;
  font-size: 0.76rem;
  line-height: 1.5;
}

.ix-enterprise-brief > p {
  margin: 0;
  color: var(--ix-enterprise-accent-strong);
  font-size: 0.78rem;
  font-weight: 650;
  line-height: 1.5;
}

.ix-enterprise-intake {
  padding: 88px 32px 112px;
  background: #ffffff;
}

.ix-enterprise-intake__inner {
  display: grid;
  grid-template-columns: minmax(250px, 0.58fr) minmax(0, 1.42fr);
  gap: 64px;
  align-items: start;
  max-width: 1080px;
  margin: 0 auto;
}

.ix-enterprise-intake__aside {
  position: sticky;
  top: 100px;
  padding-top: 18px;
}

.ix-enterprise-intake__aside h2 {
  margin: 10px 0 12px;
  color: var(--ix-enterprise-ink);
  font-family: 'Exo 2', sans-serif;
  font-size: 1.8rem;
  letter-spacing: 0;
  line-height: 1.2;
}

.ix-enterprise-intake__aside > p {
  margin: 0;
  color: var(--ix-enterprise-muted);
  font-size: 0.9rem;
  line-height: 1.7;
}

.ix-enterprise-progress {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  padding-bottom: 24px;
  margin-bottom: 28px;
  border-bottom: 1px solid #e2e8f0;
}

.ix-enterprise-progress > div {
  display: flex;
  gap: 9px;
  align-items: center;
  min-height: 42px;
  padding: 0 12px;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  background: #f8fafc;
  color: #94a3b8;
  font-size: 0.82rem;
  transition: border-color 0.2s ease, background-color 0.2s ease, color 0.2s ease;
}

.ix-enterprise-progress b {
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: #e2e8f0;
  color: #64748b;
  font-family: 'Exo 2', sans-serif;
  font-size: 0.7rem;
}

.ix-enterprise-progress .active {
  border-color: #bfdbfe;
  background: var(--ix-enterprise-accent-soft);
  color: var(--ix-enterprise-accent-strong);
  font-weight: 700;
}

.ix-enterprise-progress .complete {
  border-color: #dbeafe;
  color: #475569;
}

.ix-enterprise-progress .active b,
.ix-enterprise-progress .complete b {
  background: var(--ix-enterprise-accent);
  color: #ffffff;
}

.ix-enterprise-form {
  box-sizing: border-box;
  min-width: 0;
  padding: 30px;
  border: 1px solid var(--ix-enterprise-border);
  border-radius: 16px;
  background: #ffffff;
  box-shadow: 0 18px 50px rgba(15, 23, 42, 0.07);
  outline: none;
}

.ix-enterprise-form__step {
  display: grid;
  gap: 28px;
}

.ix-field-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 18px;
}

.ix-field {
  position: relative;
  display: grid;
  gap: 8px;
  min-width: 0;
}

.ix-field > span,
.ix-choice-group legend {
  color: #334155;
  font-size: 0.8rem;
  font-weight: 700;
}

.ix-field > span small {
  margin-left: 4px;
  color: #98a2b3;
  font-weight: 500;
}

.ix-field input,
.ix-field select,
.ix-field textarea {
  box-sizing: border-box;
  width: 100%;
  border: 1px solid #cbd5e1;
  border-radius: 10px;
  background: #ffffff;
  color: #182230;
  font: inherit;
  font-size: 0.88rem;
  letter-spacing: 0;
  outline: none;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.ix-field input,
.ix-field select {
  height: 48px;
  padding: 0 14px;
}

.ix-field textarea {
  min-height: 118px;
  padding: 12px;
  resize: vertical;
  line-height: 1.55;
}

.ix-field input:focus,
.ix-field select:focus,
.ix-field textarea:focus {
  border-color: #3b82f6;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.14);
}

.ix-field__count {
  position: absolute;
  right: 10px;
  bottom: 8px;
  color: #98a2b3;
  font-size: 0.68rem;
}

.ix-choice-group {
  min-width: 0;
  padding: 0;
  margin: 0;
  border: 0;
}

.ix-choice-group legend {
  margin-bottom: 10px;
}

.ix-choice-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.ix-choice-grid--needs {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.ix-choice {
  position: relative;
  display: flex;
  align-items: center;
  min-height: 46px;
  padding: 0 13px;
  border: 1px solid #dbe3ef;
  border-radius: 10px;
  background: #f8fafc;
  color: #475569;
  cursor: pointer;
  font-size: 0.8rem;
  line-height: 1.35;
  transition: border-color 0.15s ease, background-color 0.15s ease, color 0.15s ease;
}

.ix-choice:has(input:checked) {
  border-color: #93c5fd;
  background: var(--ix-enterprise-accent-soft);
  color: var(--ix-enterprise-accent-strong);
  font-weight: 650;
}

.ix-choice:focus-within {
  border-color: #3b82f6;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.12);
}

.ix-choice input {
  width: 15px;
  height: 15px;
  margin: 0 9px 0 0;
  accent-color: var(--ix-enterprise-accent);
  flex: 0 0 auto;
}

.ix-form-actions {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  padding-top: 4px;
}

.ix-form-actions--end {
  justify-content: flex-end;
}

.ix-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  min-height: 48px;
  padding: 0 22px;
  border-radius: 12px;
  font: inherit;
  font-size: 0.88rem;
  font-weight: 700;
  letter-spacing: 0;
  cursor: pointer;
  transition: transform 0.2s ease, border-color 0.2s ease, background-color 0.2s ease, box-shadow 0.2s ease;
}

.ix-button--primary {
  border: 1px solid var(--ix-enterprise-accent);
  background: linear-gradient(135deg, #2563eb, #3b82f6);
  color: #ffffff;
  box-shadow: 0 4px 18px rgba(37, 99, 235, 0.28);
}

.ix-button--primary:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 8px 24px rgba(37, 99, 235, 0.36);
}

.ix-button--primary:disabled {
  cursor: wait;
  opacity: 0.65;
}

.ix-button--secondary {
  border: 1px solid #cbd5e1;
  background: #ffffff;
  color: #334155;
}

.ix-button--secondary:hover {
  transform: translateY(-1px);
  border-color: #94a3b8;
  background: #f8fafc;
}

.ix-button:focus-visible {
  outline: 3px solid rgba(37, 99, 235, 0.28);
  outline-offset: 2px;
}

.ix-form-error {
  margin: -10px 0 0;
  padding: 10px 12px;
  border-left: 3px solid #c2413b;
  background: #fff5f4;
  color: #9b2c26;
  font-size: 0.78rem;
  line-height: 1.5;
}

.ix-form-privacy {
  margin: -8px 0 0;
  color: #7b8798;
  font-size: 0.7rem;
  line-height: 1.55;
}

.ix-form-privacy a {
  color: var(--ix-enterprise-accent-strong);
  font-weight: 650;
}

.ix-honeypot {
  position: absolute;
  left: -10000px;
  width: 1px;
  height: 1px;
  overflow: hidden;
}

.ix-enterprise-success {
  max-width: 720px;
  margin: 0 auto;
  padding: 76px 24px 20px;
  text-align: center;
}

.ix-enterprise-success__mark {
  display: grid;
  place-items: center;
  width: 46px;
  height: 46px;
  margin: 0 auto 20px;
  border: 1px solid #bfdbfe;
  border-radius: 50%;
  background: var(--ix-enterprise-accent-soft);
  color: var(--ix-enterprise-accent);
  font-size: 1.2rem;
  font-weight: 800;
}

.ix-enterprise-success h2 {
  margin: 0 0 16px;
  color: #101828;
  font-family: 'Exo 2', sans-serif;
  font-size: clamp(2rem, 5vw, 3.2rem);
  letter-spacing: 0;
  line-height: 1.1;
}

.ix-enterprise-success p {
  margin: 0 auto 12px;
  color: #526078;
  font-size: 1rem;
  line-height: 1.7;
}

.ix-enterprise-success small {
  display: block;
  color: #7b8798;
  font-size: 0.78rem;
}

.ix-enterprise-success a {
  display: inline-flex;
  margin-top: 28px;
  color: var(--ix-enterprise-accent-strong);
  font-size: 0.84rem;
  font-weight: 700;
}

.dark .ix-enterprise-page,
.dark .ix-enterprise-intake {
  --ix-enterprise-accent: #3b82f6;
  --ix-enterprise-accent-strong: #93c5fd;
  --ix-enterprise-accent-soft: rgba(37, 99, 235, 0.14);
  --ix-enterprise-ink: #f8fafc;
  --ix-enterprise-muted: #94a3b8;
  --ix-enterprise-border: rgba(148, 163, 184, 0.2);
  background: #0a0f1d;
  color: #e5e7eb;
}

.dark .ix-enterprise-hero {
  border-color: rgba(148, 163, 184, 0.14);
  background: #0f172a;
}

.dark .ix-enterprise-hero h1,
.dark .ix-enterprise-intake__aside h2,
.dark .ix-enterprise-success h2 {
  color: #f8fafc;
}

.dark .ix-enterprise-hero__inner > div:first-child > p,
.dark .ix-enterprise-intake__aside > p,
.dark .ix-enterprise-success p {
  color: #a8b3c5;
}

.dark .ix-enterprise-brief,
.dark .ix-enterprise-brief li,
.dark .ix-enterprise-progress,
.dark .ix-enterprise-progress > div {
  border-color: rgba(148, 163, 184, 0.18);
}

.dark .ix-enterprise-brief {
  background: rgba(15, 23, 42, 0.78);
  box-shadow: 0 24px 54px rgba(0, 0, 0, 0.24);
}

.dark .ix-enterprise-brief li strong,
.dark .ix-field > span,
.dark .ix-choice-group legend {
  color: #e2e8f0;
}

.dark .ix-enterprise-brief li small,
.dark .ix-enterprise-brief > span,
.dark .ix-enterprise-intake__aside > span {
  color: #94a3b8;
}

.dark .ix-enterprise-form {
  border-color: rgba(148, 163, 184, 0.18);
  background: #111827;
  box-shadow: none;
}

.dark .ix-enterprise-progress > div {
  background: #172033;
  color: #94a3b8;
}

.dark .ix-enterprise-progress .active,
.dark .ix-enterprise-progress .complete {
  border-color: rgba(96, 165, 250, 0.42);
  background: rgba(37, 99, 235, 0.14);
  color: #bfdbfe;
}

.dark .ix-field input,
.dark .ix-field select,
.dark .ix-field textarea,
.dark .ix-button--secondary {
  border-color: #3a485c;
  background: #172033;
  color: #e5e7eb;
}

.dark .ix-choice {
  border-color: #344154;
  background: #151e2e;
  color: #b8c2d1;
}

.dark .ix-choice:has(input:checked) {
  border-color: rgba(96, 165, 250, 0.55);
  background: rgba(37, 99, 235, 0.16);
  color: #bfdbfe;
}

.dark .ix-form-error {
  background: #321b1c;
  color: #fca5a5;
}

@media (max-width: 900px) {
  .ix-enterprise-hero__inner,
  .ix-enterprise-intake__inner {
    grid-template-columns: 1fr;
  }

  .ix-enterprise-hero__inner {
    gap: 48px;
    padding: 76px 24px 64px;
  }

  .ix-enterprise-intake {
    padding: 64px 24px 88px;
  }

  .ix-enterprise-intake__inner {
    gap: 32px;
  }

  .ix-enterprise-intake__aside {
    position: static;
  }

  .ix-enterprise-progress {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
}

@media (max-width: 620px) {
  .ix-enterprise-hero__inner {
    gap: 32px;
    padding: 48px 24px 44px;
  }

  .ix-enterprise-hero h1 {
    margin-bottom: 16px;
    font-size: 2.25rem;
    line-height: 1.1;
  }

  .ix-enterprise-hero__inner > div:first-child > p {
    font-size: 0.95rem;
    line-height: 1.65;
  }

  .ix-enterprise-brief {
    padding: 18px;
    border-radius: 14px;
  }

  .ix-enterprise-brief ol {
    margin: 10px 0 12px;
  }

  .ix-enterprise-brief li {
    grid-template-columns: 28px 1fr;
    gap: 8px;
    padding: 9px 0;
  }

  .ix-enterprise-brief li small {
    font-size: 0.72rem;
    line-height: 1.4;
  }

  .ix-enterprise-brief > p {
    font-size: 0.74rem;
  }

  .ix-enterprise-intake {
    padding: 48px 16px 72px;
  }

  .ix-enterprise-form {
    padding: 20px 16px;
  }

  .ix-field-grid,
  .ix-choice-grid,
  .ix-choice-grid--needs {
    grid-template-columns: 1fr;
  }

  .ix-form-actions {
    align-items: stretch;
    flex-direction: column-reverse;
  }

  .ix-form-actions--end {
    flex-direction: column;
  }

  .ix-button {
    width: 100%;
  }
}

@media (prefers-reduced-motion: reduce) {
  .ix-enterprise-progress > div,
  .ix-field input,
  .ix-field select,
  .ix-field textarea,
  .ix-choice,
  .ix-button {
    transition: none;
  }

  .ix-button:hover {
    transform: none;
  }
}
</style>
