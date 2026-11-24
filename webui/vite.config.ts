import { reactRouter } from '@react-router/dev/vite'
import tailwindcss from '@tailwindcss/vite'
import { createHash } from 'node:crypto'
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs'
import { join, relative, resolve } from 'node:path'
import { defineConfig, type Plugin } from 'vite'
import tsconfigPaths from 'vite-tsconfig-paths'

const target = process.env.VIZIER_URL ?? 'http://localhost:9999'

/** The token in the worker that the build replaces with the id of the build. */
const BUILD_TOKEN = '__VIZIER_BUILD__'

/** Every file under a folder, as paths relative to it. */
function filesUnder(dir: string, prefix = ''): string[] {
    let out: string[] = []
    for (const name of readdirSync(dir)) {
        const path = join(dir, name)
        if (statSync(path).isDirectory()) out = out.concat(filesUnder(path, join(prefix, name)))
        else out.push(join(prefix, name))
    }
    return out
}

/**
 * Stamps the service worker with the id of the build it belongs to.
 *
 * The worker is a static file, so without this its bytes are the same in every build: the
 * browser never installs a new one, the update offer never comes, and the cached shell stays as
 * it was. The id is a hash of what the build emitted, so it changes exactly when the app does.
 */
function stampWorker(): Plugin {
    let outDir = ''
    return {
        name: 'vizier:stamp-worker',
        apply: 'build',
        configResolved(config) {
            outDir = resolve(config.root, config.build.outDir)
        },
        closeBundle() {
            const worker = join(outDir, 'service-worker.js')
            let source: string
            try {
                source = readFileSync(worker, 'utf8')
            } catch {
                return // The server build has no worker.
            }
            const hash = createHash('sha256')
            for (const file of filesUnder(outDir).sort()) {
                hash.update(file)
                hash.update(String(statSync(join(outDir, file)).size))
            }
            const id = hash.digest('hex').slice(0, 12)
            writeFileSync(worker, source.replace(BUILD_TOKEN, id))
            console.log(`service worker stamped as ${id} (${relative(process.cwd(), worker)})`)
        },
    }
}

export default defineConfig({
    plugins: [tailwindcss(), reactRouter(), tsconfigPaths(), stampWorker()],
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
