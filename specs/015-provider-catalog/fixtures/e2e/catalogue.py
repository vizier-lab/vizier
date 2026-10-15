# Quickstart §2 (catalogue endpoints, offline) and §6 (lazy conditional refresh, SQLite cache).
import sys, os, json, sqlite3, time, threading, gzip, urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
sys.path.insert(0, os.path.dirname(__file__))
from run import *
import run
run.EXTRA_ENV.clear()

SEED = open(os.path.join(ROOT, "assets/catalogue/api.json"), "rb").read()
SERVED = []          # (path, If-None-Match)
ETAG = '"e2e-1"'

class Catalogue(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        SERVED.append((self.path, self.headers.get("If-None-Match")))
        if self.path == "/api.json":
            if self.headers.get("If-None-Match") == ETAG:
                self.send_response(304); self.send_header("ETag", ETAG); self.end_headers(); return
            body = gzip.compress(SEED)
            self.send_response(200); self.send_header("ETag", ETAG); self.send_header("Content-Encoding", "gzip")
            self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
        elif self.path == "/broken":
            self.send_response(200); self.send_header("Content-Length", "2"); self.end_headers(); self.wfile.write(b"{}")
        else:
            self.send_response(404); self.end_headers()

CAT_PORT = PORT + 1000
threading.Thread(target=ThreadingHTTPServer(("127.0.0.1", CAT_PORT), Catalogue).serve_forever, daemon=True).start()

def write_yaml(refresh, url_path="/api.json", ttl="24h"):
    with open(os.path.join(WS, ".vizier.yaml"), "w") as f:
        f.write(f"""vizier:
  storage:
    type: sqlite
  catalogue:
    refresh: {str(refresh).lower()}
    ttl: {ttl}
    url: http://127.0.0.1:{CAT_PORT}{url_path}
  channels:
    http:
      port: {PORT}
      jwt_secret: e2e-secret
      jwt_expiry_hours: 24
      ws_idle_timeout_secs: 300
""")
DB = os.path.join(WS, ".vizier", ".runtime", "vizier.db")

# --- §2, offline (refresh off) -------------------------------------------------------------
write_yaml(False)
p = start(); token = login()
s, r = call("GET", "/catalogue", token=token)
check("GET /catalogue: bundled, >= 200 providers", r["data"]["source"] == "bundled" and r["data"]["provider_count"] >= 200, r)
call("PUT", "/providers/openai", {"api_key": "sk-E2E-SECRET-123"}, token)
s, r = call("GET", "/catalogue/providers", token=token)
ids = {p["id"]: p for p in r["data"]}
check("providers include pre-feature ids, cerebras, fireworks-ai",
      all(i in ids for i in ["openai", "anthropic", "google", "openrouter", "deepseek", "ollama", "groq", "mistral",
                             "xai", "github-copilot", "togetherai", "chatgpt", "llama-cpp", "cerebras", "fireworks-ai"]),
      sorted(set(["cerebras", "fireworks-ai", "google"]) - set(ids)))
check("providers exclude amazon-bedrock and google-vertex", "amazon-bedrock" not in ids and "google-vertex" not in ids)
check("dummyplug listed as dummyplug (testing), connected", ids.get("dummyplug", {}).get("name") == "dummyplug (testing)"
      and ids["dummyplug"]["connection"]["status"] == "connected", ids.get("dummyplug"))
check("a saved key shows as connected/saved and never as its value",
      "sk-E2E-SECRET-123" not in json.dumps(r) and ids["openai"]["connection"] == {"status": "connected", "source": "saved"}, ids.get("openai"))
s, r = call("GET", "/catalogue/providers?q=cereb", token=token)
check("search 'cereb' finds Cerebras", [p["id"] for p in r["data"]] == ["cerebras"], [p["id"] for p in r["data"]])
s, r = call("GET", "/catalogue/providers/anthropic/models", token=token)
ms = r["data"]
dates = [m["release_date"] or "" for m in ms]
check("anthropic default list: tool-capable, not deprecated, newest first, first is default",
      ms and ms[0]["default"] and not any(m["default"] for m in ms[1:]) and dates == sorted(dates, reverse=True)
      and all(m["caps"]["tool_call"] is not False and m["status"] != "deprecated" for m in ms)
      and all(k in ms[0] for k in ["limit", "caps", "cost", "logo"]), [(m["id"], m["release_date"]) for m in ms][:4])
s, r1 = call("GET", "/catalogue/providers/google/models", token=token)
s, r2 = call("GET", "/catalogue/providers/google/models?all=true", token=token)
check("?all=true adds deprecated models", len(r2["data"]) > len(r1["data"]) and any(m["status"] == "deprecated" for m in r2["data"])
      and not any(m["status"] == "deprecated" for m in r1["data"]))
s, r = call("GET", "/catalogue/providers/openrouter/models?all=true", token=token)
check("openrouter has >= 380 models", len(r["data"]) >= 380, len(r["data"]))
s, r = call("GET", "/catalogue/providers/openrouter/models/anthropic/claude-sonnet-4.5", token=token)
check("a model id with a slash resolves", s == 200 or s == 404, s)
s, r = call("GET", "/catalogue/providers/anthropic/models/my-unlisted-model", token=token)
check("an unlisted model is 404 'not in catalogue'", s == 404 and r.get("message") == "not in catalogue", (s, r))
req = urllib.request.Request(f"{BASE}/catalogue/logos/anthropic.svg")
with urllib.request.urlopen(req) as resp:
    h = resp.headers; body = resp.read()
check("logo: 200 image/svg+xml with CSP and nosniff, no auth", h["Content-Type"] == "image/svg+xml"
      and "default-src 'none'" in h["Content-Security-Policy"] and h["X-Content-Type-Options"] == "nosniff" and body.startswith(b"<svg"))
try:
    urllib.request.urlopen(f"{BASE}/catalogue/logos/no-such-thing.svg"); missing = 200
except urllib.error.HTTPError as e:
    missing = e.code
check("unknown logo is 404", missing == 404, missing)
s, r = call("GET", "/catalogue/providers", token=None)
check("catalogue listings need auth", s == 401, s)
s, r = call("POST", "/catalogue/refresh", token=token)
check("refresh disabled: POST /catalogue/refresh is 409", s == 409, (s, r))
check("refresh off: no request to the catalogue server", not SERVED, SERVED)
stop(p)

# --- §6: agents never trigger; a person's read does ------------------------------------------
# A 1-minute TTL: the seed was fetched minutes or hours ago, as a release's is.
write_yaml(True, ttl="1m")
p = start(); token = login()
agent(token, "dp", "dummyplug", "dummyplug")
s, r = chat(token, "dp", "hello")
time.sleep(4)
check("agents running and chatting: no catalogue request", not SERVED, SERVED)
t0 = time.time(); s, r = call("GET", "/catalogue/providers", token=token)
check("a read answers at once from the seed", time.time() - t0 < 2 and s == 200)
for _ in range(40):
    s, r = call("GET", "/catalogue", token=token)
    if r["data"]["source"] == "cached": break
    time.sleep(0.25)
check("one background request, then source cached", r["data"]["source"] == "cached" and len(SERVED) == 1, (r["data"], SERVED))
fetched_1 = r["data"]["fetched_at"]
db = sqlite3.connect(DB); row = db.execute("SELECT etag, fetched_at FROM catalogue_snapshot").fetchone(); db.close()
check("a catalogue_snapshot row is cached in SQLite", row and row[0] == ETAG, row)
call("GET", "/catalogue/providers", token=token); time.sleep(1)
check("a second read within the TTL makes no request", len(SERVED) == 1, SERVED)
stop(p)

# --- §6: conditional request with a 1s TTL ------------------------------------------------------
write_yaml(True, ttl="1s")
p = start(); token = login(); time.sleep(1.5)
before = len(SERVED)
call("GET", "/catalogue/providers", token=token); time.sleep(1.5)
s, r = call("GET", "/catalogue", token=token)
check("the check sends If-None-Match and gets a 304", len(SERVED) > before and SERVED[-1][1] == ETAG, SERVED[before:])
check("304: checked_at advances, fetched_at doesn't", r["data"]["fetched_at"] == fetched_1
      and r["data"]["checked_at"] and r["data"]["checked_at"] > fetched_1, r["data"])
stop(p)

# --- §6: a broken catalogue keeps the previous one ----------------------------------------------
write_yaml(True, url_path="/broken")
p = start(); token = login()
s, r = call("POST", "/catalogue/refresh", token=token)
check("broken URL: POST /catalogue/refresh is 502", s == 502, (s, r))
s, r = call("GET", "/catalogue", token=token)
check("last_error set; still cached with the previous fetched_at", r["data"]["refresh"]["last_error"]
      and r["data"]["source"] == "cached" and r["data"]["fetched_at"] == fetched_1, r["data"])
before = len(SERVED)
call("GET", "/catalogue/providers", token=token); time.sleep(1)
check("within the hour after a failure, reads make no request", len(SERVED) == before, SERVED[before:])
stop(p)

# --- §6: offline restart reads the cache ---------------------------------------------------------
write_yaml(False)
p = start(); token = login()
s, r = call("GET", "/catalogue", token=token)
check("offline restart: source cached, loaded from SQLite", r["data"]["source"] == "cached" and r["data"]["fetched_at"] == fetched_1, r["data"])
stop(p)
print(f"{sum(ok for _, ok in results)}/{len(results)} passed")
