// The scripts of the logic sandbox (contracts/logic-sandbox.md): a worker that runs the agent's
// functions, a relay in the sandboxed frame that connects it to the page, and the frame's
// document. Plain JavaScript in strings, so the code that ships is the code that is tested (a
// `vm` test runs the worker text), and no bundler helper can leak into the frame.
//
// The worker removes what a function could reach for (timers, network, storage), says hello,
// receives the function definitions and answers `call` messages with JSON. The relay believes
// only its parent window.

export const MAX_CALL_DEPTH = 32
export const ERROR_CHARS = 300

/**
 * The functions' own thread. Agent code runs in a Worker so that a loop that never ends burns
 * that thread and nothing else: a script in a sandboxed frame can share the page's thread, and
 * the host's timer could then never fire. The frame's CSP is inherited by the worker, so it has
 * no network either. Plain JavaScript in a string, so the code that ships is the code tested.
 */
export const WORKER_SOURCE = `
(function () {
  'use strict';
  var blocked = ['setTimeout', 'setInterval', 'setImmediate', 'requestAnimationFrame', 'requestIdleCallback',
    'fetch', 'XMLHttpRequest', 'WebSocket', 'EventSource', 'Worker', 'SharedWorker', 'importScripts',
    'indexedDB', 'localStorage', 'sessionStorage', 'open', 'navigator', 'BroadcastChannel'];
  blocked.forEach(function (name) {
    try { Object.defineProperty(self, name, { value: undefined, configurable: true, writable: true }); } catch (e) {}
  });

  var defs = {};
  var compiled = {};
  var depth = 0;
  var fns = {};

  function say(message) { self.postMessage(message); }

  function text(error) {
    var message = error && error.message !== undefined ? error.message : error;
    return String(message).slice(0, ${ERROR_CHARS});
  }

  function lookup(name) {
    if (Object.prototype.hasOwnProperty.call(compiled, name)) return compiled[name];
    var def = Object.prototype.hasOwnProperty.call(defs, name) ? defs[name] : null;
    if (!def) throw new Error('no function named ' + name);
    var make = Function.apply(null, ['fns'].concat(def.params, [def.body]));
    compiled[name] = function () {
      if (depth >= ${MAX_CALL_DEPTH}) throw new Error('call depth exceeded');
      depth++;
      try { return make.apply(null, [fns].concat(Array.prototype.slice.call(arguments))); }
      finally { depth--; }
    };
    return compiled[name];
  }

  fns = new Proxy({}, { get: function (_, name) { return typeof name === 'string' ? lookup(name) : undefined; } });

  self.addEventListener('message', function (event) {
    var m = event.data;
    if (!m || typeof m !== 'object') return;
    if (m.type === 'init' && m.defs && typeof m.defs === 'object') {
      defs = m.defs; compiled = {};
      return;
    }
    if (m.type !== 'call' || typeof m.id !== 'number') return;
    try {
      var result = lookup(String(m.fn)).apply(null, Array.isArray(m.args) ? m.args : []);
      var json = JSON.stringify(result === undefined ? null : result);
      if (json === undefined) throw new Error('the result is not JSON');
      say({ type: 'result', id: m.id, ok: true, json: json });
    } catch (error) {
      say({ type: 'result', id: m.id, ok: false, error: text(error) });
    }
  });

  say({ type: 'hello' });
})();
`

/**
 * The frame's script: starts the worker and relays between it and the page. It believes only its
 * parent window, and passes the page's messages on without reading them.
 */
export const RELAY_SOURCE = `
(function () {
  var host = window.parent;
  var url = URL.createObjectURL(new Blob([${JSON.stringify(WORKER_SOURCE).replace(/</g, '\\u003c')}], { type: 'text/javascript' }));
  var worker = new Worker(url);
  worker.onmessage = function (event) { host.postMessage(event.data, '*'); };
  window.addEventListener('message', function (event) {
    if (event.source !== host) return;
    worker.postMessage(event.data);
  });
})();
`

/** The frame's document: the CSP first, then the relay. No style, image or network source. */
export const RUNNER_DOCUMENT = `<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-eval' 'unsafe-inline' blob:; worker-src blob:; connect-src 'none'; img-src 'none'; style-src 'none'">
</head><body><script>${RELAY_SOURCE}</script></body></html>`
