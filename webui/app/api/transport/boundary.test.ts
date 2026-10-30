import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative, sep } from 'node:path'
import { describe, expect, it } from 'vitest'

const root = join(__dirname, '..', '..')
const allowed = join(root, 'api', 'transport') + sep

function sources(dir: string): string[] {
    return readdirSync(dir).flatMap((name) => {
        const path = join(dir, name)
        if (statSync(path).isDirectory()) return sources(path)
        return /\.(ts|tsx)$/.test(name) && !/\.test\.(ts|tsx)$/.test(name) && !name.endsWith('schema.d.ts') && !path.startsWith(allowed) ? [path] : []
    })
}

describe('transport boundary', () => {
    it('keeps fetch, WebSocket, API paths and Tauri imports inside app/api/transport', () => {
        const banned = [/\bfetch\(/, /new WebSocket\(/, /['"`]\/api\/v1/, /@tauri-apps\//]
        const hits = sources(root).flatMap((file) => {
            const text = readFileSync(file, 'utf8')
            return banned.filter((re) => re.test(text)).map((re) => `${relative(root, file)}: ${re}`)
        })
        expect(hits).toEqual([])
    })
})
