import { reactRouter } from '@react-router/dev/vite'
import tailwindcss from '@tailwindcss/vite'
import { defineConfig } from 'vite'
import tsconfigPaths from 'vite-tsconfig-paths'

const target = process.env.VIZIER_URL ?? 'http://localhost:9999'

export default defineConfig({
    plugins: [tailwindcss(), reactRouter(), tsconfigPaths()],
    resolve: { dedupe: ['react', 'react-dom'] },
    server: {
        port: 5173,
        strictPort: true,
        fs: { allow: ['..'] },
        proxy: {
            '/api': { target, changeOrigin: true, ws: true },
        },
    },
})
