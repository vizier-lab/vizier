# Kubernetes

Vizier publishes a Helm 3 chart for a single server instance. The chart supports the self-contained embedded profile and an opt-in managed PostgreSQL/S3 profile. It does not install PostgreSQL, object storage, ingress, certificate, secret-sync, or observability systems.

## Support boundary

- Kubernetes 1.32–1.34; this range follows the three upstream-supported minor releases and advances with Vizier releases.
- Linux AMD64 and ARM64, matching the official multi-architecture image.
- Exactly one Vizier replica. Fleet support is required before horizontal or active-active operation is safe.
- One immutable chart version maps to the same Vizier image version. Digest pinning is supported.
- A namespace-scoped install needs no cluster role or Kubernetes API token.

The desktop app is not deployed by this chart.

## Install the embedded profile

Embedded storage is the default and needs only a PersistentVolume. It is suitable for evaluation and small single-instance deployments.

```bash
export VERSION=x.y.z
kubectl create namespace vizier
kubectl -n vizier create secret generic vizier-runtime \
  --from-literal=jwt-secret="$(openssl rand -hex 32)"

helm install vizier oci://ghcr.io/vizier-lab/charts/vizier \
  --version "$VERSION" \
  --namespace vizier \
  --set secrets.jwt.name=vizier-runtime \
  --set secrets.jwt.key=jwt-secret \
  --set persistence.size=20Gi

kubectl -n vizier rollout status deployment/vizier --timeout=5m
kubectl -n vizier port-forward service/vizier 9999:9999
```

Open `http://127.0.0.1:9999` and create the first owner. The PVC holds SQLite, uploads, artifacts, memory documents, workspace state, and the generated encryption key. Select a StorageClass whose access mode and topology fit the cluster. A `ReadWriteOnce` volume may delay replacement while its old node detaches; that is safer than a second writer.

## Install managed PostgreSQL and S3

Managed mode requires:

- PostgreSQL reachable from the namespace with the `vector` extension available;
- an S3-compatible bucket with read, write, list, and delete permissions;
- a stable 32-byte encryption key encoded as documented in [Encryption at Rest](../configuration/encryption.md);
- a Secret containing the PostgreSQL URL;
- S3 credentials supplied through Secret references.

Create Secret objects through the cluster's normal secret manager. Do not put values in a Helm values file:

```bash
kubectl create namespace vizier
kubectl -n vizier create secret generic vizier-runtime \
  --from-literal=jwt-secret="$(openssl rand -hex 32)" \
  --from-literal=encryption-key="$VIZIER_ENCRYPTION_KEY"
kubectl -n vizier create secret generic vizier-postgres \
  --from-literal=url="$VIZIER_DATABASE_URL"
kubectl -n vizier create secret generic vizier-s3 \
  --from-literal=access-key-id="$VIZIER_S3_ACCESS_KEY_ID" \
  --from-literal=secret-access-key="$VIZIER_S3_SECRET_ACCESS_KEY"
```

Use references in `managed-values.yaml`:

```yaml
storage:
  profile: managed
secrets:
  jwt: {name: vizier-runtime, key: jwt-secret}
  encryptionKey: {name: vizier-runtime, key: encryption-key}
managed:
  database:
    urlSecretRef: {name: vizier-postgres, key: url}
  objects:
    endpoint: https://s3.example
    bucket: vizier
    region: us-east-1
    pathStyle: true
    accessKeySecretRef: {name: vizier-s3, key: access-key-id}
    secretKeySecretRef: {name: vizier-s3, key: secret-access-key}
```

```bash
helm install vizier oci://ghcr.io/vizier-lab/charts/vizier \
  --version "$VERSION" --namespace vizier --values managed-values.yaml
```

Managed `/data` is ephemeral. Durable recovery depends on PostgreSQL, S3, and the encryption-key Secret—not the pod filesystem.

## Exposure and WebSockets

The default Service is internal `ClusterIP`. Prefer a Gateway or Ingress already operated by the cluster. Generic Ingress values accept class, annotations, host/path rules, and TLS Secret names:

```yaml
ingress:
  enabled: true
  className: nginx
  annotations: {}
  hosts:
    - host: vizier.example.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: vizier-tls
      hosts: [vizier.example.com]
```

The proxy must support HTTP upgrade and long-lived WebSocket connections. Controller-specific timeout and WebSocket annotations belong in `ingress.annotations`. The chart never creates a certificate issuer.

## Security and cluster integration

The supported defaults satisfy the Restricted Pod Security profile: non-root UID/GID 10001, no privilege escalation, all capabilities dropped, `RuntimeDefault` seccomp, read-only root filesystem, no host namespaces or hostPath, and no mounted service-account token. Writable mounts are `/data` and `/tmp`.

Configure CPU, memory, and ephemeral-storage requests/limits under `resources`. Standard values are available for `nodeSelector`, `affinity`, `tolerations`, `topologySpreadConstraints`, `priorityClassName`, pod metadata, and `imagePullSecrets`.

For a private CA, create one ConfigMap containing a PEM bundle and enable the
dedicated read-only mount. The chart sets `SSL_CERT_FILE` for PostgreSQL, S3,
model-provider, and other outbound TLS clients:

```yaml
trustedCa:
  enabled: true
  configMapName: corporate-ca
  key: ca.crt
```

Provider and channel credentials should use `extraEnvFrom` or
`valueFrom.secretKeyRef`, never plaintext `extraEnv.value`.

Existing OpenTelemetry settings work through environment variables. The chart does not deploy a collector. NetworkPolicy is opt-in because PostgreSQL, S3, DNS, model-provider, channel, and telemetry egress destinations differ per installation.

## Probes and graceful termination

The chart uses three unauthenticated, detail-free endpoints:

| Endpoint | Meaning | Failure effect |
|---|---|---|
| `/health/startup` | Storage validation, migrations, runtime, and HTTP bind completed | Holds other probes during startup |
| `/health/ready` | Process accepts new requests | Removes the pod from Service endpoints |
| `/health/live` | HTTP process can answer | Restarts only a stuck/dead process |

Agent, provider, channel, and temporary external dependency failures do not fail liveness. Probe bodies disclose no users, agents, paths, endpoints, buckets, credentials, or internal errors.

On SIGTERM, readiness becomes unavailable before HTTP admission closes. Vizier drains accepted HTTP work for `shutdown.gracePeriodSeconds`; durable interrupted turns resume after restart. Kubernetes' `terminationGracePeriodSeconds` must be greater and is the final SIGKILL boundary.

## Upgrade and rollback

```bash
helm upgrade vizier oci://ghcr.io/vizier-lab/charts/vizier \
  --version "$NEW_VERSION" --namespace vizier --reuse-values
kubectl -n vizier rollout status deployment/vizier --timeout=5m
```

The Deployment strategy is `Recreate`: the old pod terminates before the new pod starts. Preserve storage and encryption-key references. Read release notes before upgrades because startup may run forward-only database migrations.

Rollback is safe only when the replacement failed before accepting writes, unless release notes explicitly guarantee the older version can read the new schema:

```bash
helm rollback vizier --namespace vizier
```

## Backup and restore

### Embedded

1. Stop or scale down Vizier so SQLite and files are quiescent.
2. Snapshot or copy the complete PVC—not selected directories.
3. Preserve Helm values and the chart version.
4. Restore the complete volume before starting the same or a compatible Vizier version.

The generated encryption key is on the PVC. A partial backup can leave database references and file bytes inconsistent.

### Managed

Treat these as one recovery set:

1. PostgreSQL backup;
2. S3 bucket data (prefer versioning and a protected retention policy);
3. the matching encryption-key Secret;
4. non-secret Helm values and Secret object/key names.

Restore mutually compatible PostgreSQL and S3 points, then restore the encryption key before starting Vizier. Encrypted rows are unrecoverable with a different key.

## Storage migration

Changing `storage.profile` is not a live configuration switch. Stop Vizier, run the supported `vizier storage migrate` workflow against the quiescent source, validate the destination, update Helm values/Secret references, and then start the managed deployment. Never let old and new processes write during migration.

## Troubleshooting

- **Helm render fails**: read the named value. Common failures are missing JWT/encryption/database/S3 Secret references, managed bucket settings, ingress hosts, multiple replicas, or an application grace period not below the pod grace period.
- **Startup never succeeds**: inspect `kubectl logs`. Configuration, encryption-key mismatch, database/schema/pgvector, migration, bucket permission, and credential errors are logged without credential values.
- **Pending PVC/pod**: inspect StorageClass topology, capacity, access mode, and prior-node detach state.
- **Ready becomes false**: check whether termination has started or a mandatory server subsystem stopped. Individual agents/channels do not affect liveness.
- **Ingress UI works but chat does not**: verify WebSocket upgrade headers and controller idle/read timeouts.
- **Image pull fails**: confirm the immutable tag/digest supports the node architecture and configure `imagePullSecrets` for private mirrors.
- **Read-only filesystem errors**: direct application writes to `/data` or `/tmp`; mount custom writable locations explicitly only when required.

## Removal

```bash
helm uninstall vizier --namespace vizier
```

This removes application resources. A chart-created embedded PVC is retained by default. Operator-created Secrets, external PostgreSQL, and S3 data are never deleted. After backup verification, delete durable state with a separate explicit operator action. Deleting the namespace may bypass that protection; inspect retained resources first.
