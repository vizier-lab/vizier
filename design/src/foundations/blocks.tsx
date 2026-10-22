/**
 * Building blocks for the Foundations pages. Every value shown is read from
 * the live CSS (getComputedStyle), so tokens.css stays the only place a
 * value is written.
 */
import { useEffect, useState, type ReactNode } from "react";
import { Icon } from "../icons/Icon";
import type { IconName } from "../icons/map";
import {
  colorTokens,
  elevations,
  sizeTokens,
  toneAliases,
  toneModes,
  typeStyles,
  type TokenGroup,
} from "../tokens/tokens";

export type Theme = "dark" | "light";

/** Reads each CSS variable as it resolves in the given theme. */
export function readVars(vars: string[], theme: Theme): Record<string, string> {
  const html = document.documentElement;
  const before = html.getAttribute("data-theme");
  html.setAttribute("data-theme", theme);
  const style = getComputedStyle(html);
  const out = Object.fromEntries(vars.map((v) => [v, style.getPropertyValue(v).trim()]));
  if (before === null) html.removeAttribute("data-theme");
  else html.setAttribute("data-theme", before);
  return out;
}

/** Token values in both themes, re-read whenever the theme attribute changes. */
export function useBothThemes(vars: string[]) {
  const read = () => ({ dark: readVars(vars, "dark"), light: readVars(vars, "light") });
  const [values, setValues] = useState(read);
  useEffect(() => {
    const obs = new MutationObserver(() => {
      setValues(read());
      // Reading flips data-theme twice; drop those records or this loops.
      obs.takeRecords();
    });
    obs.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => obs.disconnect();
  }, []);
  return values;
}

function channel(c: number) {
  const s = c / 255;
  return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
}

function luminance(hex: string) {
  const m = hex.replace("#", "").match(/../g);
  if (!m) return 0;
  const [r, g, b] = m.map((x) => parseInt(x, 16));
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

/** WCAG contrast ratio of two #rrggbb colours. */
export function contrast(a: string, b: string) {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}

const GROUP_LABEL: Record<TokenGroup, string> = {
  bg: "Background",
  line: "Line",
  dot: "Dot",
  text: "Text",
  fill: "Fill",
  state: "State",
  data: "Data",
};

function Swatch({ value, label }: { value: string; label: string }) {
  return (
    <div className="flex items-center gap-8">
      <span
        role="img"
        aria-label={`${label} ${value}`}
        className="size-24 shrink-0 rounded-sm border border-border-strong"
        style={{ background: value }}
      />
      <code className="type-mono-sm text-soft">{value}</code>
    </div>
  );
}

function Label({ children }: { children: ReactNode }) {
  return <th className="px-8 py-6 text-left type-label uppercase text-muted">{children}</th>;
}

/** Every colour token with its Dark and Light values side by side. */
export function ColourTable() {
  const vars = colorTokens.map((t) => t.cssVar);
  const values = useBothThemes([...vars, "--vz-ground"]);
  const groups = Array.from(new Set(colorTokens.map((t) => t.group)));
  return (
    <div className="flex flex-col gap-24 text-fg">
      {groups.map((group) => (
        <table key={group} className="w-full border-collapse">
          <caption className="pb-8 text-left type-heading">{GROUP_LABEL[group]}</caption>
          <thead>
            <tr className="border-b border-border">
              <Label>Token</Label>
              <Label>Dark</Label>
              <Label>Light</Label>
              <Label>Use</Label>
            </tr>
          </thead>
          <tbody>
            {colorTokens
              .filter((t) => t.group === group)
              .map((t) => {
                const isText = t.group === "text";
                const ratios = isText
                  ? (["dark", "light"] as const).map((th) => contrast(values[th][t.cssVar], values[th]["--vz-ground"]))
                  : [];
                const low = ratios.some((r) => r < 4.5);
                return (
                  <tr key={t.cssVar} className="border-b border-border align-top">
                    <td className="px-8 py-8">
                      <div className="type-ui-medium">{t.name}</div>
                      <code className="type-mono-xs text-muted">
                        var({t.cssVar}) · {t.tw}
                      </code>
                    </td>
                    {(["dark", "light"] as const).map((th, i) => (
                      <td key={th} className="px-8 py-8">
                        <Swatch value={values[th][t.cssVar]} label={`${t.name} in ${th}`} />
                        {isText && (
                          <div className="pt-4 type-mono-xs text-muted">{ratios[i].toFixed(2)}:1 on ground</div>
                        )}
                      </td>
                    ))}
                    <td className="px-8 py-8 type-ui-para text-muted">
                      {t.description}
                      {low && (
                        <div className="pt-4 type-small-medium text-retrying">
                          Below 4.5:1: never for body text.
                        </div>
                      )}
                    </td>
                  </tr>
                );
              })}
          </tbody>
        </table>
      ))}
    </div>
  );
}

/** Each type style as a specimen with its computed metrics. */
export function TypeSpecimens() {
  return (
    <div className="flex flex-col text-fg">
      {typeStyles.map((t) => (
        <TypeRow key={t.utility} utility={t.utility} name={t.name} description={t.description} />
      ))}
    </div>
  );
}

function TypeRow({ utility, name, description }: { utility: string; name: string; description: string }) {
  const [metrics, setMetrics] = useState("");
  const ref = (el: HTMLElement | null) => {
    if (!el || metrics) return;
    const s = getComputedStyle(el);
    const family = s.fontFamily.split(",")[0].replace(/"/g, "").replace(" Variable", "");
    const tracking = s.letterSpacing === "normal" ? "" : ` · ${s.letterSpacing}`;
    setMetrics(`${family} · ${s.fontSize} · ${s.fontWeight} · ${s.lineHeight}${tracking}`);
  };
  const sample = utility === "type-label" ? "SECTION LABEL · 12" : "Archivist reviewed vector-db-review";
  return (
    <div className="grid grid-cols-[220px_1fr] gap-16 border-b border-border py-12">
      <div>
        <div className="type-mono-sm-medium">{utility}</div>
        <div className="type-mono-xs text-muted">{name}</div>
        <div className="pt-4 type-small text-muted">{description}</div>
      </div>
      <div className="flex flex-col gap-4">
        <div ref={ref} className={utility}>
          {sample}
        </div>
        <div className="type-mono-xs text-faint">{metrics}</div>
      </div>
    </div>
  );
}

/** Radius tokens as boxes, space tokens as bars. */
export function SizeScale() {
  const radii = sizeTokens.filter((t) => t.kind === "radius");
  const spaces = sizeTokens.filter((t) => t.kind === "space");
  const v = useBothThemes(sizeTokens.map((t) => t.cssVar)).dark;
  return (
    <div className="flex flex-col gap-32 text-fg">
      <section>
        <h3 className="pb-12 type-heading">Radius</h3>
        <div className="flex flex-wrap gap-24">
          {radii.map((t) => (
            <figure key={t.cssVar} className="flex flex-col items-center gap-8">
              <div
                className="size-64 border border-border-strong bg-surface"
                style={{ borderRadius: `var(${t.cssVar})` }}
              />
              <figcaption className="text-center">
                <div className="type-mono-sm">rounded-{t.name.split("/")[1]}</div>
                <div className="type-mono-xs text-muted">{v[t.cssVar]}</div>
              </figcaption>
            </figure>
          ))}
        </div>
      </section>
      <section>
        <h3 className="pb-12 type-heading">Space</h3>
        <p className="pb-12 type-ui-para text-muted">
          Padding, margin and gap use only these steps (<code>p-12</code>, <code>gap-8</code>);{" "}
          <code>npm run lint:tokens</code> rejects others.
        </p>
        <div className="flex flex-col gap-8">
          {spaces.map((t) => (
            <div key={t.cssVar} className="grid grid-cols-[120px_60px_1fr] items-center gap-12">
              <code className="type-mono-sm">space/{t.name.split("/")[1]}</code>
              <code className="type-mono-xs text-muted">{v[t.cssVar]}</code>
              <div className="h-12 rounded-xs bg-ink" style={{ width: `var(${t.cssVar})` }} />
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}

/** The two elevation shadows on a surface. */
export function ElevationSamples() {
  return (
    <div className="flex flex-wrap gap-48 rounded-xl bg-ground p-48 text-fg">
      {elevations.map((e) => (
        <div key={e.name} className={`flex h-120 w-240 flex-col justify-end gap-4 rounded-lg border border-border bg-surface p-16 ${e.utility}`}>
          <div className="type-ui-medium">{e.name}</div>
          <code className="type-mono-xs text-muted">{e.utility}</code>
          <div className="type-small text-muted">{e.description}</div>
        </div>
      ))}
    </div>
  );
}

const TONE_ICONS: IconName[] = ["core", "memory", "tasks", "alert", "check", "refresh"];

/** The seven tone modes applied to a row of icons. */
export function ToneRows() {
  return (
    <table className="w-full border-collapse text-fg">
      <thead>
        <tr className="border-b border-border">
          <Label>Tone</Label>
          <Label>Resolves to</Label>
          <Label>Icons</Label>
        </tr>
      </thead>
      <tbody>
        {toneModes.map((t) => (
          <tr key={t} className="border-b border-border">
            <td className="px-8 py-10 type-mono-sm">tone="{t}"</td>
            <td className="px-8 py-10 type-mono-xs text-muted">var({toneAliases[t]})</td>
            <td className={t === "inverse" ? "bg-ink px-8 py-10" : "px-8 py-10"}>
              <div className="flex gap-12">
                {TONE_ICONS.map((n) => (
                  <Icon key={n} name={n} tone={t} />
                ))}
              </div>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
