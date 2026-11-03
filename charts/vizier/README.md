# Vizier Helm chart

This chart installs one Vizier server in one namespace. It does not install PostgreSQL, S3, an ingress controller, certificate management, or an observability backend.

## Requirements

- Kubernetes 1.32–1.34 (the range is updated with supported releases)
- Helm 3
- A default or selected StorageClass for `embedded`
- PostgreSQL with pgvector and S3-compatible storage for `managed`

Vizier is single-instance. `replicaCount` is fixed to `1`, and the Deployment uses `Recreate` so updates never overlap processes using one workspace.

## Install embedded storage

Create a strong JWT secret outside Helm:

```bash
kubectl create namespace vizier
kubectl -n vizier create secret generic vizier-runtime \
  --from-literal=jwt-secret="$(openssl rand -hex 32)"
helm install vizier oci://ghcr.io/vizier-lab/charts/vizier \
  --version VERSION --namespace vizier \
  --set secrets.jwt.name=vizier-runtime \
  --set secrets.jwt.key=jwt-secret
```

The default PVC contains SQLite, files, memory documents, workspace state, and the generated encryption key. It carries Helm's `keep` policy and remains after uninstall.

## Install managed storage

Create separate Secrets for the JWT secret, stable encryption key, PostgreSQL URL, and S3 credentials. Values contain references only:

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
    accessKeySecretRef: {name: vizier-s3, key: access-key-id}
    secretKeySecretRef: {name: vizier-s3, key: secret-access-key}
```


## External access

The default Service is `ClusterIP`. Set `ingress.enabled`, `ingress.className`, hosts, annotations, and TLS Secret names for an existing controller. The controller must preserve WebSocket upgrades and long-lived connections. No controller-specific behavior is assumed.

## Security and operations

Defaults run UID/GID 10001, drop all capabilities, disallow privilege escalation, use `RuntimeDefault` seccomp, make the root filesystem read-only, and disable service-account token mounting. Only `/data` and `/tmp` are writable. Resource requests/limits, scheduling, pull Secrets, a `trustedCa` ConfigMap mounted through `SSL_CERT_FILE`, non-secret environment variables, Secret/ConfigMap `envFrom`, and OTEL variables are configurable.

An optional NetworkPolicy accepts operator-defined ingress/egress rules. It is disabled by default because required egress destinations are installation-specific.

## Health and shutdown

- `/health/startup`: initialization completed
- `/health/ready`: accepts new work
- `/health/live`: serving process is alive

Responses contain only `{"status":"ok"}` or `{"status":"unavailable"}`. SIGTERM first removes readiness, then drains accepted HTTP work for `shutdown.gracePeriodSeconds`. `terminationGracePeriodSeconds` must be greater.

## Backup, upgrades, and removal

- Embedded backup: stop Vizier and snapshot the entire PVC.
- Managed backup: capture PostgreSQL, S3 data, and the matching encryption-key Secret as one recovery set.
- Upgrade with a versioned chart/image. Roll back only before the replacement accepts writes unless release notes guarantee backward schema compatibility.
- Changing `storage.profile` is not live. Run the documented stopped-workspace migration.
- `helm uninstall` removes application resources, not a kept PVC, operator Secrets, PostgreSQL, or S3 data.

Full troubleshooting and recovery guidance: <https://www.vizier.rs/docs/deployment/kubernetes/>.
