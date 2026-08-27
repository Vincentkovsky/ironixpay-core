<script setup>
import DefaultTheme from 'vitepress/theme'
import { useData } from 'vitepress'
import CustomHero from './CustomHero.vue'
import NetworkStrip from './NetworkStrip.vue'
import HowItWorks from './HowItWorks.vue'
import CodePreview from './CodePreview.vue'
import FeaturesSection from './FeaturesSection.vue'
import HomePricing from './HomePricing.vue'
import EnterpriseAccess from './EnterpriseAccess.vue'
import SecurityBanner from './SecurityBanner.vue'
import InfraPartners from './InfraPartners.vue'
import CustomFooter from './CustomFooter.vue'
import GeoRestrictionModal from './GeoRestrictionModal.vue'
import AnalyticsConsent from './AnalyticsConsent.vue'

const { Layout } = DefaultTheme
const { frontmatter } = useData()
</script>

<template>
  <Layout>
    <template #home-hero-before>
      <CustomHero v-if="frontmatter.customHero" />
      <NetworkStrip v-if="frontmatter.customHero" />
      <HowItWorks v-if="frontmatter.customHero" />
      <CodePreview v-if="frontmatter.customHero" />
      <FeaturesSection v-if="frontmatter.customHero" />
      <HomePricing v-if="frontmatter.customHero" />
      <EnterpriseAccess v-if="frontmatter.customHero" />
      <SecurityBanner v-if="frontmatter.customHero" />
      <InfraPartners v-if="frontmatter.customHero" />
    </template>

    <template #layout-bottom>
      <CustomFooter />
      <GeoRestrictionModal />
      <AnalyticsConsent />
    </template>
  </Layout>
</template>
