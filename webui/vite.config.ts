import { reactRouter } from '@react-router/dev/vite'
import tailwindcss from '@tailwindcss/vite'
import { defineConfig } from 'vite'
import tsconfigPaths from 'vite-tsconfig-paths'

const target = process.env.VIZIER_URL ?? 'http://localhost:9999'

export default defineConfig({
    plugins: [tailwindcss(), reactRouter(), tsconfigPaths()],
    resolve: { dedupe: ['react', 'react-dom', 'motion'] },
    // Only reached through the desktop transport's lazy import, so the dep scan misses them and the
    // first desktop launch re-optimizes and reloads mid-boot (a blank window).
    optimizeDeps: { include: ['@tauri-apps/api/core', '@tauri-apps/api/event', '@tauri-apps/plugin-notification'] },
    server: {
        port: 5173,
        strictPort: true,
        fs: { allow: ['..'] },
        proxy: {
            '/api': { target, changeOrigin: true, ws: true },
        },
    },
})
