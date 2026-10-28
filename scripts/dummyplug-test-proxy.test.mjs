import assert from 'node:assert/strict';
import test from 'node:test';
import { createDummyplugTestProxy } from './dummyplug-test-proxy.mjs';

function listen(server) { return new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', () => resolve(`http://127.0.0.1:${server.address().port}`)); }); }

test('proxy injects scoped failures, redacts credentials, and cleans scenario state', async (t) => {
  const upstream = (await import('node:http')).createServer((req, res) => { res.writeHead(200, { 'content-type': 'application/json' }); res.end('{"ok":true}'); });
  const origin = await listen(upstream);
  const proxy = createDummyplugTestProxy({ upstream: origin });
  const address = await listen(proxy.server);
  t.after(async () => { await proxy.close(); await new Promise((resolve) => upstream.close(resolve)); });
  proxy.setPlan('retry', [{ status: 429, retry_after: 1, body: { error: { message: 'retry' } } }]);
  const headers = { authorization: 'Bearer secret', 'x-vizier-test-scenario': 'retry' };
  const models = await fetch(`${address}/v1/models`, { headers });
  assert.equal(models.status, 200);
  const failed = await fetch(`${address}/v1/chat/completions`, { method: 'POST', headers, body: JSON.stringify({ api_key: 'body-secret' }) });
  assert.equal(failed.status, 429);
  assert.equal(failed.headers.get('retry-after'), '1');
  const capture = proxy.requests('retry').find((item) => item.path === '/v1/chat/completions');
  assert.equal(capture.headers.authorization, '[REDACTED]');
  assert.equal(JSON.parse(capture.body).api_key, '[REDACTED]');
  proxy.clear('retry');
  assert.deepEqual(proxy.requests('retry'), []);
});

test('plans remain isolated and may substitute delayed responses', async (t) => {
  const upstream = (await import('node:http')).createServer((req, res) => { res.writeHead(200, { 'content-type': 'application/json' }); res.end('{"upstream":true}'); });
  const origin = await listen(upstream);
  const proxy = createDummyplugTestProxy({ upstream: origin });
  const address = await listen(proxy.server);
  t.after(async () => { await proxy.close(); await new Promise((resolve) => upstream.close(resolve)); });
  proxy.setPlan('one', [{ delay_ms: 5, body: { injected: 1 } }]);
  proxy.setPlan('two', [{ status: 418, body: { injected: 2 } }]);
  const post = (scenario) => fetch(`${address}/v1/chat/completions`, { method: 'POST', headers: { 'x-vizier-test-scenario': scenario }, body: '{}' });
  const [one, two] = await Promise.all([post('one'), post('two')]);
  assert.deepEqual(await one.json(), { injected: 1 });
  assert.equal(two.status, 418);
  assert.deepEqual(await two.json(), { injected: 2 });
  assert.deepEqual(proxy.requests().map((item) => item.scenario).sort(), ['one', 'two']);
});

test('a client disconnect cancels a held proxy request', async (t) => {
  const upstream = (await import('node:http')).createServer((req, res) => res.end('unexpected upstream call'));
  const origin = await listen(upstream);
  const proxy = createDummyplugTestProxy({ upstream: origin });
  const address = await listen(proxy.server);
  t.after(async () => { await proxy.close(); await new Promise((resolve) => upstream.close(resolve)); });
  const held = { hold: true };
  proxy.setPlan('held', [held]);
  const controller = new AbortController();
  const pending = fetch(`${address}/v1/chat/completions`, { method: 'POST', headers: { 'x-vizier-test-scenario': 'held' }, body: '{}', signal: controller.signal }).catch(() => {});
  for (let i = 0; i < 20 && !proxy.requests('held').length; i += 1) await new Promise((resolve) => setTimeout(resolve, 5));
  assert.equal(proxy.requests('held').length, 1);
  controller.abort();
  await pending;
  assert.equal(typeof held.release, 'function');
});
