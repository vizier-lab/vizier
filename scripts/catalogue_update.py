#!/usr/bin/env python3
"""Refresh the bundled model catalogue seed in assets/catalogue/ from models.dev.

Standard library only; run as `python3 -I scripts/catalogue_update.py` (or `just
catalogue-update`). The release workflow runs it with `--keep-on-failure`, so a models.dev
outage keeps the committed snapshot and warns instead of blocking a release.

See specs/015-provider-catalog/contracts/catalogue-snapshot.md.
"""

import argparse
import concurrent.futures
import datetime
import json
import os
import sys
import urllib.error
import urllib.request

SOURCE_URL = "https://models.dev/api.json"
LOGO_URL = "https://models.dev/logos/{}.svg"
FALLBACK_LOGO_ID = "__none__"
USER_AGENT = "vizier-catalogue-update/1"
TIMEOUT = 60

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT_DIR = os.path.join(ROOT, "assets", "catalogue")
LOGO_DIR = os.path.join(OUT_DIR, "logos")

PROVIDER_FIELDS = ("id", "name", "env", "npm", "api", "doc", "models")
MODEL_FIELDS = (
    "id",
    "name",
    "family",
    "attachment",
    "reasoning",
    "tool_call",
    "temperature",
    "knowledge",
    "release_date",
    "last_updated",
    "modalities",
    "limit",
    "cost",
    "status",
    "canonical_model_id",
    "provider",
)

# The `npm` hints vizier can speak (research R2). Kept in step with
# `src/catalogue/data.rs::dialect_for_npm`.
SUPPORTED_NPM = {
    "@ai-sdk/openai-compatible",
    "@openrouter/ai-sdk-provider",
    "@ai-sdk/groq",
    "@ai-sdk/mistral",
    "@ai-sdk/xai",
    "@ai-sdk/togetherai",
    "@ai-sdk/deepinfra",
    "@ai-sdk/cerebras",
    "@ai-sdk/perplexity",
    "@ai-sdk/cohere",
    "@ai-sdk/openai",
    "@ai-sdk/anthropic",
    "@ai-sdk/google",
    "@ai-sdk/azure",
}


class UpdateError(Exception):
    pass


def fetch(url):
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
        return resp.status, dict(resp.headers), resp.read()


def trim(raw):
    out = {}
    for pid, provider in raw.items():
        if not isinstance(provider, dict):
            continue
        p = {k: provider[k] for k in PROVIDER_FIELDS if k in provider}
        models = {}
        for mid, model in (provider.get("models") or {}).items():
            if isinstance(model, dict):
                models[mid] = {k: model[k] for k in MODEL_FIELDS if k in model}
        p["models"] = models
        out[pid] = p
    return out


def validate(data, current):
    if not isinstance(data, dict) or not data:
        raise UpdateError("catalogue is empty or not an object")
    if not any(p.get("npm") in SUPPORTED_NPM for p in data.values()):
        raise UpdateError("catalogue has no provider with a supported dialect")
    if current and len(data) < len(current) * 0.5:
        raise UpdateError(
            f"catalogue shrank from {len(current)} to {len(data)} providers"
        )


def render(data):
    """Keys sorted, one model per line, so git deltas and review diffs stay small."""
    lines = ["{"]
    pids = sorted(data)
    for i, pid in enumerate(pids):
        provider = data[pid]
        head = {k: v for k, v in sorted(provider.items()) if k != "models"}
        head_json = json.dumps(head, sort_keys=True, ensure_ascii=False)[1:-1]
        sep = ", " if head_json else ""
        lines.append(f"{json.dumps(pid)}: {{{head_json}{sep}\"models\": {{")
        mids = sorted(provider.get("models", {}))
        for j, mid in enumerate(mids):
            model = json.dumps(provider["models"][mid], sort_keys=True, ensure_ascii=False)
            comma = "," if j < len(mids) - 1 else ""
            lines.append(f"  {json.dumps(mid)}: {model}{comma}")
        comma = "," if i < len(pids) - 1 else ""
        lines.append(f"}}}}{comma}")
    lines.append("}")
    text = "\n".join(lines) + "\n"
    if json.loads(text) != data:
        raise UpdateError("rendered catalogue does not round-trip")
    return text


def logo_ids(data):
    ids = set(data)
    for provider in data.values():
        for model in provider.get("models", {}).values():
            canonical = model.get("canonical_model_id")
            if isinstance(canonical, str) and "/" in canonical:
                ids.add(canonical.split("/", 1)[0])
    return sorted(ids)


def fetch_logos(ids):
    """Returns ({id: svg bytes}, [ids without a logo], [ids that failed])."""
    _, _, fallback = fetch(LOGO_URL.format(FALLBACK_LOGO_ID))
    logos, no_logo, failed = {}, [], []

    def one(lid):
        try:
            status, _, body = fetch(LOGO_URL.format(lid))
            return lid, status, body
        except urllib.error.HTTPError as e:
            return lid, e.code, None
        except Exception:  # noqa: BLE001 - a failed logo keeps its committed file
            return lid, None, None

    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        for lid, status, body in pool.map(one, ids):
            if status == 200 and body is not None:
                if body == fallback:
                    no_logo.append(lid)
                else:
                    logos[lid] = body
            elif status == 404:
                no_logo.append(lid)
            else:
                failed.append(lid)
    return logos, sorted(no_logo), failed


def snapshot_age():
    try:
        with open(os.path.join(OUT_DIR, "meta.json")) as f:
            fetched = json.load(f)["fetched_at"]
        when = datetime.datetime.fromisoformat(fetched.replace("Z", "+00:00"))
        age = datetime.datetime.now(datetime.timezone.utc) - when
        return f"{fetched} ({age.days} days old)"
    except Exception:  # noqa: BLE001
        return "unknown (no committed snapshot)"


def load_current():
    try:
        with open(os.path.join(OUT_DIR, "api.json")) as f:
            return json.load(f)
    except FileNotFoundError:
        return None


def update():
    status, headers, body = fetch(SOURCE_URL)
    if status != 200:
        raise UpdateError(f"{SOURCE_URL} returned {status}")
    try:
        raw = json.loads(body)
    except ValueError as e:
        raise UpdateError(f"catalogue is not valid JSON: {e}") from e

    data = trim(raw)
    validate(data, load_current())

    ids = logo_ids(data)
    logos, no_logo, failed = fetch_logos(ids)

    os.makedirs(LOGO_DIR, exist_ok=True)
    with open(os.path.join(OUT_DIR, "api.json"), "w", encoding="utf-8") as f:
        f.write(render(data))

    for lid, svg in logos.items():
        with open(os.path.join(LOGO_DIR, f"{lid}.svg"), "wb") as f:
            f.write(svg)
    keep = set(logos) | set(failed)
    for name in os.listdir(LOGO_DIR):
        lid = name[: -len(".svg")] if name.endswith(".svg") else None
        if lid is not None and lid not in keep:
            os.remove(os.path.join(LOGO_DIR, name))

    headers = {k.lower(): v for k, v in headers.items()}
    model_count = sum(len(p.get("models", {})) for p in data.values())
    meta = {
        "schema": 1,
        "source_url": SOURCE_URL,
        "fetched_at": datetime.datetime.now(datetime.timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z"),
        "etag": headers.get("etag"),
        "providers": len(data),
        "models": model_count,
        "no_logo": no_logo,
    }
    with open(os.path.join(OUT_DIR, "meta.json"), "w", encoding="utf-8") as f:
        json.dump(meta, f, indent=2, sort_keys=True)
        f.write("\n")

    size = sum(
        os.path.getsize(os.path.join(dp, fn))
        for dp, _, fns in os.walk(OUT_DIR)
        for fn in fns
    )
    print(
        f"catalogue: {len(data)} providers, {model_count} models; "
        f"{len(logos)} logos, {len(no_logo)} without a logo, {len(failed)} failed; "
        f"{size / 1024:.0f} KiB in assets/catalogue"
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--keep-on-failure",
        action="store_true",
        help="on any error, warn, keep the committed snapshot and exit 0 (release use)",
    )
    args = parser.parse_args()
    try:
        update()
    except Exception as e:  # noqa: BLE001
        if args.keep_on_failure:
            print(f"::warning::catalogue update failed, keeping the committed snapshot: {e}")
            print(f"committed snapshot fetched at {snapshot_age()}")
            summary = os.environ.get("GITHUB_STEP_SUMMARY")
            if summary:
                with open(summary, "a") as f:
                    f.write(
                        f"- Model catalogue not refreshed ({e}); shipping the snapshot "
                        f"fetched at {snapshot_age()}\n"
                    )
            return 0
        print(f"catalogue update failed: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
