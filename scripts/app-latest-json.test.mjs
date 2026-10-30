import test from 'node:test';
import assert from 'node:assert/strict';
import { buildManifest } from './app-latest-json.mjs';

const sigs = {
  'Vizier_universal.app.tar.gz.sig': 'MAC',
  'Vizier_1.2.3_amd64.AppImage.sig': 'AMD',
  'Vizier_1.2.3_aarch64.AppImage.sig': 'ARM',
};

test('manifest maps both macOS architectures to the universal payload', () => {
  const m = buildManifest({
    version: '1.2.3',
    notes: 'n',
    pubDate: '2026-01-01T00:00:00Z',
    repo: 'vizier-lab/vizier',
    readSig: (name) => sigs[name],
  });
  const base = 'https://github.com/vizier-lab/vizier/releases/download/v1.2.3';
  assert.equal(m.version, '1.2.3');
  assert.equal(m.pub_date, '2026-01-01T00:00:00Z');
  assert.deepEqual(m.platforms['darwin-aarch64'], { url: `${base}/Vizier_universal.app.tar.gz`, signature: 'MAC' });
  assert.deepEqual(m.platforms['darwin-x86_64'], m.platforms['darwin-aarch64']);
  assert.deepEqual(m.platforms['linux-x86_64'], { url: `${base}/Vizier_1.2.3_amd64.AppImage`, signature: 'AMD' });
  assert.deepEqual(m.platforms['linux-aarch64'], { url: `${base}/Vizier_1.2.3_aarch64.AppImage`, signature: 'ARM' });
  assert.deepEqual(Object.keys(m.platforms).sort(), ['darwin-aarch64', 'darwin-x86_64', 'linux-aarch64', 'linux-x86_64']);
});
