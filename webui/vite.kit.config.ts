// The kit build (specs/063-html-generative-ui research R11): `app/kit/index.ts` as one classic
// script and one stylesheet in `build/client/ui-kit/`, with hashed names, plus `manifest.json`
// naming them. A classic script, not a module: a page's opaque origin would need CORS for a
// module, and the kit has no imports to share with the page. Runs after `react-router build`,
// which empties `build/client/`.
import { writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { defineConfig, type Plugin } from 'vite'
import tsconfigPaths from 'vite-tsconfig-paths'

/** Writes `manifest.json` with the hashed file names the page host links. */
function kitManifest(): Plugin {
    return {
        name: 'vizier:kit-manifest',
        enforce: 'post',
        // After the files are written, so the stylesheet Vite extracts is in the bundle too.
        writeBundle(options, bundle) {
            const files: { js?: string; css?: string } = {}
            for (const item of Object.values(bundle)) {
                if (item.type === 'chunk' && item.isEntry) files.js = item.fileName
                if (item.type === 'asset' && item.fileName.endsWith('.css')) files.css = item.fileName
            }
            if (!files.js || !files.css) this.error(`the kit build is missing a file: ${JSON.stringify(files)}`)
            writeFileSync(join(options.dir ?? 'build/client/ui-kit', 'manifest.json'), `${JSON.stringify({ version: 1, files }, null, 2)}\n`)
        },
    }
}

export default defineConfig({
    plugins: [tsconfigPaths(), kitManifest()],
    resolve: { dedupe: ['react', 'react-dom'] },
    define: { 'process.env.NODE_ENV': JSON.stringify('production') },
    build: {
        outDir: 'build/client/ui-kit',
        emptyOutDir: true,
        cssCodeSplit: false,
        copyPublicDir: false,
        rollupOptions: {
            input: 'app/kit/index.ts',
            output: {
                format: 'iife',
                entryFileNames: 'kit-[hash].js',
                assetFileNames: 'kit-[hash][extname]',
            },
        },
    },
})
