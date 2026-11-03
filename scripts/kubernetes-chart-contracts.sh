#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH='' cd "$(dirname "$0")/.." && pwd)
CHART="$ROOT/charts/vizier"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

kube=(--kube-version 1.32.0)
base=(--set secrets.jwt.name=vizier-runtime --set secrets.jwt.key=jwt-secret)
helm lint "$CHART" "${base[@]}"
for version in 1.32.0 1.34.0; do
  helm template vizier "$CHART" --kube-version "$version" "${base[@]}" >/dev/null
done
helm template vizier "$CHART" "${kube[@]}" "${base[@]}" >"$TMP/embedded.yaml"

require() {
  local pattern=$1 file=$2
  if ! grep -Eq "$pattern" "$file"; then
    printf 'missing required pattern %s in %s\n' "$pattern" "$file" >&2
    exit 1
  fi
}
reject() {
  local pattern=$1 file=$2
  if grep -Eq "$pattern" "$file"; then
    printf 'forbidden pattern %s in %s\n' "$pattern" "$file" >&2
    exit 1
  fi
}
expect_failure() {
  local name=$1
  shift
  if helm template vizier "$CHART" "${kube[@]}" "${base[@]}" "$@" >"$TMP/$name.out" 2>"$TMP/$name.err"; then
    printf 'expected Helm render to fail: %s\n' "$name" >&2
    exit 1
  fi
}

require '^kind: Deployment$' "$TMP/embedded.yaml"
require '^  replicas: 1$' "$TMP/embedded.yaml"
require '^    type: Recreate$' "$TMP/embedded.yaml"
require 'image: "ghcr.io/vizier-lab/vizier:0\.0\.0-dev"' "$TMP/embedded.yaml"
require 'runAsNonRoot: true' "$TMP/embedded.yaml"
require 'readOnlyRootFilesystem: true' "$TMP/embedded.yaml"
require 'allowPrivilegeEscalation: false' "$TMP/embedded.yaml"
require 'automountServiceAccountToken: false' "$TMP/embedded.yaml"
require 'path: /health/startup' "$TMP/embedded.yaml"
require 'path: /health/ready' "$TMP/embedded.yaml"
require 'path: /health/live' "$TMP/embedded.yaml"
require 'helm\.sh/resource-policy: keep' "$TMP/embedded.yaml"
reject '^kind: (ClusterRole|ClusterRoleBinding|CustomResourceDefinition)$' "$TMP/embedded.yaml"
reject 'hostPath:' "$TMP/embedded.yaml"
reject 'value: .*replace-with|value: .*password|value: .*secret' "$TMP/embedded.yaml"

helm template vizier "$CHART" "${kube[@]}" "${base[@]}" \
  --set storage.profile=managed \
  --set secrets.encryptionKey.name=vizier-runtime \
  --set secrets.encryptionKey.key=encryption-key \
  --set managed.database.urlSecretRef.name=vizier-postgres \
  --set managed.database.urlSecretRef.key=url \
  --set managed.objects.endpoint=https://s3.example \
  --set managed.objects.bucket=vizier \
  --set managed.objects.region=us-east-1 \
  --set managed.objects.accessKeySecretRef.name=vizier-s3 \
  --set managed.objects.accessKeySecretRef.key=access-key-id \
  --set managed.objects.secretKeySecretRef.name=vizier-s3 \
  --set managed.objects.secretKeySecretRef.key=secret-access-key >"$TMP/managed.yaml"
require 'value: postgres' "$TMP/managed.yaml"
require 'value: s3' "$TMP/managed.yaml"
require 'name: VIZIER_DATABASE_URL' "$TMP/managed.yaml"
require 'name: VIZIER_ENCRYPTION_KEY' "$TMP/managed.yaml"
reject '^kind: PersistentVolumeClaim$' "$TMP/managed.yaml"

cat >"$TMP/operations.yaml" <<'YAML'
secrets:
  jwt: {name: vizier-runtime, key: jwt-secret}
imagePullSecrets:
  - name: registry
priorityClassName: production
nodeSelector:
  kubernetes.io/os: linux
trustedCa:
  enabled: true
  configMapName: trusted-ca
  key: ca.crt
ingress:
  enabled: true
  className: nginx
  hosts:
    - host: vizier.example
      paths:
        - path: /
          pathType: Prefix
extraEnv:
  - name: OTEL_EXPORTER_OTLP_ENDPOINT
    value: http://collector:4318
extraVolumes:
  - name: custom-ca
    configMap:
      name: custom-ca
extraVolumeMounts:
  - name: custom-ca
    mountPath: /etc/vizier/ca
    readOnly: true
networkPolicy:
  enabled: true
  ingress:
    - {}
  egress:
    - {}
YAML
helm template vizier "$CHART" "${kube[@]}" -f "$TMP/operations.yaml" >"$TMP/operations-rendered.yaml"
require '^kind: Ingress$' "$TMP/operations-rendered.yaml"
require '^kind: NetworkPolicy$' "$TMP/operations-rendered.yaml"
require 'priorityClassName: "?production"?' "$TMP/operations-rendered.yaml"
require 'name: registry' "$TMP/operations-rendered.yaml"
require 'name: OTEL_EXPORTER_OTLP_ENDPOINT' "$TMP/operations-rendered.yaml"
require 'name: custom-ca' "$TMP/operations-rendered.yaml"
require 'name: SSL_CERT_FILE' "$TMP/operations-rendered.yaml"
require 'name: "?trusted-ca"?' "$TMP/operations-rendered.yaml"
reject 'hostPath:' "$TMP/operations-rendered.yaml"


expect_failure replicas --set replicaCount=2
expect_failure latest --set image.tag=latest
expect_failure no-jwt --set secrets.jwt.name= --set secrets.jwt.key=
expect_failure no-persistence --set persistence.enabled=false
expect_failure bad-grace --set shutdown.gracePeriodSeconds=45 --set terminationGracePeriodSeconds=45
expect_failure ingress-no-hosts --set ingress.enabled=true
expect_failure ingress-no-path --set ingress.enabled=true --set 'ingress.hosts[0].host=vizier.example'
expect_failure managed-incomplete --set storage.profile=managed
expect_failure root-user --set podSecurityContext.runAsUser=0
expect_failure service-token --set serviceAccount.automountServiceAccountToken=true
expect_failure host-path --set 'extraVolumes[0].name=bad' --set 'extraVolumes[0].hostPath.path=/'
expect_failure ca-no-configmap --set trustedCa.enabled=true

helm package "$CHART" --destination "$TMP" >/dev/null
[ -f "$TMP/vizier-0.0.0-dev.tgz" ]
printf 'Kubernetes chart contracts passed\n'
