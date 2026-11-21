// The page kit build (specs/063-html-generative-ui research R7a, R11), written to
// `build/client/ui-kit/` after `react-router build` (which empties `build/client/`):
//
//   vite build --config vite.kit.config.ts                 → kit-[hash].js, kit-[hash].css, lib/*
//   vite build --config vite.kit.config.ts --mode runtime  → runtime-[hash].js
//
// Each is one classic script (IIFE): the server inlines them into a page's frame document, so
// nothing is loaded by URL. `manifest.json` names the files for the server; the second build
// adds its file to what the first wrote.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, join } from 'node:path'
import { build, defineConfig, type Plugin, type Rollup } from 'vite'
import tailwindcss from '@tailwindcss/vite'
import tsconfigPaths from 'vite-tsconfig-paths'
import { manifestData } from './app/kit/manifest'

const OUT = 'build/client/ui-kit'

type Manifest = {
    version: 1
    files: { js?: string; css?: string; runtime?: string }
    libraries: Record<string, string>
    /** What the kit offers a page (`app/kit/manifest.ts`): the reference is checked against it. */
    kit?: ReturnType<typeof manifestData>
}

/** Libraries a page may `import` by name (research R11): the import name, the module to bundle and
 * the file it becomes. Each is bundled into one ES module, because the frame loads it from a blob
 * URL, where a relative import (`three.module.js` imports `./three.core.js`) cannot resolve. */
function libraries(): { name: string; entry: string; file: string }[] {
    const require = createRequire(import.meta.url)
    // `three` exports no `./package.json`; its entry is `build/three.cjs`, two levels below the root.
    const threeRoot = dirname(dirname(require.resolve('three')))
    const threeVersion = (JSON.parse(readFileSync(join(threeRoot, 'package.json'), 'utf8')) as { version: string }).version
    return [{ name: 'three', entry: join(threeRoot, 'build', 'three.module.js'), file: `lib/three-${threeVersion}.js` }]
}

/** One library as a single minified ES module. */
async function bundleLibrary(entry: string): Promise<string> {
    const result = (await build({
        configFile: false,
        logLevel: 'warn',
        build: {
            write: false,
            minify: true,
            lib: { entry, formats: ['es'], fileName: 'lib' },
            rollupOptions: { output: { inlineDynamicImports: true } },
        },
    })) as Rollup.RollupOutput | Rollup.RollupOutput[]
    const outputs = Array.isArray(result) ? result : [result]
    const chunk = outputs.flatMap((o) => o.output).find((item) => item.type === 'chunk')
    if (!chunk || chunk.type !== 'chunk') throw new Error(`no bundle for ${entry}`)
    return chunk.code
}

function readManifest(dir: string): Manifest {
    const path = join(dir, 'manifest.json')
    if (!existsSync(path)) return { version: 1, files: {}, libraries: {} }
    return JSON.parse(readFileSync(path, 'utf8')) as Manifest
}

/** Writes (or, for the runtime build, adds to) `manifest.json`, after the files are written. */
function kitManifest(runtime: boolean): Plugin {
    return {
        name: 'vizier:kit-manifest',
        enforce: 'post',
        async writeBundle(options, bundle) {
            const dir = options.dir ?? OUT
            const manifest = runtime ? readManifest(dir) : { version: 1 as const, files: {}, libraries: {} }
            for (const item of Object.values(bundle)) {
                if (item.type === 'chunk' && item.isEntry) {
                    if (runtime) manifest.files.runtime = item.fileName
                    else manifest.files.js = item.fileName
                }
                if (!runtime && item.type === 'asset' && item.fileName.endsWith('.css')) manifest.files.css = item.fileName
            }
            if (!runtime) {
                manifest.kit = manifestData()
                for (const lib of libraries()) {
                    mkdirSync(dirname(join(dir, lib.file)), { recursive: true })
                    writeFileSync(join(dir, lib.file), await bundleLibrary(lib.entry))
                    manifest.libraries[lib.name] = lib.file
                }
            }
            const missing = runtime ? !manifest.files.runtime : !manifest.files.js || !manifest.files.css
            if (missing) this.error(`the kit build is missing a file: ${JSON.stringify(manifest.files)}`)
            writeFileSync(join(dir, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`)
        },
    }
}

/** The design package's own `@fontsource-variable`, so `base.css` can name the one font file it
 * inlines (a bare package path in a CSS `url()` is not resolved otherwise). */
function fontsource(): string {
    const tokens = createRequire(import.meta.url).resolve('@vizier/design/tokens.css')
    return join(dirname(dirname(dirname(tokens))), 'node_modules', '@fontsource-variable')
}

export default defineConfig(({ mode }) => {
    const runtime = mode === 'runtime'
    return {
        plugins: [tailwindcss(), tsconfigPaths(), kitManifest(runtime)],
        resolve: {
            dedupe: ['react', 'react-dom'],
            alias: [{ find: /^@fontsource-variable\//, replacement: `${fontsource().replaceAll('\\', '/')}/` }],
        },
        define: { 'process.env.NODE_ENV': JSON.stringify('production') },
        build: {
            outDir: OUT,
            // The runtime build adds to what the kit build wrote.
            emptyOutDir: !runtime,
            cssCodeSplit: false,
            // The frame loads nothing by URL: the one font it uses (base.css) is inlined, and no
            // other asset is (the elements' other font faces are dropped by the kit script).
            assetsInlineLimit: (file: string) => /schibsted-grotesk-latin-wght-normal/.test(file),
            copyPublicDir: false,
            rollupOptions: {
                input: runtime ? 'app/kit/runtime.ts' : 'app/kit/index.ts',
                output: {
                    format: 'iife',
                    entryFileNames: runtime ? 'runtime-[hash].js' : 'kit-[hash].js',
                    assetFileNames: 'kit-[hash][extname]',
                },
            },
        },
    }
})
