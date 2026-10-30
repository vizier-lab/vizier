// Writes the desktop app's updater manifest (`latest.json`) from the release assets.
// Contract: specs/038-desktop-app-edition/contracts/release-artifacts.md
//
//   node scripts/app-latest-json.mjs --version 1.2.3 --notes-file notes.md \
//        --dir release-assets --repo vizier-lab/vizier --out release-assets/latest.json

import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

export function buildManifest({ version, notes, pubDate, repo, readSig }) {
  const base = `https://github.com/${repo}/releases/download/v${version}`;
  const mac = 'Vizier_universal.app.tar.gz';
  const amd = `Vizier_${version}_amd64.AppImage`;
  const arm = `Vizier_${version}_aarch64.AppImage`;
  const entry = (file) => ({ url: `${base}/${file}`, signature: readSig(`${file}.sig`) });
  return {
    version,
    notes,
    pub_date: pubDate,
    platforms: {
      'darwin-aarch64': entry(mac),
      'darwin-x86_64': entry(mac),
      'linux-x86_64': entry(amd),
      'linux-aarch64': entry(arm),
    },
  };
}

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 2) out[argv[i].replace(/^--/, '')] = argv[i + 1];
  return out;
}

function main() {
  const a = parseArgs(process.argv.slice(2));
  for (const k of ['version', 'dir', 'repo', 'out']) {
    if (!a[k]) throw new Error(`missing --${k}`);
  }
  const notes = a['notes-file'] && existsSync(a['notes-file']) ? readFileSync(a['notes-file'], 'utf8').trim() : '';
  const readSig = (name) => {
    const path = join(a.dir, name);
    // A missing signature leaves the field empty: dry runs have no signing key, and the
    // release job refuses to publish without the files.
    return existsSync(path) ? readFileSync(path, 'utf8').trim() : '';
  };
  const manifest = buildManifest({
    version: a.version,
    notes,
    pubDate: new Date().toISOString(),
    repo: a.repo,
    readSig,
  });
  writeFileSync(a.out, JSON.stringify(manifest, null, 2) + '\n');
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
