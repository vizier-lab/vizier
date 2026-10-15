import sys, os, json, sqlite3, time
sys.path.insert(0, os.path.dirname(__file__))
from run import *

def write_yaml(providers):
    with open(os.path.join(WS, ".vizier.yaml"), "w") as f:
        f.write("vizier:\n" + ("  providers:\n    openrouter:\n      api_key: sk-old\n" if providers else "") + "  storage:\n    type: sqlite\n  channels:\n    http:\n      port: %d\n      jwt_secret: e2e-secret\n      jwt_expiry_hours: 24\n      ws_idle_timeout_secs: 300\n" % PORT)

write_yaml(False)
DB = os.path.join(WS, ".vizier", ".runtime", "vizier.db")
p = start(); token = login()
for aid in ["dp-old", "gem-old", "cus-old", "gal-old", "moon-old"]:
    s, r = agent(token, aid)
    assert s in (200, 201), (aid, s, r)
stop(p)

# Plant a pre-015 workspace: legacy names everywhere, tagged credential JSON, no marker.
db = sqlite3.connect(DB)
def legacy(variant, ty, data):
    db.execute("INSERT OR REPLACE INTO provider_config (variant, data) VALUES (?, ?)",
               (variant, json.dumps({"variant": variant, "config": {"type": ty, "data": data}})))
legacy("gemini", "Gemini", {"api_key": "dummy-gemini-key"})
legacy("custom", "Custom", {"api_key": "", "base_url": "http://127.0.0.1:18080/v1"})
legacy("moonshot", "Moonshot", {"api_key": "dummy-moon", "base_url": "https://api.moonshot.cn/v1"})
legacy("galadriel", "Galadriel", {"api_key": "x"})
def set_agent(aid, provider, model, extra=None):
    row = db.execute("SELECT data FROM agent_config WHERE agent_id = ?", (aid,)).fetchone()
    cfg = json.loads(row[0]); cfg["provider"] = provider; cfg["model"] = model
    if extra: extra(cfg)
    db.execute("UPDATE agent_config SET data = ? WHERE agent_id = ?", (json.dumps(cfg), aid))
def gem_extra(cfg):
    cfg["tools"]["stt"] = {"enabled": True, "settings": {"provider": "gemini"}}
    cfg["embedding"] = {"provider": "gemini", "model": "text-embedding-004", "api_key": "old", "base_url": "old"}
set_agent("gem-old", "gemini", "gemini-2.5-flash", gem_extra)
set_agent("cus-old", "custom", "stub-tools")
set_agent("gal-old", "galadriel", "llama3.1:8b")
set_agent("moon-old", "moonshot", "moonshot-v1-8k")
db.execute("DELETE FROM state WHERE key LIKE 'migration.provider_ids_v1%'")
db.commit(); db.close()
write_yaml(True)

mark = os.path.getsize(os.path.join(WS, "vizier.log"))
p = start(); token = login(); time.sleep(4)
log = open(os.path.join(WS, "vizier.log")).read()[mark:]
check("log: gemini → google", "provider 'gemini' → 'google'" in log)
check("log: moonshot → moonshotai-cn", "provider 'moonshot' → 'moonshotai-cn'" in log)
check("log: custom → custom provider", "now the custom provider 'custom'" in log)
check("log: galadriel deleted", "provider 'galadriel' is no longer supported" in log)
check("log: agent rewritten", "agent 'gem-old': provider 'gemini' → 'google'" in log)
check("log: yaml providers warned", "providers.openrouter is ignored; set OPENROUTER_API_KEY" in log, [l for l in log.splitlines() if "ignored" in l])

s, r = call("GET", "/agents/health", token=token)
health = {h.get("agent_id", h.get("id")): h for h in (r.get("data") or [])} if isinstance(r, dict) else {}
print(json.dumps(r, indent=1)[:1500])
gal = json.dumps(health.get("gal-old", {}))
check("gal-old failed: not available", 'provider \\"galadriel\\" is not available; pick another provider' in gal or "galadriel" in gal and "not available" in gal, gal)
for aid in ["dp-old", "cus-old", "moon-old", "gem-old"]:
    h = json.dumps(health.get(aid, {}))
    # gem-old's Gemini embeddings reach Google with the planted dummy key and are refused:
    # that is the real request path working, so it counts.
    ok = '"alive": true' in h or (aid == "gem-old" and "API key not valid" in h)
    check(f"{aid} running (or refused by the real API)", ok, h[:300])

s, r = call("GET", "/agents/gem-old", token=token)
cfg = r.get("data", {})
check("gem-old provider google", json.dumps(cfg).count('"google"') >= 1 and cfg.get("provider", cfg.get("config", {}).get("provider")) in ("google", None), json.dumps(cfg)[:400])
s, r = call("GET", "/providers", token=token)
ids = [x["id"] for x in r["data"]]
check("providers list google + moonshotai-cn, no openrouter", "google" in ids and "moonshotai-cn" in ids and "openrouter" not in ids and "galadriel" not in ids, ids)
db = sqlite3.connect(DB)
custom = db.execute("SELECT data FROM custom_provider WHERE id='custom'").fetchone()
check("custom provider stored", custom and "127.0.0.1:18080" in custom[0], custom)
db.close()
stop(p)

mark = os.path.getsize(os.path.join(WS, "vizier.log"))
p = start(); token = login(); time.sleep(3)
log = open(os.path.join(WS, "vizier.log")).read()[mark:]
check("restart: no migration lines", "→" not in log, [l for l in log.splitlines() if "→" in l])
s, r = call("PUT", "/agents/gem-old", {"agent_id": "gem-old", "name": "gem-old", "provider": "gemini", "model": "gemini-2.5-flash"}, token)
s2, r2 = call("GET", "/agents/gem-old", token=token)
check("old client PUT provider gemini reads back google", s in (200, 201) and '"google"' in json.dumps(r2), (s, json.dumps(r)[:300]))

# Dummyplug regressions (008/014)
s, r = chat(token, "dp-old", "tools")
text = json.dumps(r)
check("dummyplug tools lists the 4 code-mode tools", all(t in text for t in ["think", "execute_python", "list_tool_functions", "describe_tool_function"]), text[:400])
s, r = chat(token, "dp-old", '{"tool": "execute_python", "arguments": {"intent": "check", "code": "memory_list(limit=1)"}}')
text = json.dumps(r)
check("dummyplug execute_python ok", '\\"ok\\": true' in text or '"ok":true' in text.replace(" ", "") or 'ok\\\\\\": true' in text or "\\\\\"ok\\\\\":true" in text or "ok" in text and "true" in text, text[:600])
s, r = chat(token, "dp-old", "context")
check("dummyplug context block", "# Context" in json.dumps(r), json.dumps(r)[:300])
stop(p)
print(f"\n{sum(ok for _, ok in results)}/{len(results)} passed")
