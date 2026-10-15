#!/usr/bin/env python3
"""A keyless LLM stub for the 015 quickstart (stdlib only; run with `python3 -I`).

    python3 -I specs/015-provider-catalog/fixtures/stub_llm.py --port 18080

Endpoints:
  GET  /v1/models            stub-tools, stub-vision (OpenAI list format)
  POST /v1/chat/completions  OpenAI chat
  POST /v1/messages          Anthropic Messages
  GET  /broken               {} (an invalid catalogue, for the refresh steps)

Both completion endpoints reply `ECHO <compact JSON of the last user message's parts>`.
When the last user text starts with `CALL <tool> <json>`, they answer with one call to
<tool> instead. After a tool result, they reply `ECHO tool-result <content>`.

`X-Stub-Status: <code>` (and optionally `X-Stub-Retry-After: <s>`) makes the first request
carrying it fail with that status; the next one succeeds. Every request body is printed to
stderr.
"""

import argparse
import json
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

MODELS = ["stub-tools", "stub-vision"]

_lock = threading.Lock()
# Error injection: one failure per distinct (status, path) before succeeding again.
_failed_once = set()


def compact(value):
    return json.dumps(value, separators=(",", ":"), sort_keys=True)


def openai_parts(content):
    """Normalise an OpenAI message `content` into a list of {type,...} parts."""
    if isinstance(content, str):
        return [{"type": "text", "text": content}]
    parts = []
    for part in content or []:
        kind = part.get("type")
        if kind == "text":
            parts.append({"type": "text", "text": part.get("text", "")})
        elif kind == "image_url":
            url = (part.get("image_url") or {}).get("url", "")
            parts.append({"type": "image", "url_prefix": url[:30]})
        elif kind == "file":
            parts.append({"type": "file"})
        else:
            parts.append({"type": kind})
    return parts


def anthropic_parts(content):
    if isinstance(content, str):
        return [{"type": "text", "text": content}]
    parts = []
    for block in content or []:
        kind = block.get("type")
        if kind == "text":
            parts.append({"type": "text", "text": block.get("text", "")})
        elif kind == "image":
            parts.append({"type": "image", "media_type": (block.get("source") or {}).get("media_type")})
        else:
            parts.append({"type": kind})
    return parts


def parse_call(parts):
    """A line `CALL <tool> <json>` in any text part → (tool, args) or None. Any line, because
    vizier puts its context block and the channel frontmatter ahead of what the person typed."""
    for part in parts:
        if part.get("type") != "text":
            continue
        for line in part.get("text", "").splitlines():
            line = line.strip()
            if line.startswith("CALL "):
                name, _, args = line[len("CALL "):].strip().partition(" ")
                try:
                    arguments = json.loads(args) if args.strip() else {}
                except ValueError:
                    arguments = {"raw": args}
                return name, arguments
    return None


class Handler(BaseHTTPRequestHandler):
    server_version = "stub-llm/1"

    def log_message(self, fmt, *args):  # quiet the default access log
        pass

    def _send(self, status, body, headers=None):
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        for k, v in (headers or {}).items():
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(data)

    def _inject_error(self):
        status = self.headers.get("X-Stub-Status")
        if not status:
            return False
        key = (status, self.path)
        with _lock:
            if key in _failed_once:
                _failed_once.discard(key)
                return False
            _failed_once.add(key)
        headers = {}
        retry_after = self.headers.get("X-Stub-Retry-After")
        if retry_after:
            headers["Retry-After"] = retry_after
        print(f"[stub] injecting {status} on {self.path}", file=sys.stderr, flush=True)
        self._send(int(status), {"error": {"message": f"stub injected {status}"}}, headers)
        return True

    def _body(self):
        length = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(length) if length else b""
        print(f"[stub] {self.command} {self.path} {raw.decode(errors='replace')}", file=sys.stderr, flush=True)
        try:
            return json.loads(raw) if raw else {}
        except ValueError:
            return {}

    def do_GET(self):
        print(f"[stub] GET {self.path}", file=sys.stderr, flush=True)
        if self._inject_error():
            return
        if self.path.rstrip("/") in ("/v1/models", "/models"):
            self._send(200, {
                "object": "list",
                "data": [{"id": m, "object": "model", "owned_by": "stub"} for m in MODELS],
            })
        elif self.path == "/broken":
            self._send(200, {})
        else:
            self._send(404, {"error": {"message": "not found"}})

    def do_POST(self):
        body = self._body()
        if self._inject_error():
            return
        if self.path.endswith("/chat/completions"):
            self._send(200, self._openai(body))
        elif self.path.endswith("/messages"):
            self._send(200, self._anthropic(body))
        else:
            self._send(404, {"error": {"message": "not found"}})

    def _openai(self, body):
        messages = body.get("messages") or []
        model = body.get("model", "stub")
        last = messages[-1] if messages else {}
        message = {"role": "assistant", "content": None}
        if last.get("role") == "tool":
            message["content"] = f"ECHO tool-result {last.get('content')}"
        else:
            user = next((m for m in reversed(messages) if m.get("role") == "user"), {})
            parts = openai_parts(user.get("content"))
            call = parse_call(parts)
            if call and body.get("tool_choice") != "none":
                name, arguments = call
                message["tool_calls"] = [{
                    "id": "call_stub_1",
                    "type": "function",
                    "function": {"name": name, "arguments": json.dumps(arguments)},
                }]
            else:
                message["content"] = f"ECHO {compact(parts)}"
        return {
            "id": "chatcmpl-stub",
            "object": "chat.completion",
            "model": model,
            "choices": [{
                "index": 0,
                "message": message,
                "finish_reason": "tool_calls" if message.get("tool_calls") else "stop",
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15},
        }

    def _anthropic(self, body):
        messages = body.get("messages") or []
        model = body.get("model", "stub")
        last = messages[-1] if messages else {}
        content = last.get("content")
        blocks = content if isinstance(content, list) else []
        results = [b for b in blocks if b.get("type") == "tool_result"]
        if last.get("role") == "user" and results:
            result = results[-1].get("content")
            if isinstance(result, list):
                result = "".join(b.get("text", "") for b in result if b.get("type") == "text")
            out = [{"type": "text", "text": f"ECHO tool-result {result}"}]
            stop = "end_turn"
        else:
            parts = anthropic_parts(content)
            call = parse_call(parts)
            choice = (body.get("tool_choice") or {}).get("type")
            if call and choice != "none":
                name, arguments = call
                out = [{"type": "tool_use", "id": "toolu_stub_1", "name": name, "input": arguments}]
                stop = "tool_use"
            else:
                out = [{"type": "text", "text": f"ECHO {compact(parts)}"}]
                stop = "end_turn"
        return {
            "id": "msg_stub",
            "type": "message",
            "role": "assistant",
            "model": model,
            "content": out,
            "stop_reason": stop,
            "usage": {"input_tokens": 10, "output_tokens": 5},
        }


def main():
    parser = argparse.ArgumentParser(description="keyless LLM stub")
    parser.add_argument("--port", type=int, default=18080)
    args = parser.parse_args()
    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    print(f"[stub] listening on http://127.0.0.1:{args.port}", file=sys.stderr, flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
