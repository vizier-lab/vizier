# Research: Kubernetes Deployment

## Decision 1: Ship one dependency-free Helm 3 application chart

**Decision**: Add `charts/vizier` as a Helm 3 application chart with no chart dependencies. Publish the chart for each release as `oci://ghcr.io/vizier-lab/charts/vizier:<version>`, using the same semantic version and default application image tag as the Vizier release.

**Rationale**: Helm provides versioned defaults, JSON Schema validation, release ownership, upgrades, rollback metadata, and native OCI distribution without adding a runtime dependency to Vizier. A dependency-free chart also honors the requirement not to install PostgreSQL, object storage, ingress controllers, certificate issuers, or observability stacks.

**Alternatives rejected**:
- Raw YAML or Kustomize: simple rendering, but no typed value validation or package/release lifecycle.
- An Operator/CRD: unnecessary cluster-wide code and permissions for one single-instance workload.
- Bundled database/object-store subcharts: conflicts with the explicit external-service boundary and makes recovery ownership ambiguous.

**Sources**:
- Helm chart format: <https://helm.sh/docs/topics/charts/>
- Helm OCI registries: <https://helm.sh/docs/topics/registries/>

## Decision 2: Use a single-replica Deployment with `Recreate`

**Decision**: Render one `apps/v1 Deployment`, fix `replicaCount` to `1`, validate any other value as an error, and set `strategy.type: Recreate`. Do not render an HPA or PodDisruptionBudget.

**Rationale**: Vizier owns a workspace lock and currently supports one process per installation. A default rolling Deployment can overlap old and new pods. `Recreate` preserves controller-managed rollout history while ensuring the old pod terminates before the replacement starts. A one-replica PodDisruptionBudget would either block voluntary maintenance or provide a false availability guarantee.

**Alternatives rejected**:
- StatefulSet: stable ordinal/network identity is not needed; it does not by itself prevent overlap during all update paths.
- RollingUpdate with `maxSurge: 0`: still permits operational ambiguity and is less explicit than `Recreate`.
- HorizontalPodAutoscaler: unsupported until Fleet provides multi-instance coordination.

**Source**: Kubernetes Deployment strategies: <https://kubernetes.io/docs/concepts/workloads/controllers/deployment/>

## Decision 3: Keep two explicit storage profiles

**Decision**:
- `embedded`: default; mount one ReadWriteOnce PVC at `/data`, set `VIZIER_DATA_DIR=/data`, and keep SQLite, local objects, workspace files, and generated encryption key on that volume.
- `managed`: set PostgreSQL and S3 backends through secret-backed environment variables. Mount an `emptyDir` at `/data` only for runtime/workspace transients. Require an operator-managed encryption key Secret so durable state does not depend on the pod filesystem.

Both profiles mount a separate `emptyDir` at `/tmp`.

**Rationale**: Embedded preserves Vizier's zero-service default. Managed storage removes durable database/object bytes from the pod while retaining a writable runtime location. Explicit profiles prevent accidental live switching and make the backup boundary inspectable.

**Alternatives rejected**:
- Always require a PVC: unnecessary durable cluster storage in managed mode and obscures the actual recovery set.
- Put managed credentials in `values.yaml`: rendered release metadata would retain plaintext secrets.
- Generate a Kubernetes Secret in the chart: Helm release state would retain the secret value; operators must provide references instead.

## Decision 4: Add three minimal unauthenticated process-health endpoints

**Decision**: Serve these outside the authenticated API and OpenAPI document:
- `GET /health/startup`
- `GET /health/ready`
- `GET /health/live`

Each successful response is HTTP 200 with `{"status":"ok"}`. An unsuccessful readiness/startup response is HTTP 503 with `{"status":"unavailable"}`. No response includes dependency, user, agent, workspace, configuration, or error details.

`startup` and `ready` become true only after runtime initialization and HTTP bind. `ready` becomes false before shutdown begins. `live` remains true while the HTTP process can serve requests and does not reflect individual provider, channel, agent, database, or object-store health.

**Rationale**: Startup, traffic eligibility, and process deadlock are different signals. Reusing authenticated agent-health APIs would expose detail and couple pod lifecycle to optional integrations.

**Alternative rejected**: TCP-only probes cannot distinguish initialized from merely listening and cannot explicitly drain traffic.

**Source**: Kubernetes probe semantics: <https://kubernetes.io/docs/concepts/workloads/pods/pod-lifecycle/#container-probes>

## Decision 5: Drain HTTP on SIGTERM with a bounded grace period

**Decision**: The binary handles SIGTERM and Ctrl-C. It first marks readiness false, then asks the HTTP server to stop accepting new connections and waits for in-flight HTTP work up to a configured application grace period. At the deadline it stops remaining runtime subsystems; durable turns remain recoverable by the existing turn journal. The chart sets `terminationGracePeriodSeconds` above the application drain duration.

**Rationale**: Kubernetes sends SIGTERM before SIGKILL. Axum's graceful shutdown can drain accepted HTTP requests, while Vizier's existing durable-turn journal provides deterministic recovery for interrupted agent work. Readiness must flip before admission closes so Services can remove the endpoint.

**Alternatives rejected**:
- Immediate process exit: drops accepted requests and may not release the workspace lock cleanly.
- `preStop: sleep`: consumes the pod grace period and duplicates lifecycle logic without informing the process.
- Waiting indefinitely: Kubernetes eventually sends SIGKILL and makes shutdown behavior non-deterministic.

## Decision 6: Make the official image Restricted-profile compatible

**Decision**: Run the image as fixed non-root UID/GID `10001`, give that identity a writable home, and let the chart enforce:
- `runAsNonRoot: true`
- `allowPrivilegeEscalation: false`
- `readOnlyRootFilesystem: true`
- `capabilities.drop: [ALL]`
- `seccompProfile.type: RuntimeDefault`
- no host namespaces, hostPath, privileged mode, or service-account token mount

Declare only `/data` and `/tmp` writable. Support extra read-only volumes/mounts for custom CA bundles.

**Rationale**: These controls satisfy the Kubernetes Restricted Pod Security Standard without needing cluster API access. Image-level non-root behavior also protects users who run the image outside Helm.

**Source**: Pod Security Standards: <https://kubernetes.io/docs/concepts/security/pod-security-standards/>

## Decision 7: Validate values at schema and template boundaries

**Decision**: Add `values.schema.json` for types, enums, ranges, required object shape, and `replicaCount.const = 1`. Add a template validation helper for cross-field rules that JSON Schema cannot express clearly, including managed-storage secret references, embedded persistence requirements, ingress host requirements, and grace-period ordering.

**Rationale**: Schema validation gives immediate local feedback; template checks cover conditional invariants. Runtime still validates actual credentials, connectivity, migrations, pgvector, and bucket permissions because rendering cannot.

**Alternative rejected**: Relying only on pod startup creates a misleading release whose resources exist but can never become ready.

## Decision 8: Keep routing and operational integrations generic

**Decision**: Default to a ClusterIP Service. Offer optional generic `networking.k8s.io/v1` Ingress values for class, annotations, hosts, paths, and TLS Secret names. Expose resource requests/limits, node selectors, affinity, tolerations, topology spread constraints, priority class, pod metadata, image-pull secrets, extra environment references, extra read-only volumes, CA mounts, and existing OTEL environment variables. Do not install controllers or telemetry backends.

**Rationale**: Kubernetes distributions differ in ingress, certificates, secret synchronization, storage classes, and telemetry. Generic native fields avoid coupling the supported package to one vendor.

## Decision 9: Validate packaging in CI and lifecycle in a real cluster

**Decision**: CI runs Helm lint, schema-negative cases, deterministic rendering checks, and a clean-cluster lifecycle smoke test. The smoke test installs the embedded profile, waits for readiness, checks all health endpoints, performs authenticated HTTP and WebSocket/dummyplug interaction, deletes the pod, confirms persisted state, upgrades without overlapping ready pods, and uninstalls without deleting the PVC by default.

Support the oldest and newest Kubernetes minor versions documented for each release; initially document the three minor versions currently supported upstream. Validate both Linux architectures through the multi-architecture image release, while the cluster smoke may run once per architecture-specific runner.

**Rationale**: Template validation does not prove the image boots under security policy, probes transition, WebSockets work, or state survives replacement. Kubernetes itself supports the latest three minor releases, which gives a bounded compatibility matrix.

**Source**: Kubernetes release/version skew policy: <https://kubernetes.io/releases/version-skew-policy/>
