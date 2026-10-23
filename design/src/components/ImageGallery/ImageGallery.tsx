import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

export type GalleryImage = { src: string; name: string; size?: string; alt?: string };

export type ImageGalleryProps = {
  images: GalleryImage[];
  /** Opens the preview at this image. */
  onOpen?: (index: number) => void;
} & HTMLAttributes<HTMLUListElement>;

/** Images in a reply, at a readable size, side by side when there are several (X40). */
export function ImageGallery({ images, onOpen, className, ...rest }: ImageGalleryProps) {
  return (
    <ul className={cx("grid gap-10", images.length > 1 ? "grid-cols-2" : "grid-cols-1", className)} {...rest}>
      {images.map((img, i) => (
        <li key={img.src + i}>
          <figure className="flex flex-col gap-6">
            <button type="button" onClick={() => onOpen?.(i)} aria-label={`Open ${img.name}`} className="overflow-hidden rounded-md bg-surface">
              <img src={img.src} alt={img.alt ?? ""} className="aspect-[3/2] w-full object-cover" />
            </button>
            <figcaption className="type-mono-xs text-faint">
              {img.name}
              {img.size && ` · ${img.size}`}
            </figcaption>
          </figure>
        </li>
      ))}
    </ul>
  );
}
