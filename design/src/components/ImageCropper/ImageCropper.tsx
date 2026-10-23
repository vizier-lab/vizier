import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent, type WheelEvent } from "react";
import { cx } from "../../lib/cx";
import { Button } from "../Button/Button";

export type ImageCropperProps = {
  /** The picture to crop: a file the person chose. */
  file: File;
  /** The square crop, as a PNG. */
  onCrop: (blob: Blob) => void;
  onCancel: () => void;
  /** Side of the saved picture, in pixels. */
  outputSize?: number;
  title?: string;
};

/** The frame on screen; the saved picture is `outputSize`. */
const FRAME = 280;
const MAX_ZOOM = 4;
const NUDGE = 8;

type Natural = { w: number; h: number };

/** Pan and zoom a picture under a square frame: where the crop lands is what is saved. */
export function clampOffset(offset: number, shown: number, frame: number): number {
  // The picture always covers the frame, so its edge can't pass the frame's.
  const slack = Math.max(0, (shown - frame) / 2);
  return Math.min(slack, Math.max(-slack, offset));
}

/** Square crop of an uploaded picture (X79): drag, zoom slider or wheel, arrow keys to nudge. */
export function ImageCropper({ file, onCrop, onCancel, outputSize = 512, title = "Crop picture" }: ImageCropperProps) {
  const titleId = useId();
  const src = useMemo(() => URL.createObjectURL(file), [file]);
  useEffect(() => () => URL.revokeObjectURL(src), [src]);

  const [natural, setNatural] = useState<Natural>();
  const [zoom, setZoom] = useState(1);
  const [pos, setPos] = useState({ x: 0, y: 0 });
  const [busy, setBusy] = useState(false);
  const drag = useRef<{ px: number; py: number; x: number; y: number }>(undefined);
  const dialog = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const d = dialog.current;
    if (d && !d.open) d.showModal();
  }, []);

  // At zoom 1 the shorter side fills the frame.
  const base = natural ? FRAME / Math.min(natural.w, natural.h) : 1;
  const shownW = natural ? natural.w * base * zoom : FRAME;
  const shownH = natural ? natural.h * base * zoom : FRAME;
  const x = clampOffset(pos.x, shownW, FRAME);
  const y = clampOffset(pos.y, shownH, FRAME);

  const move = (nx: number, ny: number) => setPos({ x: clampOffset(nx, shownW, FRAME), y: clampOffset(ny, shownH, FRAME) });

  const onPointerDown = (e: PointerEvent) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { px: e.clientX, py: e.clientY, x, y };
  };
  const onPointerMove = (e: PointerEvent) => {
    const d = drag.current;
    if (d) move(d.x + e.clientX - d.px, d.y + e.clientY - d.py);
  };
  const onWheel = (e: WheelEvent) => setZoom((z) => Math.min(MAX_ZOOM, Math.max(1, z - e.deltaY / 400)));
  const onKeyDown = (e: KeyboardEvent) => {
    const step = e.shiftKey ? NUDGE * 4 : NUDGE;
    const by: Record<string, [number, number]> = {
      ArrowLeft: [-step, 0],
      ArrowRight: [step, 0],
      ArrowUp: [0, -step],
      ArrowDown: [0, step],
    };
    const delta = by[e.key];
    if (delta) {
      e.preventDefault();
      move(x + delta[0], y + delta[1]);
    } else if (e.key === "+" || e.key === "=") setZoom((z) => Math.min(MAX_ZOOM, z + 0.1));
    else if (e.key === "-") setZoom((z) => Math.max(1, z - 0.1));
  };

  const save = async () => {
    if (!natural) return;
    setBusy(true);
    try {
      const img = await createImageBitmap(file);
      const canvas = document.createElement("canvas");
      canvas.width = canvas.height = outputSize;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      // The frame, in the picture's own pixels.
      const scale = base * zoom;
      const side = FRAME / scale;
      const sx = natural.w / 2 - side / 2 - x / scale;
      const sy = natural.h / 2 - side / 2 - y / scale;
      ctx.drawImage(img, sx, sy, side, side, 0, 0, outputSize, outputSize);
      const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
      if (blob) onCrop(blob);
    } finally {
      setBusy(false);
    }
  };

  return (
    <dialog
      ref={dialog}
      aria-labelledby={titleId}
      onCancel={(e) => {
        e.preventDefault();
        onCancel();
      }}
      className="m-auto max-w-full bg-transparent p-0 backdrop:bg-ground/60"
    >
      <div className="flex w-440 max-w-full flex-col gap-16 rounded-xl border border-border-strong bg-surface px-24 py-20 shadow-dialog">
        <h2 id={titleId} className="type-heading text-fg">
          {title}
        </h2>
        <div
          role="group"
          tabIndex={0}
          aria-label="Crop area. Drag or use the arrow keys to move the picture, plus and minus to zoom."
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={() => (drag.current = undefined)}
          onWheel={onWheel}
          onKeyDown={onKeyDown}
          style={{ width: FRAME, height: FRAME }}
          className={cx(
            "relative mx-auto cursor-grab touch-none select-none overflow-hidden rounded-lg border border-border-strong bg-ground",
            "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus active:cursor-grabbing",
          )}
        >
          <img
            src={src}
            alt=""
            draggable={false}
            onLoad={(e) => setNatural({ w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight })}
            style={{
              width: shownW,
              height: shownH,
              maxWidth: "none",
              left: "50%",
              top: "50%",
              transform: `translate(calc(-50% + ${x}px), calc(-50% + ${y}px))`,
            }}
            className="pointer-events-none absolute"
          />
        </div>
        <label className="flex items-center gap-12 type-small text-muted">
          Zoom
          <input
            type="range"
            min={1}
            max={MAX_ZOOM}
            step={0.01}
            value={zoom}
            onChange={(e) => setZoom(Number(e.target.value))}
            className="h-4 flex-1 accent-fg"
          />
        </label>
        <div className="flex justify-end gap-10 pt-4">
          <Button onClick={onCancel}>Cancel</Button>
          <Button variant="primary" icon="check" disabled={!natural || busy} onClick={save}>
            Use picture
          </Button>
        </div>
      </div>
    </dialog>
  );
}
