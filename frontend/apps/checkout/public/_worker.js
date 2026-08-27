// Redirect *.pages.dev traffic to custom domain
const CUSTOM_DOMAIN = 'pay.ironixpay.com';

export default {
    async fetch(request, env) {
        const url = new URL(request.url);
        if (url.hostname.endsWith('.pages.dev')) {
            return Response.redirect(
                `https://${CUSTOM_DOMAIN}${url.pathname}${url.search}`,
                301
            );
        }
        return env.ASSETS.fetch(request);
    },
};
