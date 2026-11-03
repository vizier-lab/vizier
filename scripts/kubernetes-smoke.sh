#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH='' cd "$(dirname "$0")/.." && pwd)
CHART=${CHART:-$ROOT/charts/vizier}
NAMESPACE=${NAMESPACE:-vizier-smoke}
RELEASE=${RELEASE:-vizier}
IMAGE_REPOSITORY=${IMAGE_REPOSITORY:-ghcr.io/vizier-lab/vizier}
IMAGE_TAG=${IMAGE_TAG:-0.0.0-dev}
IMAGE_PULL_POLICY=${IMAGE_PULL_POLICY:-IfNotPresent}
PORT=${PORT:-19999}
DUMMYPLUG_IMAGE=${DUMMYPLUG_IMAGE:-}
DUMMYPLUG_URL=${DUMMYPLUG_URL:-}
DUMMYPLUG_NODE_SELECTOR=${DUMMYPLUG_NODE_SELECTOR:-'{}'}
DUMMYPLUG_TOLERATIONS=${DUMMYPLUG_TOLERATIONS:-'[]'}
PF_PID=

cleanup() {
  status=$?
  [ -z "${PF_PID:-}" ] || kill "$PF_PID" 2>/dev/null || true
  if [ "$status" -ne 0 ] && [ "${KEEP_NAMESPACE_ON_FAILURE:-false}" = true ]; then
    printf 'keeping failed smoke namespace %s\n' "$NAMESPACE" >&2
    return "$status"
  fi
  if [ "${KEEP_NAMESPACE:-false}" != true ]; then
    helm uninstall "$RELEASE" -n "$NAMESPACE" >/dev/null 2>&1 || true
    kubectl delete namespace "$NAMESPACE" --wait=false >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

kubectl create namespace "$NAMESPACE" >/dev/null
kubectl label namespace "$NAMESPACE" pod-security.kubernetes.io/enforce=restricted --overwrite >/dev/null
if [ -n "$DUMMYPLUG_IMAGE" ]; then
  DUMMYPLUG_URL=${DUMMYPLUG_URL:-http://dummyplug:9998/v1}
  kubectl -n "$NAMESPACE" apply -f - >/dev/null <<YAML
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dummyplug
spec:
  replicas: 1
  strategy:
    type: Recreate
  selector:
    matchLabels:
      app: dummyplug
  template:
    metadata:
      labels:
        app: dummyplug
    spec:
      automountServiceAccountToken: false
      enableServiceLinks: false
      nodeSelector: ${DUMMYPLUG_NODE_SELECTOR}
      tolerations: ${DUMMYPLUG_TOLERATIONS}
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        runAsGroup: 1000
        seccompProfile:
          type: RuntimeDefault
      containers:
        - name: dummyplug
          image: ${DUMMYPLUG_IMAGE}
          imagePullPolicy: ${DUMMYPLUG_IMAGE_PULL_POLICY:-IfNotPresent}
          ports:
            - name: http
              containerPort: 9998
          readinessProbe:
            httpGet:
              path: /healthz
              port: http
          securityContext:
            allowPrivilegeEscalation: false
            capabilities:
              drop: [ALL]
---
apiVersion: v1
kind: Service
metadata:
  name: dummyplug
spec:
  selector:
    app: dummyplug
  ports:
    - name: http
      port: 9998
      targetPort: http
YAML
  kubectl -n "$NAMESPACE" rollout status deployment/dummyplug --timeout=5m
fi
kubectl -n "$NAMESPACE" create secret generic vizier-runtime \
  --from-literal=jwt-secret="$(openssl rand -hex 32)" >/dev/null

helm_args=("$@")
if [ -n "$DUMMYPLUG_URL" ]; then
  helm_args+=(--set-string extraEnv[0].name=DUMMYPLUG_URL)
  helm_args+=(--set-string "extraEnv[0].value=$DUMMYPLUG_URL")
fi
helm upgrade --install "$RELEASE" "$CHART" -n "$NAMESPACE" \
  --set secrets.jwt.name=vizier-runtime \
  --set secrets.jwt.key=jwt-secret \
  --set image.repository="$IMAGE_REPOSITORY" \
  --set image.tag="$IMAGE_TAG" \
  --set image.pullPolicy="$IMAGE_PULL_POLICY" \
  "${helm_args[@]}"

kubectl -n "$NAMESPACE" rollout status deployment/"$RELEASE" --timeout=5m
POD=$(kubectl -n "$NAMESPACE" get pod -l app.kubernetes.io/instance="$RELEASE" -o jsonpath='{.items[0].metadata.name}')
[ "$(kubectl -n "$NAMESPACE" exec "$POD" -- id -u)" = 10001 ]
kubectl -n "$NAMESPACE" exec "$POD" -- sh -c 'touch /data/.write-check /tmp/.write-check && test ! -e /var/run/secrets/kubernetes.io/serviceaccount/token'
kubectl -n "$NAMESPACE" port-forward service/"$RELEASE" "$PORT":9999 >"${TMPDIR:-/tmp}/vizier-port-forward.log" 2>&1 &
PF_PID=$!
for _ in $(seq 1 30); do
  curl --silent --fail "http://127.0.0.1:$PORT/health/ready" >/dev/null && break
  sleep 1
done

for endpoint in startup ready live; do
  body=$(curl --silent --fail "http://127.0.0.1:$PORT/health/$endpoint")
  [ "$body" = '{"status":"ok"}' ] || { printf 'unexpected %s response: %s\n' "$endpoint" "$body" >&2; exit 1; }
done

VIZIER_URL="http://127.0.0.1:$PORT" node "$ROOT/scripts/kubernetes-smoke.mjs"
POD=$(kubectl -n "$NAMESPACE" get pod -l app.kubernetes.io/instance="$RELEASE" -o jsonpath='{.items[0].metadata.name}')
SHUTDOWN_LOG="${TMPDIR:-/tmp}/vizier-shutdown.log"
kubectl -n "$NAMESPACE" logs -f "$POD" >"$SHUTDOWN_LOG" 2>&1 &
LOG_PID=$!
TERMINATION_STARTED=$(date +%s)
kubectl -n "$NAMESPACE" delete pod "$POD" --wait=true >/dev/null
TERMINATION_SECONDS=$(($(date +%s) - TERMINATION_STARTED))
wait "$LOG_PID" 2>/dev/null || true
[ "$TERMINATION_SECONDS" -le 45 ] || { printf 'pod termination exceeded 45 seconds: %ss\n' "$TERMINATION_SECONDS" >&2; exit 1; }
grep -q 'graceful shutdown started' "$SHUTDOWN_LOG"
grep -q 'vizier stopped' "$SHUTDOWN_LOG"
kubectl -n "$NAMESPACE" rollout status deployment/"$RELEASE" --timeout=5m
kill "$PF_PID" 2>/dev/null || true
wait "$PF_PID" 2>/dev/null || true
kubectl -n "$NAMESPACE" port-forward service/"$RELEASE" "$PORT":9999 >"${TMPDIR:-/tmp}/vizier-port-forward.log" 2>&1 &
PF_PID=$!
for _ in $(seq 1 30); do
  curl --silent --fail "http://127.0.0.1:$PORT/health/ready" >/dev/null && break
  sleep 1
done
VIZIER_URL="http://127.0.0.1:$PORT" node "$ROOT/scripts/kubernetes-smoke.mjs" --verify

# A metadata-only upgrade forces pod replacement. Recreate must keep the number of running
# Vizier pods at one or zero throughout the rollout.
helm upgrade "$RELEASE" "$CHART" -n "$NAMESPACE" --reuse-values \
  --set-string podAnnotations.vizier-smoke-revision="$(date +%s)" >/dev/null &
UPGRADE_PID=$!
while kill -0 "$UPGRADE_PID" 2>/dev/null; do
  RUNNING=$(kubectl -n "$NAMESPACE" get pods \
    -l app.kubernetes.io/instance="$RELEASE" \
    --field-selector=status.phase=Running -o name | wc -l | tr -d ' ')
  [ "$RUNNING" -le 1 ] || { printf 'overlapping Vizier pods observed during upgrade\n' >&2; exit 1; }
  sleep 1
done
wait "$UPGRADE_PID"
kubectl -n "$NAMESPACE" rollout status deployment/"$RELEASE" --timeout=5m

# No writes occur between the upgrade and this pre-write rollback.
helm rollback "$RELEASE" 1 -n "$NAMESPACE" --wait --timeout=5m >/dev/null
kill "$PF_PID" 2>/dev/null || true
wait "$PF_PID" 2>/dev/null || true
kubectl -n "$NAMESPACE" port-forward service/"$RELEASE" "$PORT":9999 >"${TMPDIR:-/tmp}/vizier-port-forward.log" 2>&1 &
PF_PID=$!
for _ in $(seq 1 30); do
  curl --silent --fail "http://127.0.0.1:$PORT/health/ready" >/dev/null && break
  sleep 1
done
VIZIER_URL="http://127.0.0.1:$PORT" node "$ROOT/scripts/kubernetes-smoke.mjs" --verify

[ "$(kubectl -n "$NAMESPACE" get deployment "$RELEASE" -o jsonpath='{.spec.replicas}')" = 1 ]
[ "$(kubectl -n "$NAMESPACE" get deployment "$RELEASE" -o jsonpath='{.spec.strategy.type}')" = Recreate ]
printf 'Kubernetes lifecycle smoke passed\n'
