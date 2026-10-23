// Fetches the running server's OpenAPI document into webui/openapi.json and regenerates the typed client.
// Usage: VIZIER_URL=http://localhost:9999 node scripts/openapi.mjs
import { writeFile } from 'node:fs/promises'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const base = process.env.VIZIER_URL ?? 'http://localhost:9999'
const webui = fileURLToPath(new URL('../webui/', import.meta.url))

const res = await fetch(`${base}/openapi.json`)
if (!res.ok) {
    console.error(`GET ${base}/openapi.json -> ${res.status}`)
    process.exit(1)
}
await writeFile(`${webui}openapi.json`, JSON.stringify(await res.json(), null, 2) + '\n')

const gen = spawnSync('npm', ['run', 'gen:api'], { cwd: webui, stdio: 'inherit' })
process.exit(gen.status ?? 1)
