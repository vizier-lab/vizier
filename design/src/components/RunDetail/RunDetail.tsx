import { useState, type HTMLAttributes, type ReactNode } from "react";
import { cx } from "../../lib/cx";
import { CodeBlock } from "../CodeBlock/CodeBlock";
import { Eyebrow } from "../Eyebrow/Eyebrow";
import { TextLink } from "../TextLink/TextLink";

export type RunDetailProps = {
  /** What the model said the script was for. */
  description?: ReactNode;
  code: string;
  /** ScriptCalls made from inside the script. */
  calls?: ReactNode;
  callCount?: number;
  /** What the script printed. */
  output?: string;
  /** What it returned. */
  result?: string;
  /** A raised exception: shows the traceback block instead of a result. */
  traceback?: { where: string; message: string; full?: string };
  /** A sandbox limit stopped it: amber, which limit and how far it got. */
  limit?: { title: ReactNode; detail: ReactNode };
} & HTMLAttributes<HTMLDivElement>;

function Section({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-8">
      <Eyebrow as="h4">{label}</Eyebrow>
      {children}
    </section>
  );
}

/** Everything a python run did (X48): the script, the calls it made, what it printed and returned. */
export function RunDetail({ description, code, calls, callCount, output, result, traceback, limit, className, ...rest }: RunDetailProps) {
  const [full, setFull] = useState(false);
  const lines = code.replace(/\n$/, "").split("\n").length;
  return (
    <div className={cx("flex flex-col gap-16", className)} {...rest}>
      {description && <p className="type-ui-para text-soft">{description}</p>}
      <Section label={`Code · ${lines} lines`}>
        <CodeBlock code={code} language="python" lineNumbers />
      </Section>
      {calls && <Section label={callCount != null ? `Calls inside · ${callCount}` : "Calls inside"}>{calls}</Section>}
      {output && (
        <Section label={`Output · ${output.replace(/\n$/, "").split("\n").length} lines`}>
          <pre className="overflow-x-auto rounded-md bg-ground px-12 py-10 type-code-block text-soft">{output}</pre>
        </Section>
      )}
      {traceback && (
        <div role="alert" className="flex flex-col gap-4 rounded-md border border-failed-line bg-failed-bg px-12 py-10">
          <pre className="overflow-x-auto type-code-block text-soft">{full && traceback.full ? traceback.full : traceback.where}</pre>
          <p className="type-code text-failed">{traceback.message}</p>
          {traceback.full && (
            <div>
              <TextLink onClick={() => setFull(!full)} aria-expanded={full}>
                {full ? "Hide full traceback" : "Show full traceback"}
              </TextLink>
            </div>
          )}
        </div>
      )}
      {limit && (
        <div role="status" className="flex items-start gap-8 rounded-md border border-retrying-line bg-retrying-bg px-12 py-10">
          <span aria-hidden className="mt-6 size-7 shrink-0 rounded-pill bg-retrying" />
          <div className="flex flex-col gap-2">
            <p className="type-ui-medium text-fg">{limit.title}</p>
            <p className="type-mono-sm text-muted">{limit.detail}</p>
          </div>
        </div>
      )}
      {result && !traceback && (
        <Section label="Result">
          <CodeBlock code={result} language="json" />
        </Section>
      )}
    </div>
  );
}
