# Shared helpers for the 015 quickstart checks: start the debug binary on a scratch workspace,
# set up the first user, and call the HTTP API. Usage: python3 -I <script>.py <workspace> <port>
# (the stub must be running: python3 -I specs/015-provider-catalog/fixtures/stub_llm.py --port 18080).
import json, os, sqlite3, subprocess, sys, time, urllib.request, signal

ROOT = "/home/blinfoldking/Projects/vizier"
WS = sys.argv[1]
PORT = int(sys.argv[2])
BASE = f"http://127.0.0.1:{PORT}/api/v1"
LOG = open(os.path.join(WS, "vizier.log"), "a")

def start():
    env = dict(os.environ, VIZIER_DATA_DIR=WS, VIZIER_JWT_SECRET="e2e-secret", VIZIER_CATALOGUE_REFRESH="false", RUST_LOG="vizier=info")
    for k in ["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "GEMINI_API_KEY", "DEEPSEEK_API_KEY"]:
        env.pop(k, None)
    p = subprocess.Popen([f"{ROOT}/target/debug/vizier", "run", "--port", str(PORT)], cwd=WS, env=env, stdout=LOG, stderr=LOG)
    for _ in range(120):
        try:
            urllib.request.urlopen(f"{BASE}/ping", timeout=1); return p
        except Exception:
            time.sleep(0.5)
    raise SystemExit("server did not start")

def stop(p):
    p.send_signal(signal.SIGINT)
    try: p.wait(15)
    except subprocess.TimeoutExpired: p.kill(); p.wait()

def call(method, path, body=None, token=None, ok=True):
    req = urllib.request.Request(BASE + path, method=method, data=json.dumps(body).encode() if body is not None else None)
    req.add_header("content-type", "application/json")
    if token: req.add_header("authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            raw = r.read(); status = r.status
    except urllib.error.HTTPError as e:
        raw = e.read(); status = e.code
    try:
        return status, json.loads(raw or b"null")
    except ValueError:
        return status, {"raw": raw.decode(errors="replace")}

def login():
    call("POST", "/auth/setup", {"username": "admin", "password": "secret-pass-1"})
    _, r = call("POST", "/auth/login", {"username": "admin", "password": "secret-pass-1"})
    return r["data"]["token"]

def agent(token, aid, provider="dummyplug", model="dummyplug"):
    return call("POST", "/agents", {"agent_id": aid, "name": aid, "provider": provider, "model": model}, token)

def chat(token, aid, text):
    s, r = call("POST", f"/agents/{aid}/chat", {"channel_id": "e2e", "topic_id": "e2e", "content": text}, token)
    return s, (r or {}).get("data", r)

results = []
def check(name, cond, detail=""):
    results.append((name, bool(cond)))
    print(("PASS" if cond else "FAIL"), name, "" if cond else detail)
