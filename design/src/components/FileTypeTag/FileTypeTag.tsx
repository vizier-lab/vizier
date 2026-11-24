import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

export type FileTypeTagProps = {
  /** A file name or an extension; shown as the upper-case extension. */
  file: string;
} & HTMLAttributes<HTMLSpanElement>;

export function fileExtension(file: string) {
  const dot = file.lastIndexOf(".");
  return (dot >= 0 ? file.slice(dot + 1) : file).toUpperCase().slice(0, 4) || "FILE";
}

/** The type of a file: CSV, MD, PDF, PNG, XLSX, ZIP (X39). */
export function FileTypeTag({ file, className, ...rest }: FileTypeTagProps) {
  return (
    <span
      className={cx("inline-flex h-22 shrink-0 items-center rounded-sm bg-raised px-6 type-label text-muted", className)}
      {...rest}
    >
      {fileExtension(file)}
    </span>
  );
}
