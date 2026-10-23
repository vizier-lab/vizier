import type { HTMLAttributes } from "react";
import { cx } from "../../lib/cx";

export type ProseProps = {
  /** Reply: chat replies at reading size. Document: Core and the skill editor. */
  size?: "reply" | "document";
} & HTMLAttributes<HTMLDivElement>;

// Rendered markdown, styled through its elements so any renderer's output fits.
const BASE = [
  "text-fg",
  "[&>*+*]:mt-12",
  "[&_h1]:type-title [&_h2]:type-heading [&_h3]:type-body-strong [&_h1]:text-fg [&_h2]:text-fg [&_h3]:text-fg",
  "[&_:is(h1,h2,h3)+*]:mt-8",
  "[&_ul]:list-disc [&_ol]:list-decimal [&_:is(ul,ol)]:pl-20 [&_li+li]:mt-4",
  "[&_a]:text-fg [&_a]:underline [&_a]:underline-offset-2",
  "[&_strong]:font-semibold",
  "[&_blockquote]:border-l-2 [&_blockquote]:border-border-strong [&_blockquote]:pl-12 [&_blockquote]:text-muted",
  "[&_:not(pre)>code]:rounded-xs [&_:not(pre)>code]:bg-raised [&_:not(pre)>code]:px-4 [&_:not(pre)>code]:type-code [&_:not(pre)>code]:text-soft",
  "[&_pre]:overflow-x-auto [&_pre]:rounded-md [&_pre]:border [&_pre]:border-border [&_pre]:bg-surface [&_pre]:px-16 [&_pre]:py-12 [&_pre]:type-code-block [&_pre]:text-soft",
  "[&_table]:w-full [&_table]:border-collapse [&_th]:border-b [&_th]:border-border [&_th]:px-8 [&_th]:py-6 [&_th]:text-left [&_th]:type-label [&_th]:uppercase [&_th]:text-faint",
  "[&_td]:border-b [&_td]:border-border [&_td]:px-8 [&_td]:py-6 [&_td]:type-ui [&_td]:text-soft",
  "[&_hr]:border-border",
].join(" ");

/** Rendered markdown (X49): headings, paragraphs, lists, tables, code, quotes. */
export function Prose({ size = "reply", className, ...rest }: ProseProps) {
  return <div className={cx(BASE, size === "reply" ? "type-prose" : "type-body-para", className)} {...rest} />;
}
