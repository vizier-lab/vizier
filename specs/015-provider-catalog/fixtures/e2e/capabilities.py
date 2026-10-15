# Quickstart §2 (context windows, save warnings) and §3 (images by capability, FR-013/FR-033).
import sys, os, json, sqlite3, time, base64
sys.path.insert(0, os.path.dirname(__file__))
from run import *

with open(os.path.join(WS, ".vizier.yaml"), "w") as f:
    f.write("vizier:\n  storage:\n    type: sqlite\n  channels:\n    http:\n      port: %d\n      jwt_secret: e2e-secret\n      jwt_expiry_hours: 24\n      ws_idle_timeout_secs: 300\n" % PORT)
DB = os.path.join(WS, ".vizier", ".runtime", "vizier.db")
p = start(); token = login(); stop(p)
db = sqlite3.connect(DB)
db.execute("INSERT INTO custom_provider (id, data) VALUES ('stub', ?)", (json.dumps({
    "id": "stub", "name": "Stub", "protocol": "openai_compatible", "base_url": "http://127.0.0.1:18081/v1",
    "api_key": "k", "headers": {}, "models": [
        {"id": "stub-tools", "tool_call": True, "image_input": False, "edited": True},
        {"id": "stub-vision", "tool_call": True, "image_input": True, "edited": True}]}),))
db.commit(); db.close()

p = start(); token = login()
for pid in ["anthropic", "openai"]:
    call("PUT", f"/providers/{pid}", {"api_key": "dummy"}, token)

def health(aid):
    for _ in range(40):
        s, r = call("GET", "/agents/health", token=token)
        for h in r["data"]:
            if h["agent_id"] == aid and h["state"] in ("running", "failed"):
                return h
        time.sleep(0.5)
    return {}

# §2 context windows
s, r = agent(token, "ant-1", "anthropic", "claude-sonnet-4-5")
h = health("ant-1")
check("ant-1 context window from the catalogue", (h.get("model") or {}).get("context_window") == 200000
      and h["model"]["context_source"] == "catalogue", h)
s, r = call("PUT", "/agents/ant-1", {"agent_id": "ant-1", "name": "ant-1", "provider": "anthropic",
                                     "model": "claude-sonnet-4-5", "context_window": 50000}, token)
time.sleep(2); h = health("ant-1")
check("ant-1 override shows 50000", (h.get("model") or {}).get("context_window") == 50000
      and h["model"]["context_source"] == "override", h)

# §2 warnings
s, r = agent(token, "notools", "openai", "gpt-3.5-turbo")
check("tool_call:false model warns no_tool_calling", s == 201 and [w["code"] for w in r["data"]["warnings"]] == ["no_tool_calling"], r)
s, r = agent(token, "unlisted", "anthropic", "my-unlisted-model")
check("unlisted model warns unknown_model", s == 201 and [w["code"] for w in r["data"]["warnings"]] == ["unknown_model"], r)

# §3 images by capability
def read_image_agent(aid, model):
    return call("POST", "/agents", {"agent_id": aid, "name": aid, "provider": "stub", "model": model,
                                    "tools": {"read_image": True}}, token)
png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
def send_image(aid):
    s, r = call("POST", f"/agents/{aid}/chat", {"channel_id": "e2e", "topic_id": "img", "content":
        'CALL execute_python {"intent": "look at the image", "code": "read_image_file(filename=\\"dot.png\\")"}',
        "attachments": [{"filename": "dot.png", "content": {"base64": png}}]}, token)
    return json.dumps(r)

for aid, model in [("img-yes", "stub-vision"), ("img-no", "stub-tools")]:
    s, r = read_image_agent(aid, model)
    print(aid, s, json.dumps(r)[:200])
time.sleep(3)
out = send_image("img-yes")
check("img-yes: the image goes straight to the model", "images=1" in out and "notes=0" in out, out[:500])
out = send_image("img-no")
check("img-no: the turn succeeds with a note instead of the image", "images=0" in out and "notes=1" in out, out[:500])
h = health("img-no")
check("img-no health warns about read_image", any("cannot view images" in w for w in h.get("warnings", [])), h)
s, r = call("PUT", "/agents/img-no", {"agent_id": "img-no", "name": "img-no", "provider": "stub", "model": "stub-tools",
    "tools": {"read_image": True, "read_image_settings": {"provider": "stub", "model": "stub-vision"}}}, token)
time.sleep(2)
out = send_image("img-no")
check("img-no with a vision model gets the vision model's description", "ECHO tool-result" in out and "image" in out, out[:500])
stop(p)
print(f"{sum(ok for _, ok in results)}/{len(results)} passed")
