import { createI18n } from 'vue-i18n';
import en from './en-US';
import cn from './zh-CN';

function detectLocale(): string {
    const saved = localStorage.getItem('app-locale');
    if (saved && ['en-US', 'zh-CN'].includes(saved)) return saved;

    const browserLang = navigator.language || (navigator as any).userLanguage || '';
    if (browserLang.startsWith('zh')) return 'zh-CN';

    return 'en-US';
}

const i18n = createI18n({
    locale: detectLocale(),
    fallbackLocale: 'en-US',
    legacy: false,
    allowComposition: true,
    messages: {
        'en-US': en,
        'zh-CN': cn,
    },
});

export default i18n;
