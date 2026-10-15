# Quickstart §4 (custom providers, FR-022–FR-027) and the providers API (US5, FR-010a–c).
import sys, os, json, time
sys.path.insert(0, os.path.dirname(__file__))
from run import *

STUB = "http://127.0.0.1:18080/v1"
p = start(); token = login()

def body(**kw):
    b = {"name": "Home", "protocol": "openai_compatible", "base_url": STUB, "api_key": "sk-home-secret",
         "headers": {"x-team": "t-secret"}}
    b.update(kw); return b

s, r = call("POST", "/providers/custom/test", body(), token)
check("test against the stub lists 2 models", s == 200 and r["data"] == {"ok": True, "model_count": 2}, r)
s, r = call("POST", "/providers/custom/test", body(base_url="http://127.0.0.1:18099/v1"), token)
check("test against a closed port fails, refused", s == 200 and not r["data"]["ok"] and "refused" in r["data"]["error"].lower(), r)
s, r = call("POST", "/providers/custom", body(id="openai"), token)
check("id 'openai' clashes with the catalogue (409)", s == 409 and "catalogue" in r["message"], r)
s, r = call("POST", "/providers/custom", body(id="home"), token)
check("create home", s == 200 and r["data"]["id"] == "home", r)
s, r = call("POST", "/providers/custom", body(id="home"), token)
check("a second home clashes with the custom provider (409)", s == 409 and "custom" in r["message"], r)
s, r = call("POST", "/providers/custom/home/fetch-models", None, token)
models = {m["id"]: m for m in r["data"]["models"]}
check("fetch-models adds both stub models", s == 200 and set(models) == {"stub-tools", "stub-vision"}, r)
s, r = call("GET", "/catalogue/providers/home/models", token=token)
st = [m for m in r["data"] if m["id"] == "stub-tools"][0]
check("stub-tools has no catalogue match: empty limits", st["limit"]["context"] is None and st["filled_from_catalogue"] == [], st)

rows = [dict(models["stub-tools"], context=32768), models["stub-vision"], {"id": "extra", "tool_call": True}]
s, r = call("PUT", "/providers/custom/home", {"name": "Home", "protocol": "openai_compatible", "base_url": STUB, "models": rows}, token)
check("edit: changed and new rows are edited", s == 200 and [m["edited"] for m in r["data"]["models"]] == [True, False, True], r)
s, r = call("POST", "/providers/custom/home/fetch-models", None, token)
models = {m["id"]: m for m in r["data"]["models"]}
check("a fetch keeps the edited context and the extra row", models["stub-tools"]["context"] == 32768 and "extra" in models, r)
s, r = call("PUT", "/providers/custom/home", body(id="home2"), token)
check("the id is immutable (400)", s == 400, r)

s, r = call("GET", "/providers/custom", token=token)
raw = json.dumps(r)
home = [c for c in r["data"] if c["id"] == "home"][0]
check("GET lists has_api_key and header names, never values",
      home["has_api_key"] and home["headers"] == ["x-team"] and "secret" not in raw, raw[:400])

# The custom provider through the agent loop, both protocols.
s, r = agent(token, "c1", "home", "stub-tools"); time.sleep(2)
s, out = chat(token, "c1", "CALL list_tool_functions {}")
check("c1: tool call through the custom provider", "ECHO tool-result" in json.dumps(out), out)
s, r = call("POST", "/providers/custom", {"id": "home-anth", "name": "Home Anthropic", "protocol": "anthropic",
                                          "base_url": "http://127.0.0.1:18080"}, token)
check("create home-anth (anthropic)", s == 200, r)
s, r = agent(token, "c2", "home-anth", "stub-tools"); time.sleep(2)
s, out = chat(token, "c2", "hello")
check("c2: reply through /v1/messages", "ECHO" in json.dumps(out), out)

# Providers API.
s, r = call("PUT", "/providers/anthropic", {"base_url": "http://x"}, token)
check("a base_url on a cloud provider is refused (400, FR-027)", s == 400 and "custom provider" in r["message"], r)
s, r = call("PUT", "/providers/openai", {"account_id": "x"}, token)
check("a field the provider doesn't use is refused (400)", s == 400, r)
s, r = call("PUT", "/providers/anthropic", {"api_key": "dummy"}, token)
check("save an anthropic key", s == 200 and r["data"]["source"] == "saved" and r["data"]["has_api_key"], r)
call("PUT", "/agents/c2", {"agent_id": "c2", "name": "c2", "provider": "anthropic", "model": "claude-sonnet-4-5",
                           "dream_provider": "anthropic"}, token)
s, r = call("GET", "/providers", token=token)
ant = [x for x in r["data"] if x["id"] == "anthropic"]
check("GET /providers shows anthropic used by 1 chat agent", ant and ant[0]["used_by"]["chat"] == 1 and "dummy" not in json.dumps(r), r)
check("GET /providers never lists dummyplug or custom providers", not [x for x in r["data"] if x["id"] in ("dummyplug", "home")], r)
s, r = call("POST", "/providers/home/test", None, token)
check("POST /providers/home/test works for a custom provider", s == 200 and r["data"]["ok"], r)
s, r = call("DELETE", "/providers/anthropic", None, token)
check("deleting a used provider is 409 listing c2", s == 409 and r["data"]["agents"][0]["id"] == "c2", r)

s, r = call("DELETE", "/providers/custom/home", None, token)
check("deleting home is 409 listing c1", s == 409 and [a["id"] for a in r["data"]["agents"]] == ["c1"], r)
s, r = call("DELETE", "/providers/custom/home?confirm=true", None, token)
check("confirmed delete", s == 200, r)
time.sleep(2)
s, r = call("GET", "/agents/health", token=token)
c1 = [h for h in r["data"] if h["agent_id"] == "c1"]
check("c1 is Failed with 'not available'",
      c1 and c1[0]["state"] == "failed" and "not available" in json.dumps(c1), c1)
stop(p)
print(f"{sum(ok for _, ok in results)}/{len(results)} passed")
