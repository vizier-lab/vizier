import { createServer } from 'node:http';

const wait = (ms, signal) => new Promise((resolve, reject) => {
  if (signal.aborted) return reject(signal.reason);
  const timer = setTimeout(resolve, ms);
  signal.addEventListener('abort', () => { clearTimeout(timer); reject(signal.reason); }, { once: true });
});

function redactHeaders(headers) {
  const out = {};
  for (const [name, value] of Object.entries(headers)) {
    out[name] = /authorization|cookie|api[-_]?key|token/i.test(name) ? '[REDACTED]' : value;
  }
  return out;
}

function redactBody(raw) {
  try {
    const visit = (value) => {
      if (Array.isArray(value)) return value.map(visit);
      if (!value || typeof value !== 'object') return value;
      return Object.fromEntries(Object.entries(value).map(([key, child]) => [
        key,
        /authorization|api[-_]?key|access[-_]?token|secret|password/i.test(key) ? '[REDACTED]' : visit(child),
      ]));
    };
    return JSON.stringify(visit(JSON.parse(raw.toString('utf8'))));
  } catch {
    return '[NON_JSON_BODY_OMITTED]';
  }
}

function redactPath(path) {
  const url = new URL(path ?? '/', 'http://localhost');
  for (const key of url.searchParams.keys()) {
    if (/key|token|secret|password/i.test(key)) url.searchParams.set(key, '[REDACTED]');
  }
  return `${url.pathname}${url.search}`;
}

/** A per-test forwarding proxy. It owns plans and captures; dummyplug stays stateless. */
export function createDummyplugTestProxy({ upstream = process.env.DUMMYPLUG_UPSTREAM ?? 'http://127.0.0.1:9998' } = {}) {
  const plans = new Map();
  const captures = [];
  const server = createServer(async (request, response) => {
    const controller = new AbortController();
    const cancel = () => controller.abort(new Error('client disconnected'));
    request.once('aborted', cancel);
    response.once('close', () => { if (!response.writableEnded) cancel(); });
    const chunks = [];
    try {
      for await (const chunk of request) chunks.push(chunk);
      const rawBody = Buffer.concat(chunks);
      const scenario = String(request.headers['x-vizier-test-scenario'] ?? 'default');
      const plan = plans.get(scenario) ?? [];
      const target = new URL(request.url ?? '/', upstream);
      // Provider setup and catalogue refresh also call /v1/models. A response plan is for chat
      // attempts in one turn; discovery requests must not consume retries or injected replies.
      const isChatCompletion = request.method === 'POST' && target.pathname.endsWith('/chat/completions');
      const step = isChatCompletion ? plan.shift() : undefined;
      captures.push({ at: Date.now(), scenario, method: request.method, path: redactPath(request.url), headers: redactHeaders(request.headers), body: redactBody(rawBody) });

      if (step?.delay_ms) await wait(step.delay_ms, controller.signal);
      if (step?.hold) await new Promise((resolve, reject) => {
        step.release = resolve;
        controller.signal.addEventListener('abort', () => reject(controller.signal.reason), { once: true });
      });
      if (step?.status) {
        const body = typeof step.body === 'string' ? step.body : JSON.stringify(step.body ?? { error: { message: 'injected test failure' } });
        response.writeHead(step.status, { 'content-type': 'application/json', ...(step.retry_after ? { 'retry-after': String(step.retry_after) } : {}) });
        response.end(body);
        return;
      }
      const upstreamResponse = await fetch(target, {
        method: request.method,
        headers: Object.fromEntries(Object.entries(request.headers).filter(([name]) => !['host', 'connection', 'content-length', 'x-vizier-test-scenario'].includes(name))),
        body: ['GET', 'HEAD'].includes(request.method ?? 'GET') ? undefined : rawBody,
        signal: controller.signal,
      });
      let responseBody = Buffer.from(await upstreamResponse.arrayBuffer());
      if (step?.body !== undefined) {
        responseBody = Buffer.from(typeof step.body === 'string' ? step.body : JSON.stringify(step.body));
      }
      const headers = Object.fromEntries(upstreamResponse.headers.entries());
      delete headers['content-length'];
      if (step?.status) response.writeHead(step.status, headers); else response.writeHead(upstreamResponse.status, headers);
      response.end(responseBody);
    } catch (error) {
      if (!response.destroyed && !response.writableEnded) {
        response.writeHead(controller.signal.aborted ? 499 : 502, { 'content-type': 'application/json' });
        response.end(JSON.stringify({ error: { message: controller.signal.aborted ? 'request cancelled' : 'upstream unavailable' } }));
      }
    } finally {
      request.removeListener('aborted', cancel);
    }
  });
  return {
    server,
    setPlan(scenario, steps) { plans.set(String(scenario), [...steps]); },
    requests(scenario) { return captures.filter((item) => scenario === undefined || item.scenario === String(scenario)).map((item) => ({ ...item, headers: { ...item.headers } })); },
    clear(scenario) {
      if (scenario === undefined) { plans.clear(); captures.length = 0; return; }
      plans.delete(String(scenario));
      for (let i = captures.length - 1; i >= 0; i -= 1) if (captures[i].scenario === String(scenario)) captures.splice(i, 1);
    },
    async close() {
      for (const plan of plans.values()) for (const step of plan) step.release?.();
      if (server.listening) await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
      plans.clear(); captures.length = 0;
    },
  };
}
