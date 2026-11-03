# Quickstart: Kubernetes Deployment

## Prerequisites

- Helm 3 with OCI support.
- `kubectl` access to a conformant Kubernetes cluster.
- A default StorageClass for the embedded profile, or managed PostgreSQL with pgvector plus S3-compatible object storage.
- A namespace-scoped Secret containing a strong JWT secret. Managed installs also require a stable encryption key, PostgreSQL URL, and S3 credentials.

Examples use release `VERSION`; replace it with one published version. Do not use `latest`.

## Embedded install

```bash
kubectl create namespace vizier
kubectl -n vizier create secret generic vizier-runtime \
  --from-literal=jwt-secret='replace-with-a-long-random-value'

helm install vizier oci://ghcr.io/vizier-lab/charts/vizier \
  --version "$VERSION" \
  --namespace vizier \
  --set secrets.jwt.name=vizier-runtime \
  --set secrets.jwt.key=jwt-secret \
  --set persistence.size=20Gi

kubectl -n vizier rollout status deployment/vizier --timeout=5m
kubectl -n vizier port-forward service/vizier 9999:9999
```

Open `http://127.0.0.1:9999` and complete owner setup.

## Probe smoke

```bash
curl --fail http://127.0.0.1:9999/health/startup
curl --fail http://127.0.0.1:9999/health/ready
curl --fail http://127.0.0.1:9999/health/live
```

Every response is exactly a minimal status object. No credentials or deployment details appear.

## Managed-storage install

Create Secrets without placing their values in Helm values:

```bash
kubectl create namespace vizier-managed
kubectl -n vizier-managed create secret generic vizier-runtime \
  --from-literal=jwt-secret='replace-with-a-long-random-value' \
  --from-literal=encryption-key='base64-encoded-32-byte-key'
kubectl -n vizier-managed create secret generic vizier-postgres \
  --from-literal=url='postgresql://user:password@postgres.example/vizier'
kubectl -n vizier-managed create secret generic vizier-s3 \
  --from-literal=access-key-id='replace-me' \
  --from-literal=secret-access-key='replace-me'
```

Install with a private values file containing references only:

```yaml
storage:
  profile: managed
secrets:
  jwt:
    name: vizier-runtime
    key: jwt-secret
  encryptionKey:
    name: vizier-runtime
    key: encryption-key
managed:
  database:
    urlSecretRef:
      name: vizier-postgres
      key: url
  objects:
    endpoint: https://s3.example
    bucket: vizier
    region: us-east-1
    accessKeySecretRef:
      name: vizier-s3
      key: access-key-id
    secretKeySecretRef:
      name: vizier-s3
      key: secret-access-key
```

```bash
helm install vizier oci://ghcr.io/vizier-lab/charts/vizier \
  --version "$VERSION" \
  --namespace vizier-managed \
  --values managed-values.yaml
kubectl -n vizier-managed rollout status deployment/vizier --timeout=5m
```

## Dummyplug end-to-end gate

After owner setup, create one agent through the WebUI or API using provider `dummyplug`, then open its WebUI conversation through the forwarded Service.

1. Send `tools`.
   - Expected: the reply lists callable tool functions.
2. Send one listed tool name.
   - Expected: dummyplug asks for the JSON input for that tool.
3. Send valid JSON input.
   - Expected: the real agent loop runs the tool and returns its result.
4. Reload the page and reconnect the WebSocket.
   - Expected: saved conversation history is present and the socket continues to receive frames.

This proves authenticated HTTP, WebSocket upgrade, embedded WebUI, runtime, and persistence through the Kubernetes Service—not only probe reachability.

## Pod replacement and persistence

Record the dummyplug conversation, then replace the pod:

```bash
POD=$(kubectl -n vizier get pod -l app.kubernetes.io/instance=vizier -o jsonpath='{.items[0].metadata.name}')
kubectl -n vizier delete pod "$POD"
kubectl -n vizier rollout status deployment/vizier --timeout=5m
```

Reconnect. Owner identity, agent, and conversation must remain. Embedded installs must reuse the same PVC; managed installs must use the same PostgreSQL, bucket, and encryption-key Secret.

## Upgrade and pre-write rollback

```bash
helm upgrade vizier oci://ghcr.io/vizier-lab/charts/vizier \
  --version "$NEW_VERSION" \
  --namespace vizier \
  --reuse-values
kubectl -n vizier rollout status deployment/vizier --timeout=5m
```

Observe that `spec.strategy.type` is `Recreate` and never more than one Vizier pod is running. Roll back only if the replacement failed before accepting writes, unless release notes explicitly state backward schema compatibility:

```bash
helm rollback vizier --namespace vizier
```

## Backup boundary

- Embedded: quiesce Vizier and snapshot/copy the entire workspace PVC. It includes the encryption key.
- Managed: back up PostgreSQL, S3 object/document data, and the encryption-key Secret as one recovery set. Retain non-secret Helm values and Secret names/keys.

Never restore encrypted data without its matching encryption key.

## Removal

```bash
helm uninstall vizier --namespace vizier
```

The chart retains its PVC by default. Operator-created Secrets and external PostgreSQL/S3 data are never removed. Delete those only after backup validation and explicit approval.
