import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vitest/config'

const here = (path: string) => fileURLToPath(new URL(path, import.meta.url))

export default defineConfig({
    // One React for the app and the design library, as in vite.config.ts: a component test that
    // draws design components would otherwise meet two copies and fail on hooks.
    resolve: {
        dedupe: ['react', 'react-dom', 'motion'],
        alias: [
            { find: /^react$/, replacement: here('node_modules/react') },
            { find: /^react\/(.*)$/, replacement: here('node_modules/react/$1') },
            { find: /^react-dom$/, replacement: here('node_modules/react-dom') },
            { find: /^react-dom\/(.*)$/, replacement: here('node_modules/react-dom/$1') },
        ],
    },
    test: {
        include: ['app/**/*.test.ts', 'app/**/*.test.tsx'],
        environment: 'node',
        setupFiles: ['app/test/oneReact.ts'],
        server: { deps: { inline: [/@vizier\/design/, /lucide-react/, /\/design\/node_modules\//] } },
    },
})
