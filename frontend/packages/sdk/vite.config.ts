import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig({
    build: {
        lib: {
            entry: resolve(__dirname, 'src/index.ts'),
            name: 'IronixPay',
            fileName: (format) => `ironix-pay.${format === 'es' ? 'mjs' : 'umd.js'}`
        },
        rollupOptions: {
            output: {
                exports: 'named'
            }
        },
        minify: 'esbuild',
        sourcemap: true
    }
});
