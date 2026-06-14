import html2canvas from 'html2canvas-pro'
import { jsPDF } from 'jspdf'

const A4_WIDTH_MM = 210
const A4_HEIGHT_MM = 297
const MARGIN_MM = 10
const RENDER_SCALE = 2

function resolveBackgroundColor(element: HTMLElement): string {
  const surface = getComputedStyle(element).getPropertyValue('--surface').trim()
  if (surface) return surface
  const bodySurface = getComputedStyle(document.body).getPropertyValue('--surface').trim()
  if (bodySurface) return bodySurface
  return '#ffffff'
}

function stripMediaElements(clone: HTMLElement): void {
  const media = clone.querySelectorAll('audio, video')
  media.forEach((node) => {
    const placeholder = document.createElement('span')
    placeholder.textContent = '[Voice message]'
    placeholder.style.display = 'inline-block'
    placeholder.style.padding = '2px 8px'
    placeholder.style.border = '1px solid currentColor'
    placeholder.style.borderRadius = '4px'
    placeholder.style.fontSize = '12px'
    placeholder.style.opacity = '0.7'
    node.parentNode?.replaceChild(placeholder, node)
  })
}

async function captureElement(element: HTMLElement): Promise<HTMLCanvasElement> {
  const clone = element.cloneNode(true) as HTMLElement
  clone.style.position = 'fixed'
  clone.style.left = '-99999px'
  clone.style.top = '0'
  clone.style.width = `${element.offsetWidth}px`
  clone.style.background = resolveBackgroundColor(element)
  document.body.appendChild(clone)

  try {
    stripMediaElements(clone)
    return await html2canvas(clone, {
      scale: RENDER_SCALE,
      backgroundColor: resolveBackgroundColor(element),
      useCORS: true,
      logging: false,
    })
  } finally {
    clone.remove()
  }
}

function sliceCanvasForPage(
  source: HTMLCanvasElement,
  pageHeightPx: number,
  yOffset: number,
): HTMLCanvasElement {
  const sliceHeight = Math.min(pageHeightPx, source.height - yOffset)
  const slice = document.createElement('canvas')
  slice.width = source.width
  slice.height = sliceHeight
  const ctx = slice.getContext('2d')
  if (!ctx) throw new Error('Failed to acquire 2d context for PDF slicing')
  ctx.drawImage(
    source,
    0,
    yOffset,
    source.width,
    sliceHeight,
    0,
    0,
    source.width,
    sliceHeight,
  )
  return slice
}

export async function exportElementAsPdf(
  element: HTMLElement,
  filename: string,
): Promise<void> {
  const canvas = await captureElement(element)

  const pdf = new jsPDF({
    unit: 'mm',
    format: 'a4',
    orientation: 'portrait',
  })

  const contentWidthMm = A4_WIDTH_MM - MARGIN_MM * 2
  const contentHeightMm = A4_HEIGHT_MM - MARGIN_MM * 2
  const scale = contentWidthMm / canvas.width
  const pageHeightPx = contentHeightMm / scale

  let yOffset = 0
  let pageIndex = 0

  while (yOffset < canvas.height) {
    const slice = sliceCanvasForPage(canvas, pageHeightPx, yOffset)
    const sliceData = slice.toDataURL('image/png')
    const sliceHeightMm = slice.height * scale

    if (pageIndex > 0) pdf.addPage()
    pdf.addImage(sliceData, 'PNG', MARGIN_MM, MARGIN_MM, contentWidthMm, sliceHeightMm)

    yOffset += slice.height
    pageIndex += 1
  }

  pdf.save(filename)
}
