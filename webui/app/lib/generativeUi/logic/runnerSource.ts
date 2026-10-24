// The script that runs inside the logic sandbox frame (contracts/logic-sandbox.md). Plain
// JavaScript in a string, so the code that ships is the code that is tested (a `vm` test runs
// this very text), and no bundler helper can leak into the frame.
//
// It does four things: remove what a function could reach for (timers, network, storage, the
// parent), say hello, receive the function definitions, and answer `call` messages with JSON.
// It talks to nobody but its parent, and only believes its parent.

export const MAX_CALL_DEPTH = 32
export const ERROR_CHARS = 300

export const RUNNER_SOURCE = `
(function () {
  'use strict';
  var host = window.parent;
  var blocked = ['setTimeout', 'setInterval', 'setImmediate', 'requestAnimationFrame', 'requestIdleCallback',
    'fetch', 'XMLHttpRequest', 'WebSocket', 'EventSource', 'Worker', 'SharedWorker', 'importScripts',
    'indexedDB', 'localStorage', 'sessionStorage', 'open', 'navigator', 'BroadcastChannel'];
  blocked.forEach(function (name) {
    try { Object.defineProperty(window, name, { value: undefined, configurable: true, writable: true }); } catch (e) {}
  });

  var defs = {};
  var compiled = {};
  var depth = 0;
  var fns = {};

  function say(message) { host.postMessage(message, '*'); }

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

  window.addEventListener('message', function (event) {
    if (event.source !== host) return;
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

/** The frame's document: the CSP first, then the runner. No style, image or network source. */
export const RUNNER_DOCUMENT = `<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-eval' 'unsafe-inline'; connect-src 'none'; img-src 'none'; style-src 'none'">
</head><body><script>${RUNNER_SOURCE}</script></body></html>`
