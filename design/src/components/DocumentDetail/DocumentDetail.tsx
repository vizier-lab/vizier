import type { HTMLAttributes, ReactNode } from "react";
import { Icon } from "../../icons/Icon";
import { cx } from "../../lib/cx";
import { Badge } from "../Badge/Badge";
import { Button } from "../Button/Button";
import { Eyebrow } from "../Eyebrow/Eyebrow";
import { IconButton } from "../IconButton/IconButton";

export type DocLink = { label: string; href: string; note?: string };

export type DocumentDetailProps = {
  title: ReactNode;
  /** "infra / vector-dbs/summary.md · 3 passages". */
  path: ReactNode;
  tags?: string[];
  summary?: ReactNode;
  linksTo?: DocLink[];
  linkedFrom?: DocLink[];
  /** "Created by Archivist · today 14:03 (chat vector-db-review) · rev 3". */
  provenance?: ReactNode;
  onEdit?: () => void;
  onHistory?: () => void;
  onClose?: () => void;
} & Omit<HTMLAttributes<HTMLElement>, "title">;

function Links({ label, links }: { label: string; links: DocLink[] }) {
  return (
    <section className="flex flex-col gap-6">
      <Eyebrow as="h3">{label}</Eyebrow>
      <ul className="flex flex-col gap-4">
        {links.map((l) => (
          <li key={l.href}>
            <a href={l.href} className="inline-flex items-center gap-6 rounded-xs type-mono-sm text-fg hover:underline">
              <Icon name="arrow-right" size={12} tone="muted" />
              {l.label}
              {l.note && <span className="text-faint">({l.note})</span>}
            </a>
          </li>
        ))}
      </ul>
    </section>
  );
}

/** A memory document beside the graph (X74): what it says, what it links, where it came from. */
export function DocumentDetail({ title, path, tags, summary, linksTo, linkedFrom, provenance, onEdit, onHistory, onClose, className, ...rest }: DocumentDetailProps) {
  return (
    <article className={cx("flex flex-col gap-14", className)} {...rest}>
      <header className="flex items-start gap-12">
        <div className="flex min-w-0 flex-1 flex-col gap-4">
          <h2 className="type-title text-fg">{title}</h2>
          <p className="type-mono-xs text-faint">{path}</p>
        </div>
        {onClose && <IconButton variant="ghost" size="sm" icon="close" aria-label="Close" onClick={onClose} />}
      </header>
      {tags && tags.length > 0 && (
        <div className="flex flex-wrap gap-6">
          {tags.map((t) => (
            <Badge key={t}>{t}</Badge>
          ))}
        </div>
      )}
      {summary && <p className="type-body-para text-fg">{summary}</p>}
      {linksTo && linksTo.length > 0 && <Links label="Links to" links={linksTo} />}
      {linkedFrom && linkedFrom.length > 0 && <Links label="Linked from" links={linkedFrom} />}
      {provenance && <p className="type-small-para text-muted">{provenance}</p>}
      {(onEdit || onHistory) && (
        <div className="flex gap-10">
          {onEdit && <Button icon="pencil" onClick={onEdit}>Edit</Button>}
          {onHistory && <Button icon="history" onClick={onHistory}>History</Button>}
        </div>
      )}
    </article>
  );
}

export type SelectedBundleProps = {
  name: string;
  /** "42 documents · 12 links to 6 bundles · last written today by Archivist". */
  meta: ReactNode;
  onOpen: () => void;
  onExport: () => void;
  onDelete: () => void;
} & HTMLAttributes<HTMLElement>;

/** The bundle selected in the all-bundles view, and what can be done with it. */
export function SelectedBundle({ name, meta, onOpen, onExport, onDelete, className, ...rest }: SelectedBundleProps) {
  return (
    <section aria-label={`Bundle ${name}`} className={cx("flex flex-col gap-10", className)} {...rest}>
      <h3 className="flex items-center gap-8 type-code-medium text-fg">
        <Icon name="folder" size={15} tone="muted" />
        {name}
      </h3>
      <p className="type-small-para text-muted">{meta}</p>
      <div className="flex gap-8">
        <Button variant="primary" onClick={onOpen}>Open bundle</Button>
        <Button onClick={onExport}>Export .zip</Button>
        <Button variant="danger" onClick={onDelete}>Delete</Button>
      </div>
    </section>
  );
}
