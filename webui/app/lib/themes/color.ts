// lint-tokens-exempt: colour maths, not styling.
// Hex colours, OKLCH, contrast and colour difference. Only opaque #rrggbb is ever stored or painted,
// which is also what keeps a theme file from being able to supply anything but a colour.
import type { Hex } from './roles'

const HEX = /^#[0-9a-fA-F]{6}$/
const SHORT_HEX = /^#[0-9a-fA-F]{3}$/

export const isHex = (value: unknown): value is Hex => typeof value === 'string' && HEX.test(value)

/** What a person typed as a colour, as lowercase #rrggbb, or null if it isn't one. */
export function normalizeHex(input: string): Hex | null {
    const value = input.trim()
    if (SHORT_HEX.test(value)) return `#${[...value.slice(1)].map((c) => c + c).join('')}`.toLowerCase()
    return HEX.test(value) ? value.toLowerCase() : null
}

type Rgb = [number, number, number]
type Oklab = [number, number, number]
export type Oklch = { l: number; c: number; h: number }

const toLinear = (x: number) => (x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4)
const toGamma = (x: number) => (x <= 0.0031308 ? 12.92 * x : 1.055 * x ** (1 / 2.4) - 0.055)

function hexToRgb(hex: Hex): Rgb {
    return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255) as Rgb
}

function rgbToHex(rgb: Rgb): Hex {
    return `#${rgb.map((x) => Math.round(Math.min(1, Math.max(0, x)) * 255).toString(16).padStart(2, '0')).join('')}`
}

function hexToOklab(hex: Hex): Oklab {
    const [r, g, b] = hexToRgb(hex).map(toLinear)
    const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
    const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
    const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)
    return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s]
}

function oklabToLinear([L, a, b]: Oklab): Rgb {
    const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3
    const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3
    const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3
    return [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s]
}

export function hexToOklch(hex: Hex): Oklch {
    const [l, a, b] = hexToOklab(hex)
    const h = (Math.atan2(b, a) * 180) / Math.PI
    return { l, c: Math.hypot(a, b), h: h < 0 ? h + 360 : h }
}

const inGamut = (rgb: Rgb) => rgb.every((x) => x >= -0.0001 && x <= 1.0001)

/** Back to hex, lowering chroma as far as it takes to fit in sRGB, so the hue and lightness hold. */
export function oklchToHex({ l, c, h }: Oklch): Hex {
    const lab = (chroma: number): Oklab => [l, chroma * Math.cos((h * Math.PI) / 180), chroma * Math.sin((h * Math.PI) / 180)]
    let fitted = oklabToLinear(lab(c))
    if (!inGamut(fitted)) {
        let low = 0
        let high = c
        for (let i = 0; i < 16; i++) {
            const mid = (low + high) / 2
            if (inGamut(oklabToLinear(lab(mid)))) low = mid
            else high = mid
        }
        fitted = oklabToLinear(lab(low))
    }
    return rgbToHex(fitted.map((x) => toGamma(Math.max(0, x))) as Rgb)
}

// Below this a colour reads as grey, and its hue is an accident of rounding.
const ACHROMATIC = 0.02

export const isAchromatic = (hex: Hex) => hexToOklch(hex).c < ACHROMATIC

/**
 * `weight` of the way from `from` to `to`, in OKLCH by the shorter way round the hue. A grey has no
 * real hue, so the other colour's is used, or a mix with white would swing through a rainbow.
 */
export function mix(from: Hex, to: Hex, weight: number): Hex {
    const a = hexToOklch(from)
    const b = hexToOklch(to)
    const hueA = a.c < ACHROMATIC ? b.h : a.h
    const hueB = b.c < ACHROMATIC ? a.h : b.h
    const delta = ((hueB - hueA + 540) % 360) - 180
    return oklchToHex({ l: a.l + (b.l - a.l) * weight, c: a.c + (b.c - a.c) * weight, h: hueA + delta * weight })
}

/** The same colour with its hue turned, at a chroma of at least `minChroma`. */
export function rotateHue(hex: Hex, degrees: number, minChroma = 0): Hex {
    const { l, c, h } = hexToOklch(hex)
    return oklchToHex({ l, c: Math.max(c, minChroma), h: (h + degrees + 360) % 360 })
}

export const withLightness = (hex: Hex, l: number): Hex => oklchToHex({ ...hexToOklch(hex), l: Math.min(1, Math.max(0, l)) })

function luminance(hex: Hex): number {
    const [r, g, b] = hexToRgb(hex).map(toLinear)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

/** The WCAG contrast ratio, from 1 to 21. */
export function contrast(a: Hex, b: Hex): number {
    const [x, y] = [luminance(a), luminance(b)]
    return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05)
}

/** The distance between two colours in OKLab, which is how alike they look. */
export function deltaEok(a: Hex, b: Hex): number {
    const [x, y] = [hexToOklab(a), hexToOklab(b)]
    return Math.hypot(x[0] - y[0], x[1] - y[1], x[2] - y[2])
}

/**
 * `fg` with its lightness moved as little as possible to reach `min` against `bg`. If neither
 * direction can reach it, the direction that gets furthest. The hue stays.
 */
export function ensureContrast(fg: Hex, bg: Hex, min: number): Hex {
    if (contrast(fg, bg) >= min) return fg
    const { l } = hexToOklch(fg)
    const search = (end: number): Hex | null => {
        if (contrast(withLightness(fg, end), bg) < min) return null
        let near = l
        let far = end
        for (let i = 0; i < 20; i++) {
            const mid = (near + far) / 2
            if (contrast(withLightness(fg, mid), bg) >= min) far = mid
            else near = mid
        }
        return withLightness(fg, far)
    }
    const up = search(1)
    const down = search(0)
    if (up && down) return Math.abs(hexToOklch(up).l - l) <= Math.abs(hexToOklch(down).l - l) ? up : down
    return up ?? down ?? (contrast('#ffffff', bg) >= contrast('#000000', bg) ? withLightness(fg, 1) : withLightness(fg, 0))
}
