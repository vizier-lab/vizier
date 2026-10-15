import sys, os, json, sqlite3, time
sys.path.insert(0, os.path.dirname(__file__))
from run import *
with open(os.path.join(WS, ".vizier.yaml"), "w") as f:
    f.write("vizier:\n  storage:\n    type: sqlite\n  channels:\n    http:\n      port: %d\n      jwt_secret: e2e-secret\n      jwt_expiry_hours: 24\n      ws_idle_timeout_secs: 300\n" % PORT)
DB = os.path.join(WS, ".vizier", ".runtime", "vizier.db")
p = start(); token = login(); stop(p)
db = sqlite3.connect(DB)
for pid, headers in [("stub", {}), ("flaky", {"X-Stub-Status": "429", "X-Stub-Retry-After": "2"})]:
    db.execute("INSERT INTO custom_provider (id, data) VALUES (?, ?)", (pid, json.dumps({
        "id": pid, "name": pid, "protocol": "openai_compatible", "base_url": "http://127.0.0.1:18080/v1",
        "api_key": "k", "headers": headers, "models": []})))
db.commit(); db.close()
p = start(); token = login()
print(agent(token, "dp", "dummyplug", "dummyplug")[0], agent(token, "c1", "stub", "stub-tools")[0], agent(token, "r1", "flaky", "stub-tools")[0])
time.sleep(3)
s, r = chat(token, "dp", '{"tool": "execute_python", "arguments": {"intent": "check", "code": "memory_list(limit=1)"}}')
print("execute_python:", json.dumps(r)[:500])
s, r = chat(token, "c1", "hello")
check("c1 (custom provider over HTTP) replies ECHO", "ECHO" in json.dumps(r), json.dumps(r)[:300])
s, r = chat(token, "c1", "CALL list_tool_functions {}")
check("c1 tool call runs and result is echoed", "ECHO tool-result" in json.dumps(r), json.dumps(r)[:400])
mark = os.path.getsize(os.path.join(WS, "vizier.log"))
t0 = time.time(); s, r = chat(token, "r1", "hello"); dt = time.time() - t0
log = open(os.path.join(WS, "vizier.log")).read()[mark:]
import re
log = re.sub(r"\x1b\[[0-9;]*m", "", log); waits = [int(w) for w in re.findall(r"wait_ms=(\d+)", log)]
check("r1 retried once with wait_ms >= 2000 and answered", "ECHO" in json.dumps(r) and waits and min(waits) >= 2000 and dt >= 2, (waits, dt, json.dumps(r)[:200]))
stop(p)
print(f"{sum(ok for _, ok in results)}/{len(results)} passed")
