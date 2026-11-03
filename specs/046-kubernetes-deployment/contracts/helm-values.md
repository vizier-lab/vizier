# Contract: Helm Values

The chart is `charts/vizier`. `values.schema.json` validates types and local constraints; `_validate.tpl` rejects conditional contradictions during `helm lint`, `helm template`, install, and upgrade.

## Core values

| Value | Default | Contract |
|---|---:|---|
| `replicaCount` | `1` | Any other value is rejected; Fleet is required for multiple replicas. |
| `image.repository` | `ghcr.io/vizier-lab/vizier` | Override supports private mirrors. |
| `image.tag` | chart `appVersion` | Must be immutable in production; no `latest` default. |
| `image.digest` | empty | When set, takes precedence over tag. |
| `image.pullPolicy` | `IfNotPresent` | Standard Kubernetes policy. |
| `imagePullSecrets` | `[]` | Existing Secret names only. |
| `storage.profile` | `embedded` | Enum: `embedded`, `managed`. |
| `terminationGracePeriodSeconds` | `45` | Must exceed `shutdown.gracePeriodSeconds`. |
| `shutdown.gracePeriodSeconds` | `30` | Positive bounded application drain duration. |

## Secret references

Every secret value uses `{name, key}`. Plaintext secret value fields do not exist.

| Value | Required when | Runtime environment |
|---|---|---|
| `secrets.jwt` | recommended for every network install | `VIZIER_JWT_SECRET` |
| `secrets.encryptionKey` | managed profile; optional embedded override | `VIZIER_ENCRYPTION_KEY` |
| `managed.database.urlSecretRef` | managed profile | `VIZIER_DATABASE_URL` |
| `managed.objects.accessKeySecretRef` | managed profile | `VIZIER_S3_ACCESS_KEY_ID` |
| `managed.objects.secretKeySecretRef` | managed profile | `VIZIER_S3_SECRET_ACCESS_KEY` |
| `managed.objects.sessionTokenSecretRef` | optional temporary credentials | `VIZIER_S3_SESSION_TOKEN` |

`extraEnv` supports non-sensitive values. `extraEnvFrom` and `extraSecretEnv` support provider/channel credentials and external-secret projections without chart-specific fields.

## Embedded profile

- `persistence.enabled: true` is required.
- `persistence.existingClaim` uses an existing PVC and suppresses chart PVC creation.
- Otherwise `persistence.size`, `storageClass`, `accessModes`, and annotations define one PVC.
- The PVC mounts at `/data`; `VIZIER_DATA_DIR=/data`.
- Uninstall retains a chart-created PVC by default through the Helm keep policy; deletion is explicit and documented.

## Managed profile

Required:
- PostgreSQL URL Secret reference.
- S3-compatible endpoint and bucket.
- S3 credential Secret references.
- Encryption key Secret reference.

The pod sets PostgreSQL/entity storage and S3 object/document storage explicitly. `/data` is an `emptyDir`, not a recovery component.

## Networking

- `service.type` defaults to `ClusterIP`.
- `ingress.enabled` defaults to `false`.
- Enabled ingress requires at least one host/path and accepts `className`, annotations, and TLS Secret names.
- No ingress controller or certificate resource is installed.
- WebSocket support depends on the selected controller preserving HTTP upgrade and long-lived connections; controller-specific annotations remain operator values.

## Operations and security

The contract exposes:
- requests/limits including CPU, memory, and ephemeral storage;
- node selector, affinity, tolerations, topology spread constraints, and priority class;
- pod labels/annotations and service annotations;
- pod/container security context with secure defaults;
- service-account creation/name with token automount disabled by default;
- custom CA bundles through `trustedCa` as a read-only ConfigMap mount exported by `SSL_CERT_FILE`;
- OTEL environment variables and arbitrary non-secret environment values;
- optional NetworkPolicy with operator-provided egress rules.

Overrides may tighten security. The supported profile does not permit privileged mode, host namespaces, hostPath, added capabilities, or root execution.

## Render-time failures

Rendering fails before rollout for:
- `replicaCount != 1`;
- unknown storage profile;
- embedded profile without persistence;
- managed profile missing PostgreSQL, S3, or encryption-key settings;
- incomplete Secret reference (`name` without `key` or vice versa);
- ingress enabled without hosts;
- application grace greater than or equal to pod termination grace;
- a chart-managed PVC with no valid size.
