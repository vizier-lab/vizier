#!/usr/bin/env python3
"""Offline stdio MCP server for the 014 quickstart: standard library only, no network.

Usage: echo_mcp.py TOOL [TOOL ...]
Every TOOL takes an optional `text` argument and answers "<TOOL>: <text>", so a test can tell
which server and tool a call reached.
"""
import json
import sys

TOOLS = sys.argv[1:] or ["echo"]
PROTOCOL = "2025-06-18"


def reply(msg_id, result=None, error=None):
    out = {"jsonrpc": "2.0", "id": msg_id}
    if error is not None:
        out["error"] = error
    else:
        out["result"] = result
    sys.stdout.write(json.dumps(out) + "\n")
    sys.stdout.flush()


for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    msg = json.loads(line)
    method, msg_id = msg.get("method"), msg.get("id")
    if msg_id is None:  # notifications (e.g. notifications/initialized) get no reply
        continue
    if method == "initialize":
        reply(msg_id, {
            "protocolVersion": msg.get("params", {}).get("protocolVersion", PROTOCOL),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "echo-mcp", "version": "0.1.0"},
        })
    elif method == "tools/list":
        reply(msg_id, {"tools": [
            {
                "name": name,
                "description": f"Echo text back, tagged with '{name}'.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"text": {"type": "string", "description": "Text to echo"}},
                },
            }
            for name in TOOLS
        ]})
    elif method == "tools/call":
        params = msg.get("params", {})
        name = params.get("name")
        if name not in TOOLS:
            reply(msg_id, error={"code": -32602, "message": f"unknown tool {name}"})
            continue
        text = (params.get("arguments") or {}).get("text", "")
        reply(msg_id, {"content": [{"type": "text", "text": f"{name}: {text}"}], "isError": False})
    elif method == "ping":
        reply(msg_id, {})
    else:
        reply(msg_id, error={"code": -32601, "message": f"method not found: {method}"})
