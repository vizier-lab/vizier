#!/usr/bin/env node
// FR-015: no raw values outside src/tokens/. See contracts/scripts.md.
//
// Fails on hex / rgb() / hsl() colour literals, Tailwind arbitrary values for
// colour, spacing, radius or font size, `px` literals in CSS, and padding,
// margin or gap off the space scale. Logo brand colours are allowed in the two
// brands.ts files. Prints `file:line: match` per violation.

import { readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const src = path.join(root, "src");

const EXEMPT_DIRS = [path.join(src, "tokens")];
const BRAND_FILES = new Set([
  path.join(src, "components/ProviderLogo/brands.ts"),
  path.join(src, "components/IntegrationLogo/brands.ts"),
]);

const SPACE_SCALE = new Set(["0", "px", "2", "4", "6", "8", "10", "12", "14", "16", "20", "24", "32", "48", "auto"]);

// Hex colours and rgb()/hsl(). Three- or four-digit runs of plain digits ("PR #418")
// are issue numbers, not colours.
const COLOUR = /#(?![0-9]{3,4}\b)[0-9a-fA-F]{3,8}\b|\b(?:rgba?|hsla?)\(/g;
// Arbitrary values on colour, spacing, radius, size and type utilities.
const ARBITRARY =
  /(?<![\w-])-?(?:bg|text|border(?:-[trblxy])?|ring|outline|fill|stroke|shadow|from|via|to|divide|accent|caret|decoration|p[trblxy]?|m[trblxy]?|gap(?:-[xy])?|space-[xy]|inset(?:-[xy])?|top|right|bottom|left|rounded(?:-[trbl]{1,2})?|leading|tracking)-\[[^\]]+\]/g;
const SPACING = /(?<![\w-])-?(?:p[trblxy]?|m[trblxy]?|gap(?:-[xy])?|space-[xy])-(\d+(?:\.\d+)?)(?![\w.])/g;
const CSS_PX = /(?<![\w.-])(\d*\.?\d+)px\b/g;

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    const p = path.join(dir, name);
    if (EXEMPT_DIRS.some((d) => p.startsWith(d))) continue;
    if (statSync(p).isDirectory()) walk(p, out);
    else if (/\.(tsx?|css)$/.test(name)) out.push(p);
  }
  return out;
}

const violations = [];
for (const file of walk(src)) {
  // Blank out block comments, keeping line numbers.
  const text = readFileSync(file, "utf8").replace(/\/\*[\s\S]*?\*\//g, (c) => c.replace(/[^\n]/g, " "));
  const lines = text.split("\n");
  const isCss = file.endsWith(".css");
  const brand = BRAND_FILES.has(file);
  lines.forEach((line, i) => {
    // Comments may mention values.
    const code = line.replace(/(^|[^:])\/\/.*$/, "$1");
    const report = (m) => violations.push(`${path.relative(root, file)}:${i + 1}: ${m}`);
    if (!brand) for (const m of code.matchAll(COLOUR)) report(m[0]);
    for (const m of code.matchAll(ARBITRARY)) report(m[0]);
    if (!isCss) for (const m of code.matchAll(SPACING)) if (!SPACE_SCALE.has(m[1])) report(m[0]);
    if (isCss) for (const m of code.matchAll(CSS_PX)) if (m[1] !== "0" && m[1] !== "1") report(m[0]);
  });
}

if (violations.length) {
  for (const v of violations) console.log(v);
  console.error(`error: ${violations.length} raw value${violations.length === 1 ? "" : "s"} outside src/tokens/`);
  process.exit(1);
}
