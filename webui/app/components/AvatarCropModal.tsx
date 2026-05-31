import { useState, useCallback } from 'react'
import Cropper from 'react-easy-crop'
import type { Area } from 'react-easy-crop'

interface AvatarCropModalProps {
  file: File | null
  onClose: () => void
  onCropped: (blob: Blob) => void
}

async function getCroppedImg(imageSrc: string, crop: Area): Promise<Blob> {
  const image = new Image()
  image.src = imageSrc
  await new Promise((resolve) => { image.onload = resolve })

  const canvas = document.createElement('canvas')
  const ctx = canvas.getContext('2d')!
  canvas.width = 256
  canvas.height = 256

  ctx.drawImage(
    image,
    crop.x,
    crop.y,
    crop.width,
    crop.height,
    0,
    0,
    256,
    256,
  )

  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (blob) resolve(blob)
      else reject(new Error('Failed to crop image'))
    }, 'image/png')
  })
}

export default function AvatarCropModal({ file, onClose, onCropped }: AvatarCropModalProps) {
  const [crop, setCrop] = useState({ x: 0, y: 0 })
  const [zoom, setZoom] = useState(1)
  const [croppedArea, setCroppedArea] = useState<Area | null>(null)
  const [processing, setProcessing] = useState(false)

  const imageSrc = file ? URL.createObjectURL(file) : ''

  const onCropComplete = useCallback((_croppedArea: Area, croppedAreaPixels: Area) => {
    setCroppedArea(croppedAreaPixels)
  }, [])

  const handleConfirm = useCallback(async () => {
    if (!croppedArea || !imageSrc) return
    setProcessing(true)
    try {
      const blob = await getCroppedImg(imageSrc, croppedArea)
      onCropped(blob)
    } catch (err) {
      console.error('Failed to crop image:', err)
    } finally {
      setProcessing(false)
    }
  }, [croppedArea, imageSrc, onCropped])

  if (!file) return null

  return (
    <>
      <div
        style={{
          position: 'fixed',
          top: 0,
          left: 0,
          right: 0,
          bottom: 0,
          background: 'rgba(0, 0, 0, 0.6)',
          zIndex: 1000,
          backdropFilter: 'blur(4px)',
        }}
        onClick={onClose}
      />
      <div
        style={{
          position: 'fixed',
          top: '50%',
          left: '50%',
          transform: 'translate(-50%, -50%)',
          background: 'var(--background)',
          borderRadius: '12px',
          padding: '1.5rem',
          width: '400px',
          maxWidth: '90vw',
          zIndex: 1001,
          border: '1px solid var(--border)',
          boxShadow: 'var(--shadow-xl)',
        }}
      >
        <h3 style={{ margin: '0 0 1rem 0' }}>Crop Avatar</h3>

        <div style={{ position: 'relative', width: '100%', height: '280px', borderRadius: '8px', overflow: 'hidden' }}>
          <Cropper
            image={imageSrc}
            crop={crop}
            zoom={zoom}
            aspect={1}
            cropShape="round"
            onCropChange={setCrop}
            onZoomChange={setZoom}
            onCropComplete={onCropComplete}
          />
        </div>

        <div style={{ margin: '1rem 0', display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
          <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)', minWidth: '40px' }}>Zoom</span>
          <input
            type="range"
            min={1}
            max={3}
            step={0.1}
            value={zoom}
            onChange={(e) => setZoom(parseFloat(e.target.value))}
            style={{ flex: 1 }}
          />
        </div>

        <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
          <button
            className="btn btn-secondary"
            onClick={onClose}
            disabled={processing}
          >
            Cancel
          </button>
          <button
            className="btn btn-primary"
            onClick={handleConfirm}
            disabled={processing}
          >
            {processing ? 'Cropping...' : 'Confirm'}
          </button>
        </div>
      </div>
    </>
  )
}
