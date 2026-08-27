import { usePaths } from 'vitepress-openapi'
import spec from '../../public/openapi.json'

// Chinese translation map for API operation summaries
// Used for page titles and meta descriptions in the Chinese locale
const zhSummaryMap = {
    create_session: '创建收银台会话',
    get_session: '获取收银台会话',
    list_sessions: '列出收银台会话',
    create_payout: '创建出款',
    get_payout: '获取出款详情',
    list_payouts: '列出出款记录',
    create_sub_merchant: '创建子商户',
    get_sub_merchant: '获取子商户详情',
    list_sub_merchants: '列出子商户',
    update_sub_merchant: '更新子商户',
}

export default {
    paths() {
        return usePaths({ spec })
            .getPathsByVerbs()
            .map(({ operationId, summary }) => {
                const zhSummary = zhSummaryMap[operationId] || summary || operationId
                return {
                    params: {
                        operationId,
                        pageTitle: zhSummary,
                        pageDescription: `${zhSummary} — IronixPay REST API 接口文档。`,
                    },
                }
            })
    },
}
