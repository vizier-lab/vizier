import tailwindcss from '@tailwindcss/vite'
import { defineConfig } from 'vite'
import { viteSingleFile } from 'vite-plugin-singlefile'
import tsconfigPaths from 'vite-tsconfig-paths'

// Single-file viewer template (specs/056-shared-chat R9): one HTML file with all script and CSS
// inlined, written to build/client/viewer-template.html after the SPA build.
export default defineConfig({
    plugins: [tailwindcss(), tsconfigPaths(), viteSingleFile()],
    resolve: { dedupe: ['react', 'react-dom', 'motion'] },
    publicDir: false, // the SPA build already copied public/
    build: {
        outDir: 'build/client',
        emptyOutDir: false, // the SPA build already wrote here
        cssCodeSplit: false,
        assetsInlineLimit: 100_000_000,
        rollupOptions: {
            input: 'viewer.html',
            output: { inlineDynamicImports: true },
        },
    },
})
