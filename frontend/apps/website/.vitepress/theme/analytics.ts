export const GA_MEASUREMENT_ID = 'G-HDPR8XVM02'
export const ANALYTICS_CONSENT_KEY = 'ironixpay-analytics-consent'
export const ANALYTICS_PREFERENCES_EVENT = 'ironixpay:open-analytics-preferences'

export type AnalyticsConsent = 'granted' | 'denied'
type AnalyticsParams = Record<string, string | number | boolean>

declare global {
  interface Window {
    dataLayer?: unknown[]
    gtag?: (...args: unknown[]) => void
    __ironixPayLoadAnalytics?: () => void
  }
}

let clickTrackingInstalled = false

export function getAnalyticsConsent(): AnalyticsConsent | null {
  if (typeof window === 'undefined') return null

  try {
    const value = window.localStorage.getItem(ANALYTICS_CONSENT_KEY)
    return value === 'granted' || value === 'denied' ? value : null
  } catch {
    return null
  }
}

function clearAnalyticsCookies() {
  if (typeof document === 'undefined') return

  const hostnameParts = window.location.hostname.split('.')
  const domains = ['', window.location.hostname]

  if (hostnameParts.length > 1) {
    domains.push(`.${hostnameParts.slice(-2).join('.')}`)
  }

  document.cookie.split(';').forEach((cookie) => {
    const name = cookie.split('=')[0]?.trim()
    if (!name || !/^_ga(?:_|$)|^_gid$|^_gat(?:_|$)/.test(name)) return

    domains.forEach((domain) => {
      const domainAttribute = domain ? `; domain=${domain}` : ''
      document.cookie = `${name}=; Max-Age=0; path=/${domainAttribute}; SameSite=Lax`
    })
  })
}

export function setAnalyticsConsent(consent: AnalyticsConsent) {
  if (typeof window === 'undefined') return

  try {
    window.localStorage.setItem(ANALYTICS_CONSENT_KEY, consent)
  } catch {
    // Consent still applies to the current page when storage is unavailable.
  }

  window.gtag?.('consent', 'update', {
    analytics_storage: consent,
    ad_storage: 'denied',
    ad_user_data: 'denied',
    ad_personalization: 'denied',
  })

  if (consent === 'granted') {
    window.__ironixPayLoadAnalytics?.()
    schedulePageView()
  } else {
    clearAnalyticsCookies()
  }
}

function getSafePagePath(path = window.location.pathname) {
  try {
    return new URL(path, window.location.origin).pathname
  } catch {
    return window.location.pathname
  }
}

function getLocale(path: string) {
  return path === '/en' || path.startsWith('/en/') ? 'en' : 'zh'
}

export function trackPageView(path?: string) {
  if (typeof window === 'undefined' || getAnalyticsConsent() !== 'granted') return

  const pagePath = getSafePagePath(path)
  window.gtag?.('event', 'page_view', {
    page_title: document.title,
    page_location: `${window.location.origin}${pagePath}`,
    page_path: pagePath,
    locale: getLocale(pagePath),
  })
}

export function schedulePageView(path?: string) {
  if (typeof window === 'undefined') return
  window.requestAnimationFrame(() => trackPageView(path))
}

export function trackEvent(eventName: string, params: AnalyticsParams = {}) {
  if (typeof window === 'undefined' || getAnalyticsConsent() !== 'granted') return

  const pagePath = getSafePagePath()
  window.gtag?.('event', eventName, {
    ...params,
    page_path: pagePath,
    locale: getLocale(pagePath),
  })
}

export function installAnalyticsClickTracking() {
  if (typeof document === 'undefined' || clickTrackingInstalled) return
  clickTrackingInstalled = true

  document.addEventListener('click', (event) => {
    const target = event.target
    if (!(target instanceof Element)) return

    const trackedElement = target.closest<HTMLElement>('[data-analytics-event]')
    if (!trackedElement) return

    const eventName = trackedElement.dataset.analyticsEvent
    if (!eventName) return

    trackEvent(eventName, {
      cta_name: trackedElement.dataset.analyticsName || 'unknown',
      cta_location: trackedElement.dataset.analyticsLocation || 'unknown',
    })
  })
}
