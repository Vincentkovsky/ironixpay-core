import DefaultTheme from 'vitepress/theme'
import './custom.css'
import { theme, useOpenapi, useTheme } from 'vitepress-openapi/client'
import 'vitepress-openapi/dist/style.css'
import CustomLayout from './components/CustomLayout.vue'
import PricingTable from './components/PricingTable.vue'
import FlowChart from './components/FlowChart.vue'
import FlowStep from './components/FlowStep.vue'
import CheckoutPage from './components/CheckoutPage.vue'
import PayoutsPage from './components/PayoutsPage.vue'
import EnterprisePage from './components/EnterprisePage.vue'
import { installAnalyticsClickTracking, schedulePageView } from './analytics'

const LANG_PREF_KEY = 'ironixpay-lang-pref'

/**
 * Track MANUAL language switches only (not initial page load).
 * Only writes to localStorage when the locale actually changes
 * (e.g. user clicks the language switcher: /en/ ↔ /).
 */
let prevLocale: string | null = null

function getLocaleFromPath(path: string): 'en' | 'zh' {
    return path.startsWith('/en/') || path === '/en' ? 'en' : 'zh'
}

function trackLanguageSwitch(path: string) {
    if (typeof window === 'undefined') return
    const locale = getLocaleFromPath(path)
    if (prevLocale !== null && prevLocale !== locale) {
        // Real language switch detected — store preference
        localStorage.setItem(LANG_PREF_KEY, locale)
    }
    prevLocale = locale
}

export default {
    extends: DefaultTheme,
    Layout: CustomLayout,
    async enhanceApp(ctx: any) {
        const { app, router } = ctx

        // Dynamic import: Vite splits the 30KB spec into its own async chunk,
        // so it is NOT inlined into the main app bundle. Non-API pages never
        // load this chunk. During SSG, the dynamic import resolves synchronously
        // (bundled), so SSR hydration works correctly.
        const { default: spec } = await import('../../public/openapi.json')

        const initialOpenapiLocale = getLocaleFromPath(
            typeof window === 'undefined' ? router.route.path : window.location.pathname
        )
        const openapiTheme = useTheme({
            i18n: {
                locale: initialOpenapiLocale,
                fallbackLocale: initialOpenapiLocale,
            },
        })

        useOpenapi({
            spec,
        })

        // Register vitepress-openapi theme components (OAOperation, OASpec, etc.)
        theme.enhanceApp(ctx)

        // Register custom IronixPay components
        app.component('PricingTable', PricingTable)
        app.component('FlowChart', FlowChart)
        app.component('Step', FlowStep)
        app.component('CheckoutPage', CheckoutPage)
        app.component('PayoutsPage', PayoutsPage)
        app.component('EnterprisePage', EnterprisePage)

        if (typeof document !== 'undefined') {
            // Set initial locale without storing (avoids overwriting head script)
            prevLocale = getLocaleFromPath(window.location.pathname)
            installAnalyticsClickTracking()
            schedulePageView(window.location.pathname)

            router.onAfterRouteChanged = (to: string) => {
                const el = document.getElementById('VPContent')
                if (el) el.setAttribute('role', 'main')
                const locale = getLocaleFromPath(to)
                openapiTheme.setI18nConfig({
                    locale,
                    fallbackLocale: locale,
                })
                trackLanguageSwitch(to)
                schedulePageView(to)
            }
        }
    },
}
