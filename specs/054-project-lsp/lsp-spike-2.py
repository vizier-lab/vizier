#!/usr/bin/env python3
"""Second measurement for spec 054: process lifetime, a crate with real dependencies, Vizier itself."""
import json, os, pathlib, signal, subprocess, sys, tempfile, time

src = pathlib.Path(sys.argv[1]).read_text().split("(rust if sys.argv")[0]
exec(src)

def alive(pid):
    try:
        os.kill(pid, 0); return True
    except OSError:
        return False

def rss_mb(pid):
    out = subprocess.run(["ps", "-o", "rss=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()
    return round(int(out) / 1024) if out else None

def init(s, root, options=None):
    rid = s.request("initialize", {"processId": os.getpid(), "rootUri": uri(root), "capabilities": CAPS,
                                   "initializationOptions": options or {},
                                   "workspaceFolders": [{"uri": uri(root), "name": "w"}]})
    t, m = s.wait(rid, 30); s.notify("initialized", {}); return m["result"]["capabilities"]

def tiny():
    root = tempfile.mkdtemp(prefix="vz-life-"); os.makedirs(f"{root}/src")
    pathlib.Path(f"{root}/Cargo.toml").write_text('[package]\nname="t"\nversion="0.1.0"\nedition="2021"\n')
    pathlib.Path(f"{root}/src/lib.rs").write_text("pub fn f() {}\n")
    return root

def lifecycle():
    root = tiny()
    print("== stdin close after initialize ==")
    s = Lsp(["rust-analyzer"], root); init(s, root); s.pump(2)
    s.p.stdin.close(); t = time.time()
    try:
        s.p.wait(timeout=15); print(f"  exited after {round(time.time()-t,2)}s, code {s.p.returncode}")
    except subprocess.TimeoutExpired:
        print("  STILL RUNNING 15s after EOF"); s.p.kill()

    print("== parent killed: processId given, stdin held open by a survivor ==")
    # An intermediate parent starts the server and reports its pid; a third process keeps the pipe open
    # so only the processId route can end the server, then the same with the pipe closing too.
    for keep_pipe in (True, False):
        code = f"""
import json,os,subprocess,sys,time
r,w=os.pipe()
p=subprocess.Popen(['rust-analyzer'],cwd={root!r},stdin=r,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,start_new_session=True)
os.close(r)
body=json.dumps({{'jsonrpc':'2.0','id':1,'method':'initialize','params':{{'processId':os.getpid(),'rootUri':'file://{root}','capabilities':{{}}}}}}).encode()
os.write(w,b'Content-Length: %d\\r\\n\\r\\n'%len(body)+body)
b2=json.dumps({{'jsonrpc':'2.0','method':'initialized','params':{{}}}}).encode()
os.write(w,b'Content-Length: %d\\r\\n\\r\\n'%len(b2)+b2)
print(p.pid,flush=True)
{"subprocess.Popen(['sleep','40'],stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,pass_fds=(w,),start_new_session=True)" if keep_pipe else ""}
time.sleep(60)
"""
        parent = subprocess.Popen([sys.executable, "-I", "-c", code], stdout=subprocess.PIPE, text=True)
        pid = int(parent.stdout.readline()); time.sleep(3)
        print(f"  keep_pipe={keep_pipe}: server alive before kill: {alive(pid)}")
        parent.kill(); parent.wait(); t = time.time(); gone = None
        while time.time() - t < 15:
            if not alive(pid): gone = round(time.time() - t, 2); break
            time.sleep(0.2)
        print(f"  keep_pipe={keep_pipe}: server gone after {gone}s" if gone is not None else f"  keep_pipe={keep_pipe}: STILL RUNNING after 15s")
        if alive(pid): os.kill(pid, signal.SIGKILL)

def medium():
    root = tempfile.mkdtemp(prefix="vz-med-"); os.makedirs(f"{root}/src")
    pathlib.Path(f"{root}/Cargo.toml").write_text('[package]\nname="med"\nversion="0.1.0"\nedition="2021"\n[dependencies]\nserde={version="1",features=["derive"]}\nserde_json="1"\ntokio={version="1",features=["full"]}\nregex="1"\n')
    lib = f"{root}/src/lib.rs"; other = f"{root}/src/other.rs"
    pathlib.Path(lib).write_text('pub mod other;\n#[derive(serde::Serialize)]\npub struct S { pub a: u32 }\npub fn f(a: u32) -> u32 { a + 1 }\n')
    pathlib.Path(other).write_text('pub fn g() -> u32 { crate::f(1) }\n')
    print("== crate with serde, tokio, regex (cold target) ==")
    s = Lsp(["rust-analyzer"], root); caps = init(s, root)
    s.notify("textDocument/didOpen", {"textDocument": {"uri": uri(lib), "languageId": "rust", "version": 1, "text": pathlib.Path(lib).read_text()}})
    s.notify("textDocument/didOpen", {"textDocument": {"uri": uri(other), "languageId": "rust", "version": 1, "text": pathlib.Path(other).read_text()}})
    # What does a question return while indexing?
    s.pump(0.5)
    rid = s.request("textDocument/definition", {"textDocument": {"uri": uri(other)}, "position": {"line": 0, "character": 27}})
    r = s.wait(rid, 10)
    print(f"  question at {s.now()}s, open progress {list(s.progress.values())}:", json.dumps(r[1].get("result", r[1].get("error")))[:110] if r else "no answer in 10s")
    t = time.time(); indexed = None
    while time.time() - t < 150:
        s.pump(0.5)
        titles = list(s.progress.values())
        if indexed is None and s.ended and not [x for x in titles if x != "cargo check"] and s.now() - s.ended[-1] > 1:
            indexed = s.now(); print(f"  quiet except cargo check at {indexed}s; open: {titles}; rss {rss_mb(s.p.pid)} MB")
        if not titles and s.ended and s.now() - s.ended[-1] > 4: break
    print(f"  all quiet at {s.now()}s; rss {rss_mb(s.p.pid)} MB")

    print("-- edit during nothing running --")
    broken = 'pub mod other;\n#[derive(serde::Serialize)]\npub struct S { pub a: u32 }\npub fn f(a: u32, b: u32) -> u32 { a + b }\npub fn h() -> u32 { "s" }\n'
    pathlib.Path(lib).write_text(broken); at = s.now(); s.push.clear()
    s.notify("textDocument/didChange", {"textDocument": {"uri": uri(lib), "version": 2}, "contentChanges": [{"text": broken}]})
    for name, f in (("edited", lib), ("other", other)):
        rid = s.request("textDocument/diagnostic", {"textDocument": {"uri": uri(f)}})
        r = s.wait(rid, 10)
        if r: print(f"  PULL {name} +{round(r[0]-at,2)}s:", [x["message"][:45] for x in r[1].get("result", {}).get("items", [])])
    save = s.now(); s.push.clear()
    s.notify("textDocument/didSave", {"textDocument": {"uri": uri(lib)}})
    s.pump(0.3)
    print("-- a cargo build started while cargo check runs --")
    t = time.time(); b = subprocess.run(["cargo", "build", "--offline"], cwd=root, capture_output=True, text=True)
    print(f"  cargo build took {round(time.time()-t,2)}s; blocked on lock: {'Blocking waiting' in b.stderr}")
    s.pump(5)
    print(f"  pushes after save: {[(round(a-save,2),n,len(d)) for a,n,c,d in s.push][:6]}; progress now {list(s.progress.values())}")
    rid = s.request("shutdown", None); s.wait(rid, 5); s.notify("exit", None); s.p.wait(timeout=10)

def vizier():
    root = sys.argv[3]
    print("== Vizier workspace, read-only: build scripts, proc macros and check off ==")
    opts = {"cargo": {"buildScripts": {"enable": False}}, "procMacro": {"enable": False}, "checkOnSave": False,
            "check": {"workspace": False}, "cachePriming": {"enable": True}}
    s = Lsp(["rust-analyzer"], root); caps = init(s, root, opts)
    f = f"{root}/crates/vizier-core/src/projects/ledger.rs"; text = pathlib.Path(f).read_text()
    s.notify("textDocument/didOpen", {"textDocument": {"uri": uri(f), "languageId": "rust", "version": 1, "text": text}})
    t = time.time(); peak = 0
    while time.time() - t < 280:
        s.pump(1); peak = max(peak, rss_mb(s.p.pid) or 0)
        if s.ended and not s.progress and s.now() - s.ended[-1] > 5: break
    print(f"  quiet at {s.now()}s; open {list(s.progress.values())}; rss {rss_mb(s.p.pid)} MB, peak {peak} MB")
    for label, change in (("first pull after edit", text.replace("pub fn record(&self, session: &str", "pub fn record(&self, session: u32")),):
        at = s.now(); s.notify("textDocument/didChange", {"textDocument": {"uri": uri(f), "version": 2}, "contentChanges": [{"text": change}]})
        rid = s.request("textDocument/diagnostic", {"textDocument": {"uri": uri(f)}}); r = s.wait(rid, 60)
        print(f"  {label}: +{round(r[0]-at,2)}s, items {len(r[1].get('result',{}).get('items',[]))}" if r else "  no answer in 60s")
        at = s.now(); rid = s.request("textDocument/references", {"textDocument": {"uri": uri(f)}, "position": {"line": 52, "character": 12}, "context": {"includeDeclaration": False}}); r = s.wait(rid, 60)
        print(f"  references: +{round(r[0]-at,2)}s, {len(r[1].get('result') or [])} results" if r else "  references: no answer in 60s")
    rid = s.request("shutdown", None); s.wait(rid, 10); s.notify("exit", None); s.p.wait(timeout=10)

def lock():
    root = tempfile.mkdtemp(prefix="vz-lock-"); os.makedirs(f"{root}/src")
    pathlib.Path(f"{root}/Cargo.toml").write_text('[package]\nname="lk"\nversion="0.1.0"\nedition="2021"\n[dependencies]\nserde={version="1",features=["derive"]}\nserde_json="1"\ntokio={version="1",features=["full"]}\nregex="1"\n')
    pathlib.Path(f"{root}/src/lib.rs").write_text('pub fn f() {}\n')
    print("== cargo build overlapping the first cargo check ==")
    s = Lsp(["rust-analyzer"], root); init(s, root)
    t = time.time()
    while time.time() - t < 120 and "cargo check" not in s.progress.values(): s.pump(0.05)
    print(f"  cargo check began at {s.now()}s")
    # Touch the build while the check is running, reading the server as we go.
    proc = subprocess.Popen(["cargo", "build", "--offline"], cwd=root, stderr=subprocess.PIPE, text=True)
    start = time.time()
    while proc.poll() is None: s.pump(0.2)
    err = proc.stderr.read()
    print(f"  cargo build took {round(time.time()-start,2)}s, exit {proc.returncode}; 'Blocking waiting': {'Blocking waiting' in err}; check open: {list(s.progress.values())}")
    rid = s.request("shutdown", None); s.wait(rid, 5); s.notify("exit", None); s.p.wait(timeout=10)

{"lock": lock, "lifecycle": lifecycle, "medium": medium, "vizier": vizier}[sys.argv[2]]()
