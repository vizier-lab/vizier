# Implementation Plan: Kubernetes Deployment

**Branch**: `046-kubernetes-deployment` | **Date**: 2026-11-03 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/046-kubernetes-deployment/spec.md`

## Summary

Ship a dependency-free Helm 3 chart for one Vizier instance in either embedded
PVC-backed or managed PostgreSQL/S3 mode. Add minimal unauthenticated workload
health endpoints, bounded SIGTERM/HTTP draining, a Restricted-profile-compatible
non-root image, release-time OCI chart publishing, cluster lifecycle validation,
and operator documentation. The chart uses a single-replica `Deployment` with
`Recreate`, references existing Secrets rather than rendering secret values, and
does not install external stateful or routing services.

## Technical Context

**Language/Version**: Rust 2024 edition (`rust-toolchain.toml`); Helm 3 templates and JSON Schema; Kubernetes YAML

**Primary Dependencies**: Existing Tokio signal support, Axum 0.8 graceful shutdown, Helm 3, Kubernetes `apps/v1`, `v1`, and `networking.k8s.io/v1` APIs; no new Rust crate

**Storage**: Embedded SQLite/local object and document storage on one PVC, or opt-in PostgreSQL with pgvector plus S3-compatible object/document storage; Kubernetes Secrets preserve credentials and the managed encryption key

**Testing**: Rust unit/HTTP integration tests, `cargo test --workspace`, `cargo clippy --workspace`, `helm lint`, positive and negative `helm template`, container startup smoke, and clean-cluster install/pod-replacement lifecycle smoke; end-to-end: authenticated HTTP and WebSocket dummyplug conversation through the Kubernetes Service as scripted in [quickstart.md](./quickstart.md)

**Target Platform**: Linux AMD64 and ARM64 containers; the three Kubernetes minor versions supported upstream at release time

**Project Type**: Rust server/CLI plus distribution packaging

**Performance Goals**: Probe handling remains constant-time and allocation-minimal; readiness changes within 5 seconds of termination; normal shutdown completes within the configured grace period; healthy installs are ready within 5 minutes excluding migrations and image pulls

**Constraints**: Exactly one process per installation; no cluster-wide permissions; no plaintext secret values in Helm configuration; no bundled PostgreSQL/S3/ingress/cert/telemetry systems; non-root read-only root filesystem; no mutable default image tag; storage profile changes require an offline migration

**Scale/Scope**: One namespaced Deployment, Service, optional Ingress/NetworkPolicy, ServiceAccount, ConfigMap, and embedded PVC; three health endpoints; one release workflow extension; Kubernetes operations documentation

## Constitution Check

*GATE: Passed before Phase 0 research and re-checked after Phase 1 design.*

| Principle / Gate | Result | Evidence |
|---|---|---|
| I. Lean by Default | PASS | Uses Helm and existing Tokio/Axum capabilities; adds no Rust dependency, operator, CRD, or bundled service. |
| II. Trait-Based Extensibility | PASS | No provider, storage, channel, or tool dispatch changes. Packaging selects existing storage implementations through configuration. |
| III. Self-Contained Default | PASS | Embedded SQLite/local storage is the chart default. PostgreSQL and S3 remain explicit managed-profile choices. Bare binary and desktop behavior remain unchanged. |
| IV. Portability | PASS | Signal handling uses Tokio's cross-platform Ctrl-C plus Unix SIGTERM behind the existing target boundary. Container output remains AMD64/ARM64. |
| V. Errors & Observability | PASS | Runtime errors continue through existing result types and `tracing`; health responses intentionally omit diagnostics. |
| Single-binary output | PASS | The chart deploys the existing image/binary; no sidecar or external runtime component is required for embedded mode. |
| No required external services | PASS | Default chart profile needs only standard Kubernetes persistent storage. |
| Cross-compilation | PASS | No new native or third-party Rust dependency. |
| Runtime/build manual verification | REQUIRED | Run the changed binary, container, Helm render/install, probe transition, SIGTERM drain, and pod replacement. |
| Dummyplug end-to-end gate | REQUIRED | The HTTP/WebSocket surface is changed, so run the quickstart dummyplug flow through the Kubernetes Service. |
| Conventional commits and matching branches | PASS | Work remains on `046-kubernetes-deployment`; all generated and final commit/PR titles use conventional format. |

No constitution violation requires a complexity exception.

## Design Decisions

1. **Package**: `charts/vizier`, Helm 3 application chart, dependency-free, published to GHCR as OCI with chart/app version parity.
2. **Controller**: one `Deployment`, `replicaCount: 1`, `strategy.type: Recreate`; no HPA or PodDisruptionBudget.
3. **Storage**: embedded mode mounts `/data` from a retained PVC; managed mode uses PostgreSQL/S3 Secrets and ephemeral `/data`, with a required persistent encryption-key Secret.
4. **Health**: `/health/startup`, `/health/ready`, and `/health/live` sit outside authenticated APIs and return only a minimal status.
5. **Shutdown**: SIGTERM/Ctrl-C mark readiness false, gracefully stop HTTP admission, drain up to the application deadline, then stop runtime work before Kubernetes' outer deadline.
6. **Security**: image UID/GID 10001; chart defaults to the Restricted Pod Security controls, read-only root, writable `/data` and `/tmp`, and no service-account token.
7. **Validation**: JSON Schema handles shape/ranges and template helpers enforce cross-field storage, route, secret, replica, and grace invariants.
8. **Verification**: static render checks plus a real cluster install, probes, authenticated HTTP/WebSocket dummyplug turn, pod replacement, persistence check, and no-overlap upgrade.

Full rationale and rejected alternatives are in [research.md](./research.md).

## Project Structure

### Documentation (this feature)

```text
specs/046-kubernetes-deployment/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── health.md
│   ├── helm-values.md
│   └── lifecycle.md
└── tasks.md
```

### Source Code (repository root)

```text
charts/vizier/
├── Chart.yaml
├── values.yaml
├── values.schema.json
├── README.md
└── templates/
    ├── _helpers.tpl
    ├── _validate.tpl
    ├── deployment.yaml
    ├── service.yaml
    ├── serviceaccount.yaml
    ├── pvc.yaml
    ├── ingress.yaml
    ├── networkpolicy.yaml
    └── NOTES.txt

crates/vizier-http/src/
├── health.rs                  # minimal health state and handlers
└── lib.rs                     # public router/serve integration and graceful serving

crates/vizier/src/cli/
├── run.rs                     # shutdown-grace CLI/environment input
└── server.rs                  # OS signal selection and bounded lifecycle

Dockerfile                     # fixed non-root runtime identity
docker-entrypoint.sh           # environment/CLI mapping
.github/workflows/
├── docker-publish.yml         # OCI chart package and publish
└── kubernetes.yml             # lint/render/cluster lifecycle validation

docs/src/
├── SUMMARY.md
└── deployment/
    └── kubernetes.md

crates/vizier-http/tests/
└── health.rs
```

**Structure Decision**: Keep process behavior in the existing binary/HTTP crates,
distribution assets in a conventional root `charts/` tree, release automation in
the existing image workflow, and operator guidance in the existing docs source.
No new application crate or deployment controller is warranted.

## Implementation Sequence

1. Add process-health state and endpoint tests without changing authenticated API contracts.
2. Add signal-aware bounded HTTP/runtime shutdown and exercise readiness transition plus lock release.
3. Make the release image non-root and verify direct container startup with writable declared paths.
4. Build the chart, schema, conditional validation, profiles, security defaults, routes, and operational overrides.
5. Add static Helm checks and the clean-cluster lifecycle workflow.
6. Publish the version-matched OCI chart from the release workflow.
7. Add installation, ingress/WebSocket, backup/restore, migration, upgrade/rollback, troubleshooting, and removal docs.
8. Run workspace checks, binary/container smoke, chart negative/positive renders, and the real-cluster dummyplug/persistence lifecycle.

## Complexity Tracking

No constitution violations.
