#!/usr/bin/env node
// Regenerates the PWA icons from public/icons/icon.svg.
//
// The mark is the favicon's: the 8×8 dot grid from design/src/brand/Mark. The PNGs cannot be
// hand-written, so they are rasterised here with sharp — a build-time-only dependency that
// never reaches the bundle or the Rust binary.
//
//   npm run icons
import { readFile } from 'node:fs/promises'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import sharp from 'sharp'

const here = dirname(fileURLToPath(import.meta.url))
const icons = join(here, '..', 'public', 'icons')
const source = join(icons, 'icon.svg')

/** The ground the mark sits on. */
const GROUND = '#0b0b0c'
/** How much of the maskable icon the mark may fill: the platform crops it to the inner 80%. */
const MASKABLE_FILL = 0.55
/** The mark's ink box in the source's units: the dots run from 3.75 to 156.25 either way. */
const MARK = 152.5
/** The centre of that box. */
const CENTRE = 80

const svg = await readFile(source, 'utf8')

/** The dots on their own, so the maskable icon can put them on a full-bleed ground. */
function markOf(source) {
    const open = '<g id="mark">'
    const start = source.indexOf(open)
    if (start < 0) throw new Error('icon.svg has no <g id="mark">')
    const end = source.lastIndexOf('</g>')
    if (end < start) throw new Error('icon.svg has no closed <g id="mark">')
    return source.slice(start + open.length, end)
}

/** The mark on a square that reaches every edge, for the platforms that crop it to a shape. */
function maskableSvg(mark, size) {
    const scale = (size * MASKABLE_FILL) / MARK
    return (
        `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${size} ${size}">` +
        `<rect width="${size}" height="${size}" fill="${GROUND}"/>` +
        `<g transform="translate(${size / 2} ${size / 2}) scale(${scale}) translate(${-CENTRE} ${-CENTRE})">${mark}</g>` +
        `</svg>`
    )
}

/** The plain icons: the mark on the dark rounded square, as the favicon draws it. */
for (const size of [192, 512]) {
    await sharp(Buffer.from(svg), { density: 384 }).resize(size, size).png().toFile(join(icons, `icon-${size}.png`))
    console.log(`icon-${size}.png`)
}

/** The maskable one: the same mark, on a ground that reaches every edge. */
const maskable = 512
await sharp(Buffer.from(maskableSvg(markOf(svg), maskable)), { density: 384 })
    .resize(maskable, maskable)
    .png()
    .toFile(join(icons, 'icon-maskable-512.png'))
console.log('icon-maskable-512.png')
