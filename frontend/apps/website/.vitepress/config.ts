import { defineConfig } from 'vitepress'
import { useSidebar } from 'vitepress-openapi'
import spec from '../public/openapi.json'

// Generate API Reference sidebar from OpenAPI spec (English only; Chinese uses hand-crafted translations)
const openApiSidebarEn = useSidebar({ spec, linkPrefix: '/en/api/operations/' })

export default defineConfig({
    vite: {
        server: { port: 3004 },
    },
    title: 'IronixPay',
    description: 'USDT/USDC 支付网关，支持 TRON、Solana、BSC、ETH 等 8 条链',

    sitemap: {
        hostname: 'https://ironixpay.com',
    },

    head: [
        ['script', {}, `(function(w,d,id,key){
  w.dataLayer=w.dataLayer||[];
  w.gtag=w.gtag||function(){w.dataLayer.push(arguments)};
  w.gtag('consent','default',{
    analytics_storage:'denied',
    ad_storage:'denied',
    ad_user_data:'denied',
    ad_personalization:'denied',
    security_storage:'granted',
    wait_for_update:500
  });
  w.__ironixPayLoadAnalytics=function(){
    if(w.__ironixPayGaLoaded)return;
    w.__ironixPayGaLoaded=true;
    w.gtag('consent','update',{analytics_storage:'granted'});
    w.gtag('js',new Date());
    w.gtag('config',id,{
      send_page_view:false,
      allow_google_signals:false,
      allow_ad_personalization_signals:false
    });
    var s=d.createElement('script');
    s.async=true;
    s.src='https://www.googletagmanager.com/gtag/js?id='+encodeURIComponent(id);
    d.head.appendChild(s);
  };
  try{if(w.localStorage.getItem(key)==='granted')w.__ironixPayLoadAnalytics()}catch(e){}
})(window,document,'G-HDPR8XVM02','ironixpay-analytics-consent')`],
        ['link', { rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' }],
        ['link', { rel: 'preconnect', href: 'https://fonts.googleapis.com' }],
        ['link', { rel: 'preconnect', href: 'https://fonts.gstatic.com', crossorigin: '' }],
        // Language routing: handles stored preference AND auto-detection.
        // Also handles legacy /zh/ URLs (redirect to root).
        ['script', {}, `(function(){
  var p=location.pathname;
  if(p.startsWith('/zh/')||p==='/zh'){location.replace(p.replace(/^\\/zh/,'') || '/');return}
  var K='ironixpay-lang-pref',s=localStorage.getItem(K);
  var onEn=p.startsWith('/en/')||p==='/en';
  if(s==='zh')return;
  if(s==='en'){if(!onEn){location.replace('/en'+(p==='/'?'/':p))}return}
  if(!onEn){
    var l=(navigator.language||'en').toLowerCase();
    if(!l.startsWith('zh')){
      localStorage.setItem(K,'en');
      location.replace('/en'+(p==='/'?'/':p));
    }
  }
})()`],
        // Open Graph
        ['meta', { property: 'og:type', content: 'website' }],
        ['meta', { property: 'og:title', content: 'IronixPay — Crypto Payment Gateway' }],
        ['meta', { property: 'og:description', content: 'Accept USDT & USDC payments across 8 chains — TRON, Solana, BSC, ETH, Polygon, Arbitrum, Optimism, and Base. Low fees, near-instant settlement, auto-sweeping to your treasury.' }],
        ['meta', { property: 'og:url', content: 'https://ironixpay.com' }],
        // Twitter Card
        ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
        ['meta', { name: 'twitter:site', content: '@IronixPay' }],
        ['meta', { name: 'twitter:title', content: 'IronixPay — Crypto Payment Gateway' }],
        ['meta', { name: 'twitter:description', content: 'Accept USDT & USDC payments across 8 chains — TRON, Solana, BSC, ETH, Polygon, Arbitrum, Optimism, and Base. Low fees, near-instant settlement, auto-sweeping to your treasury.' }],
    ],

    // Clean URLs without .html extension
    cleanUrls: true,

    // Dynamic page title/description for API operation pages
    transformPageData(pageData) {
        if (pageData.params?.pageTitle) {
            pageData.title = pageData.params.pageTitle
            pageData.titleTemplate = ':title | IronixPay API'
        }
        if (pageData.params?.pageDescription) {
            pageData.description = pageData.params.pageDescription
        }
    },

    locales: {
        // Chinese as root (default language)
        root: {
            label: '中文',
            lang: 'zh-CN',
            themeConfig: {
                nav: [
                    {
                        text: '产品',
                        items: [
                            { text: '收款 Checkout', link: '/checkout' },
                            { text: '出款 Payouts', link: '/payouts' },
                            { text: '支持的网络', link: '/guide/networks' },
                            { text: '定价', link: '/pricing' },
                            { text: '企业接入', link: '/enterprise' },
                            { text: '在线体验', link: '/demo' },
                        ],
                    },
                    { text: '开发者指南', link: '/guide/quickstart' },
                    { text: '使用场景', link: '/use-cases/telegram-bot' },
                    { text: 'API 参考', link: '/api/operations/create_session' },
                    { text: '控制台', link: 'https://app.ironixpay.com', target: '_blank' },
                ],
                sidebar: {
                    '/guide/': [
                        {
                            text: '快速上手',
                            items: [
                                { text: '快速开始', link: '/guide/quickstart' },
                                { text: '支持的网络', link: '/guide/networks' },
                                { text: '前端集成', link: '/guide/integration' },
                                { text: '身份认证', link: '/guide/authentication' },
                                { text: '测试指南', link: '/guide/testing' },
                            ],
                        },
                        {
                            text: '核心概念',
                            items: [
                                { text: 'Checkout Sessions', link: '/guide/checkout' },
                                { text: 'Payouts', link: '/guide/payouts' },
                                { text: 'Webhooks', link: '/guide/webhooks' },
                                { text: '错误处理', link: '/guide/errors' },
                                { text: '幂等性', link: '/guide/idempotency' },
                                { text: '异常支付', link: '/guide/exceptions' },
                            ],
                        },
                    ],
                    '/use-cases/': [
                        {
                            text: '集成方案',
                            items: [
                                { text: 'Telegram Bot', link: '/use-cases/telegram-bot' },
                                { text: 'WooCommerce', link: '/use-cases/woocommerce' },
                                { text: 'Next.js / React', link: '/use-cases/nextjs' },
                            ],
                        },
                        {
                            text: '行业场景',
                            items: [
                                { text: '外汇 Forex', link: '/use-cases/forex' },
                                { text: '跨境电商', link: '/use-cases/ecommerce' },
                                { text: 'PSP 与聚合平台', link: '/use-cases/psp-marketplace' },
                            ],
                        },
                    ],
                    '/api/': [
                        {
                            text: 'API 参考',
                            items: [
                                { text: '概览', link: '/api/' },
                            ],
                        },
                        {
                            text: '收银台会话',
                            items: [
                                { text: '创建收银台会话', link: '/api/operations/create_session' },
                                { text: '获取收银台会话', link: '/api/operations/get_session' },
                                { text: '列出收银台会话', link: '/api/operations/list_sessions' },
                            ],
                        },
                        {
                            text: '出款',
                            items: [
                                { text: '创建出款', link: '/api/operations/create_payout' },
                                { text: '获取出款详情', link: '/api/operations/get_payout' },
                                { text: '列出出款记录', link: '/api/operations/list_payouts' },
                            ],
                        },
                        {
                            text: '子商户',
                            items: [
                                { text: '创建子商户', link: '/api/operations/create_sub_merchant' },
                                { text: '获取子商户详情', link: '/api/operations/get_sub_merchant' },
                                { text: '列出子商户', link: '/api/operations/list_sub_merchants' },
                                { text: '更新子商户', link: '/api/operations/update_sub_merchant' },
                            ],
                        },
                    ],
                },
                outline: { label: '页面导航' },
                docFooter: { prev: '上一页', next: '下一页' },
                lastUpdated: { text: '最后更新于' },
            },
        },

        // English
        en: {
            label: 'English',
            lang: 'en',
            title: 'IronixPay',
            description: 'USDT & USDC payment gateway — TRON, Solana, BSC, ETH, Polygon, Arbitrum, Optimism, Base',
            themeConfig: {
                nav: [
                    {
                        text: 'Products',
                        items: [
                            { text: 'Checkout', link: '/en/checkout' },
                            { text: 'Payouts', link: '/en/payouts' },
                            { text: 'Supported Networks', link: '/en/guide/networks' },
                            { text: 'Pricing', link: '/en/pricing' },
                            { text: 'Enterprise', link: '/en/enterprise' },
                            { text: 'Live Demo', link: '/en/demo' },
                        ],
                    },
                    { text: 'Developer Guide', link: '/en/guide/quickstart' },
                    { text: 'Use Cases', link: '/en/use-cases/telegram-bot' },
                    { text: 'API Reference', link: '/en/api/operations/create_session' },
                    { text: 'Dashboard', link: 'https://app.ironixpay.com', target: '_blank' },
                ],
                sidebar: {
                    '/en/guide/': [
                        {
                            text: 'Getting Started',
                            items: [
                                { text: 'Quick Start', link: '/en/guide/quickstart' },
                                { text: 'Supported Networks', link: '/en/guide/networks' },
                                { text: 'Frontend Integration', link: '/en/guide/integration' },
                                { text: 'Authentication', link: '/en/guide/authentication' },
                                { text: 'Testing', link: '/en/guide/testing' },
                            ],
                        },
                        {
                            text: 'Core Concepts',
                            items: [
                                { text: 'Checkout Sessions', link: '/en/guide/checkout' },
                                { text: 'Payouts', link: '/en/guide/payouts' },
                                { text: 'Webhooks', link: '/en/guide/webhooks' },
                                { text: 'Errors', link: '/en/guide/errors' },
                                { text: 'Idempotency', link: '/en/guide/idempotency' },
                                { text: 'Payment Exceptions', link: '/en/guide/exceptions' },
                            ],
                        },
                    ],
                    '/en/use-cases/': [
                        {
                            text: 'Integrations',
                            items: [
                                { text: 'Telegram Bot', link: '/en/use-cases/telegram-bot' },
                                { text: 'WooCommerce', link: '/en/use-cases/woocommerce' },
                                { text: 'Next.js / React', link: '/en/use-cases/nextjs' },
                            ],
                        },
                        {
                            text: 'Industries',
                            items: [
                                { text: 'Forex Brokers', link: '/en/use-cases/forex' },
                                { text: 'E-commerce', link: '/en/use-cases/ecommerce' },
                                { text: 'PSP & Marketplace', link: '/en/use-cases/psp-marketplace' },
                            ],
                        },
                    ],
                    '/en/api/': [
                        {
                            text: 'API Reference',
                            items: [
                                { text: 'Overview', link: '/en/api/' },
                            ],
                        },
                        ...openApiSidebarEn.generateSidebarGroups(),
                    ],
                },
            },
        },
    },

    themeConfig: {
        siteTitle: false,
        logo: {
            light: '/logo.svg',
            dark: '/logo-white.svg',
            alt: 'IronixPay',
        },
        socialLinks: [
            { icon: 'x', link: 'https://x.com/IronixPay', ariaLabel: 'Follow IronixPay on X (Twitter)' },
            { icon: { svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M20.665 3.717l-17.73 6.837c-1.21.486-1.203 1.161-.222 1.462l4.552 1.42 10.532-6.645c.498-.303.953-.14.579.192l-8.533 7.701h-.002l.002.001-.314 4.692c.46 0 .663-.211.921-.46l2.211-2.15 4.599 3.397c.848.467 1.457.227 1.668-.785l3.019-14.228c.309-1.239-.473-1.8-1.282-1.434z"/></svg>' }, link: 'https://t.me/ironixpay', ariaLabel: 'Contact us on Telegram' },
        ],
        search: {
            provider: 'algolia',
            options: {
                appId: 'J47W6DAG42',
                apiKey: '3b1e351f75d00a01a0455e350a47a857',
                indexName: 'Guide website',
                askAi: {
                    assistantId: 'dN69M91PQSRq',
                },
                locales: {
                    root: {
                        placeholder: '搜索文档',
                        translations: {
                            button: {
                                buttonText: '搜索文档',
                                buttonAriaLabel: '搜索文档',
                            },
                            modal: {
                                searchBox: {
                                    clearButtonTitle: '清除查询条件',
                                    clearButtonAriaLabel: '清除查询条件',
                                    closeButtonText: '关闭',
                                    closeButtonAriaLabel: '关闭',
                                    placeholderText: '搜索文档',
                                    placeholderTextAskAi: '向 AI 提问：',
                                    placeholderTextAskAiStreaming: '回答中...',
                                    searchInputLabel: '搜索',
                                    backToKeywordSearchButtonText: '返回关键字搜索',
                                    backToKeywordSearchButtonAriaLabel: '返回关键字搜索',
                                },
                                startScreen: {
                                    recentSearchesTitle: '搜索历史',
                                    noRecentSearchesText: '没有搜索历史',
                                    saveRecentSearchButtonTitle: '保存至搜索历史',
                                    removeRecentSearchButtonTitle: '从搜索历史中移除',
                                    favoriteSearchesTitle: '收藏',
                                    removeFavoriteSearchButtonTitle: '从收藏中移除',
                                    recentConversationsTitle: '最近的对话',
                                    removeRecentConversationButtonTitle: '从历史记录中删除对话',
                                },
                                errorScreen: {
                                    titleText: '无法获取结果',
                                    helpText: '你可能需要检查你的网络连接',
                                },
                                noResultsScreen: {
                                    noResultsText: '无法找到相关结果',
                                    suggestedQueryText: '你可以尝试查询',
                                    reportMissingResultsText: '你认为该查询应该有结果？',
                                    reportMissingResultsLinkText: '点击反馈',
                                },
                                resultsScreen: {
                                    askAiPlaceholder: '向 AI 提问：',
                                },
                                askAiScreen: {
                                    disclaimerText: '答案由 AI 生成，可能不准确，请自行验证。',
                                    relatedSourcesText: '相关来源',
                                    thinkingText: '思考中...',
                                    copyButtonText: '复制',
                                    copyButtonCopiedText: '已复制！',
                                    copyButtonTitle: '复制',
                                    likeButtonTitle: '赞',
                                    dislikeButtonTitle: '踩',
                                    thanksForFeedbackText: '感谢你的反馈！',
                                    preToolCallText: '搜索中...',
                                    duringToolCallText: '搜索 ',
                                    afterToolCallText: '已搜索',
                                },
                                footer: {
                                    selectText: '选择',
                                    submitQuestionText: '提交问题',
                                    selectKeyAriaLabel: 'Enter 键',
                                    navigateText: '切换',
                                    navigateUpKeyAriaLabel: '向上箭头',
                                    navigateDownKeyAriaLabel: '向下箭头',
                                    closeText: '关闭',
                                    backToSearchText: '返回搜索',
                                    closeKeyAriaLabel: 'Esc 键',
                                    poweredByText: '搜索提供者',
                                },
                            },
                        },
                    },
                },
            },
        },
    },
})
