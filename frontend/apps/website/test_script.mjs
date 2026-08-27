import fs from 'fs';
const spec = JSON.parse(fs.readFileSync('public/openapi.json', 'utf-8'));
spec.info.title = 'IronixPay API (ZH)';
const zhMap = {
    'create_checkouts_session_v1_checkout__post': '创建收银台会话',
    'get_checkout_session_v1_checkout__session_id__get': '获取收银台会话',
    // add a few summaries for testing
};
for (const path of Object.values(spec.paths)) {
    for (const op of Object.values(path)) {
        if (zhMap[op.operationId]) {
            op.summary = zhMap[op.operationId];
        }
    }
}
fs.writeFileSync('public/openapi-zh.json', JSON.stringify(spec, null, 2));
