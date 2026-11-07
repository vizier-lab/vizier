#!/usr/bin/env python3
"""Measure real language-server behaviour for spec 054: capabilities, readiness, and how fast
diagnostics arrive after an edit (pull vs push, with and without didSave)."""
import json, os, pathlib, select, subprocess, sys, tempfile, time

def uri(p): return pathlib.Path(p).as_uri()

class Lsp:
    def __init__(self, cmd, root):
        self.p = subprocess.Popen(cmd, cwd=root, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL)
        self.buf = b""; self.next = 0; self.t0 = time.time(); self.log = []
        self.push = []; self.progress = {}; self.ended = []; self.responses = {}
    def now(self): return round(time.time() - self.t0, 2)
    def send(self, obj):
        body = json.dumps({"jsonrpc": "2.0", **obj}).encode()
        self.p.stdin.write(b"Content-Length: %d\r\n\r\n" % len(body) + body); self.p.stdin.flush()
    def notify(self, m, params): self.send({"method": m, "params": params})
    def request(self, m, params):
        self.next += 1; self.send({"id": self.next, "method": m, "params": params}); return self.next
    def pump(self, secs):
        end = time.time() + secs
        while time.time() < end:
            r, _, _ = select.select([self.p.stdout], [], [], max(0, min(0.1, end - time.time())))
            if r:
                chunk = os.read(self.p.stdout.fileno(), 1 << 16)
                if not chunk: return
                self.buf += chunk
            while True:
                i = self.buf.find(b"\r\n\r\n")
                if i < 0: break
                n = int([l for l in self.buf[:i].split(b"\r\n") if l.lower().startswith(b"content-length")][0].split(b":")[1])
                if len(self.buf) < i + 4 + n: break
                self.handle(json.loads(self.buf[i + 4:i + 4 + n])); self.buf = self.buf[i + 4 + n:]
    def handle(self, m):
        if "id" in m and "method" in m:
            result = None
            if m["method"] == "workspace/configuration":
                result = [None] * len(m["params"]["items"])
            self.send({"id": m["id"], "result": result}); return
        if "id" in m:
            self.responses[m["id"]] = (self.now(), m); return
        if m.get("method") == "$/progress":
            v = m["params"]["value"]; tok = str(m["params"]["token"])
            if v["kind"] == "begin": self.progress[tok] = v.get("title"); print(f"  [{self.now()}] progress begin {v.get('title')!r}")
            if v["kind"] == "end":
                print(f"  [{self.now()}] progress end   {self.progress.pop(tok, tok)!r}"); self.ended.append(self.now())
        if m.get("method") == "textDocument/publishDiagnostics":
            d = m["params"]
            self.push.append((self.now(), d["uri"].rsplit("/", 1)[-1], d.get("version"), [x["message"][:60] for x in d["diagnostics"]]))
            print(f"  [{self.now()}] PUSH {d['uri'].rsplit('/',1)[-1]} v={d.get('version')} n={len(d['diagnostics'])} {[x.get('source') for x in d['diagnostics']]}")
    def wait(self, rid, secs):
        end = time.time() + secs
        while rid not in self.responses and time.time() < end: self.pump(0.1)
        return self.responses.get(rid)

CAPS = {"general": {"positionEncodings": ["utf-8", "utf-16"]},
        "window": {"workDoneProgress": True},
        "workspace": {"configuration": True, "didChangeWatchedFiles": {"dynamicRegistration": False}},
        "textDocument": {"publishDiagnostics": {"versionSupport": True},
                         "diagnostic": {"dynamicRegistration": False, "relatedDocumentSupport": False},
                         "synchronization": {"didSave": True}}}

def start(cmd, root):
    s = Lsp(cmd, root)
    rid = s.request("initialize", {"processId": os.getpid(), "rootUri": uri(root), "capabilities": CAPS,
                                   "workspaceFolders": [{"uri": uri(root), "name": "w"}]})
    t, m = s.wait(rid, 30)
    caps = m["result"]["capabilities"]
    print(f"initialize answered at {t}s")
    print("  positionEncoding:", caps.get("positionEncoding"))
    print("  diagnosticProvider:", json.dumps(caps.get("diagnosticProvider")))
    print("  textDocumentSync:", json.dumps(caps.get("textDocumentSync")))
    s.notify("initialized", {})
    return s, caps

def rust():
    root = tempfile.mkdtemp(prefix="vz-lsp-")
    os.makedirs(f"{root}/src")
    pathlib.Path(f"{root}/Cargo.toml").write_text('[package]\nname="spike"\nversion="0.1.0"\nedition="2021"\n')
    lib = f"{root}/src/lib.rs"; other = f"{root}/src/other.rs"
    pathlib.Path(lib).write_text('pub mod other;\npub fn f(a: u32) -> u32 { a + 1 }\n')
    pathlib.Path(other).write_text('pub fn g() -> u32 { crate::f(1) }\n')
    print("== rust-analyzer ==")
    s, caps = start(["rust-analyzer"], root)
    s.notify("textDocument/didOpen", {"textDocument": {"uri": uri(lib), "languageId": "rust", "version": 1, "text": pathlib.Path(lib).read_text()}})
    s.notify("textDocument/didOpen", {"textDocument": {"uri": uri(other), "languageId": "rust", "version": 1, "text": pathlib.Path(other).read_text()}})
    # Readiness: quiet 3s after the last progress end, 60s at most.
    t = time.time()
    while time.time() - t < 60:
        s.pump(0.5)
        if not s.progress and s.ended and s.now() - s.ended[-1] > 3: break
    print(f"indexed by {s.now()}s; open progress: {list(s.progress.values())}")
    s.pump(5)

    print("-- definition on a position (ready?) --")
    rid = s.request("textDocument/definition", {"textDocument": {"uri": uri(other)}, "position": {"line": 0, "character": 27}})
    r = s.wait(rid, 10); print("  definition:", json.dumps(r[1].get("result"))[:160] if r else "no answer in 10s")

    # A signature change that breaks the OTHER file, and a type error in the edited file.
    print("-- edit: signature change + local type error (in memory, no save) --")
    broken = 'pub mod other;\npub fn f(a: u32, b: u32) -> u32 { a + b }\npub fn h() -> u32 { "s" }\n'
    pathlib.Path(lib).write_text(broken)
    edit_at = s.now(); s.push.clear()
    s.notify("textDocument/didChange", {"textDocument": {"uri": uri(lib), "version": 2}, "contentChanges": [{"text": broken}]})
    if caps.get("diagnosticProvider"):
        rid = s.request("textDocument/diagnostic", {"textDocument": {"uri": uri(lib)}})
        r = s.wait(rid, 10)
        if r: print(f"  PULL edited file at +{round(r[0]-edit_at,2)}s:", [x["message"][:50] for x in r[1].get("result", {}).get("items", [])] if "result" in r[1] else r[1].get("error"))
        rid = s.request("textDocument/diagnostic", {"textDocument": {"uri": uri(other)}})
        r = s.wait(rid, 10)
        if r: print(f"  PULL other file at +{round(r[0]-edit_at,2)}s:", [x["message"][:50] for x in r[1].get("result", {}).get("items", [])] if "result" in r[1] else r[1].get("error"))
    s.pump(3)
    print(f"  pushes within 3s of the edit: {[(round(a-edit_at,2),b,c,d) for a,b,c,d in s.push]}")

    print("-- didSave --")
    save_at = s.now(); s.push.clear()
    s.notify("textDocument/didSave", {"textDocument": {"uri": uri(lib)}})
    s.pump(25)
    print(f"  pushes after save: {[(round(a-save_at,2),b,c,d) for a,b,c,d in s.push]}")

    rid = s.request("shutdown", None); s.wait(rid, 5); s.notify("exit", None)
    s.p.wait(timeout=10); print("  exit code", s.p.returncode)

    print("-- stdin close ends the server? --")
    s2 = Lsp(["rust-analyzer"], root); s2.p.stdin.close()
    try: s2.p.wait(timeout=5); print("  exited on EOF, code", s2.p.returncode)
    except subprocess.TimeoutExpired: print("  STILL RUNNING 5s after EOF"); s2.p.kill()

def clangd():
    root = tempfile.mkdtemp(prefix="vz-c-")
    c = f"{root}/a.c"; pathlib.Path(c).write_text("int f(int a) { return a; }\n")
    print("== clangd ==")
    s, caps = start(["clangd"], root)
    s.notify("textDocument/didOpen", {"textDocument": {"uri": uri(c), "languageId": "c", "version": 1, "text": pathlib.Path(c).read_text()}})
    s.pump(2); s.push.clear()
    broken = "int f(int a) { return b; }\n"; pathlib.Path(c).write_text(broken); at = s.now()
    s.notify("textDocument/didChange", {"textDocument": {"uri": uri(c), "version": 2}, "contentChanges": [{"text": broken}]})
    s.pump(3)
    print(f"  pushes within 3s: {[(round(a-at,2),b,cv,d) for a,b,cv,d in s.push]}")
    rid = s.request("shutdown", None); s.wait(rid, 5); s.notify("exit", None); s.p.wait(timeout=10)

(rust if sys.argv[1] == "rust" else clangd)()
