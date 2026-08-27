import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import tailwindcss from '@tailwindcss/vite';
export default defineConfig({
    plugins: [vue(), tailwindcss()],
    server: {
        port: 3001
    },
    build: {
        // Target modern browsers for smaller output
        target: 'es2020',
        rollupOptions: {
            output: {
                // Split vendor libraries into separate chunks for parallel loading
                manualChunks: {
                    'vendor-vue': ['vue', 'vue-router', 'pinia'],
                    'vendor-i18n': ['vue-i18n'],
                    'vendor-ui': ['lucide-vue-next', 'qrcode.vue'],
                    'vendor-utils': ['date-fns'],
                }
            }
        }
    }
});
