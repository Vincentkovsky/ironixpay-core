import { defineConfig } from 'vite'
import { resolve } from 'path'

export default defineConfig({
    build: {
        lib: {
            entry: resolve(__dirname, 'src/index.ts'),
            name: 'IronixPayApiClient',
            fileName: 'index'
        },
        rollupOptions: {
            external: ['axios'],
            output: {
                globals: {
                    axios: 'axios'
                }
            }
        }
    }
})
