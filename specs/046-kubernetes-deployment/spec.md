# Feature Specification: Kubernetes Deployment

**Feature Branch**: `046-kubernetes-deployment`

**Created**: 2026-11-03

**Status**: Draft

**Input**: User description: "Support production deployment of Vizier on Kubernetes"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Install Vizier on a Cluster (Priority: P1)

A platform operator installs a versioned, first-party Vizier deployment into a chosen namespace, supplies secrets through existing cluster secret management, and reaches the WebUI through a cluster service or an optional external route. The installation supports both an embedded single-instance profile and a managed-storage profile without requiring operators to author Kubernetes resources from scratch.

**Why this priority**: A repeatable, supported installation is the minimum useful Kubernetes capability. Without it, every operator maintains an unreviewed private deployment.

**Independent Test**: Install Vizier into an empty namespace on a conformant cluster, provide the required secrets and storage settings, and complete first-user setup through the WebUI.

**Acceptance Scenarios**:

1. **Given** an empty namespace and no external database or object store, **when** an operator selects the embedded profile and allocates persistent storage, **then** one Vizier instance becomes ready and its state survives pod replacement.
2. **Given** reachable PostgreSQL and S3-compatible services, **when** an operator selects managed storage and references valid credentials, **then** Vizier becomes ready without storing application files on the pod filesystem.
3. **Given** an installation with no external route enabled, **when** it completes, **then** Vizier is reachable only inside the cluster until the operator explicitly exposes it.
4. **Given** an operator-supplied secret, **when** deployment configuration is rendered or inspected, **then** the secret value is not copied into non-secret configuration or command-line arguments.

---

### User Story 2 - Operate a Healthy Single Instance (Priority: P1)

A cluster controller can distinguish startup, readiness, and liveness without authentication or disclosure of deployment details. It removes an instance from service before termination, gives active work a bounded drain period, and restarts a failed process automatically.

**Why this priority**: Kubernetes cannot safely manage traffic or lifecycle using authenticated agent-health APIs. Incorrect probes cause restart loops, dropped requests, or traffic to a half-started process.

**Independent Test**: Exercise startup, dependency failure, normal traffic, graceful termination, and forced process failure while observing probe state, endpoints, and persisted state.

**Acceptance Scenarios**:

1. **Given** Vizier is starting migrations and subsystems, **when** probes run, **then** startup may pass only after initialization completes and readiness remains false until the service can accept requests.
2. **Given** Vizier is healthy, **when** readiness and liveness are checked, **then** both succeed without credentials and return no configuration, identity, secret, or dependency details.
3. **Given** termination has begun, **when** readiness is checked, **then** it fails before the process stops accepting new work.
4. **Given** active requests during termination, **when** the configured grace period is sufficient, **then** accepted work is allowed to finish before shutdown; after the deadline, remaining work is stopped consistently and the process exits.
5. **Given** an unreachable required database or object store, **when** Vizier starts, **then** it never becomes ready and emits a credential-safe, actionable failure reason.

---

### User Story 3 - Upgrade and Roll Back Safely (Priority: P2)

A platform operator upgrades Vizier using a new immutable application version without running two Vizier processes against one deployment. Persistent state and the encryption key survive replacement. If the new version cannot start before it accepts writes, the operator can restore the prior deployment version.

**Why this priority**: Vizier currently supports one process per deployment. A default rolling update could overlap old and new processes, conflict on the workspace lock, and make rollback unsafe.

**Independent Test**: Install a prior supported version with data, upgrade to a new version, verify state, then perform a pre-write rollback while observing that at most one application instance runs at any time.

**Acceptance Scenarios**:

1. **Given** a running installation, **when** it is upgraded, **then** the old instance terminates before the replacement starts using the same deployment state.
2. **Given** an embedded installation, **when** its pod is replaced or upgraded, **then** the database, files, and encryption key remain available.
3. **Given** a managed-storage installation, **when** its pod is replaced or upgraded, **then** database records, object data, and the encryption key remain available.
4. **Given** a replacement that fails before accepting writes, **when** the operator restores the prior version, **then** the prior version can use the unchanged persistent state according to the documented compatibility boundary.
5. **Given** an operator requests more than one replica, **when** deployment configuration is validated, **then** it is rejected with an explanation that multi-instance operation requires Fleet support.

---

### User Story 4 - Fit Cluster Security and Operations (Priority: P2)

A platform operator can apply normal resource, scheduling, security, networking, and observability policies without rebuilding the Vizier image or editing the deployment package.

**Why this priority**: Production clusters require explicit resource ownership, non-privileged workloads, secret injection, placement controls, and observability integration.

**Independent Test**: Deploy with restrictive workload security, custom resources and scheduling, external telemetry, and an optional network policy; verify Vizier operates without cluster-wide permissions or host access.

**Acceptance Scenarios**:

1. **Given** a cluster enforcing restricted workload security, **when** Vizier is installed, **then** it runs without privileged mode, host namespaces, host paths, added Linux capabilities, or write access outside declared writable locations.
2. **Given** operator-selected resource and placement settings, **when** Vizier is scheduled, **then** those settings are applied without modifying the published deployment package.
3. **Given** an existing telemetry destination, **when** its endpoint and attributes are configured, **then** Vizier exports its existing telemetry without requiring an in-cluster observability stack.
4. **Given** a private image registry, custom certificate authority, or image-pull secret, **when** those settings are supplied, **then** the same deployment package can use them.

---

### User Story 5 - Diagnose and Recover the Deployment (Priority: P3)

An operator has concise documentation and status signals for common installation failures, backup boundaries, pod replacement, storage migration, and disaster recovery.

**Why this priority**: Kubernetes packaging without recovery guidance moves configuration into the cluster but does not make the deployment supportable.

**Independent Test**: Follow the documentation from a clean cluster through installation, backup, pod loss, restore, and the documented failure cases without consulting repository source code.

**Acceptance Scenarios**:

1. **Given** missing or invalid required configuration, **when** installation is validated, **then** it fails before creating a misleading ready workload and names the missing setting without exposing values.
2. **Given** a running installation, **when** the documented backup procedure is followed, **then** it identifies every state component required for recovery, including the encryption key.
3. **Given** a lost pod but intact persistent services, **when** the workload is recreated, **then** Vizier returns to service without manual database repair.
4. **Given** a failed startup, **when** an operator follows the troubleshooting guide, **then** probe state and logs distinguish configuration, dependency, migration, and process failures.

### Edge Cases

- A persistent volume supports only single-node attachment and the replacement is scheduled on another node.
- The cluster terminates a pod before the requested drain period expires.
- Database migrations take longer than ordinary readiness delays.
- PostgreSQL is reachable but lacks the vector extension, or the object bucket is reachable with insufficient permissions.
- The encryption key secret is missing, malformed, rotated incorrectly, or does not match protected data.
- An external route or proxy does not preserve WebSocket upgrades or long-lived connections.
- The container registry is private, unavailable, or serves only one processor architecture.
- A deployment upgrade changes configuration but not the application image, or changes the image but not configuration.
- An operator changes from embedded to managed storage without first running the supported offline migration.
- A node runs out of ephemeral space while temporary files or model downloads are in use.
- Cluster policy injects a service-account token or grants permissions Vizier does not need.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The project MUST publish a versioned, first-party Kubernetes deployment package for every supported Vizier release.
- **FR-002**: The deployment package MUST use an immutable Vizier application version and MUST support the processor architectures published by the official container image.
- **FR-003**: The deployment package MUST install into an operator-selected namespace without requiring cluster-wide permissions.
- **FR-004**: The deployment MUST run exactly one Vizier application replica and MUST reject configuration requesting more than one replica until multi-instance operation is supported.
- **FR-005**: The deployment MUST provide an embedded profile using persistent storage for the workspace, database, files, and generated encryption key.
- **FR-006**: The deployment MUST provide a managed-storage profile using operator-supplied PostgreSQL and S3-compatible services.
- **FR-007**: The managed-storage profile MUST preserve the encryption key independently of pod lifecycle and MUST NOT rely on the pod filesystem for durable state.
- **FR-008**: The deployment package MUST NOT install PostgreSQL, an object store, an ingress controller, a certificate issuer, or an observability backend; it MUST integrate with operator-supplied services.
- **FR-009**: Sensitive values MUST be supplied through references to Kubernetes Secrets or equivalent external-secret projections and MUST NOT appear in non-secret configuration, process arguments, probe responses, events, or generated documentation output.
- **FR-010**: The deployment MUST support separate references for the JWT secret, encryption key, database credentials, object-store credentials, provider credentials, and channel credentials without requiring all values in one Secret.
- **FR-011**: The deployment MUST expose Vizier through an internal cluster service by default and MUST make external routing explicitly optional.
- **FR-012**: External routing configuration MUST support TLS termination and WebSocket traffic without assuming a specific ingress or gateway implementation.
- **FR-013**: Vizier MUST expose unauthenticated startup, readiness, and liveness signals intended only for workload management.
- **FR-014**: Probe responses MUST reveal no user, agent, provider, workspace, credential, database, bucket, or internal error details.
- **FR-015**: Readiness MUST remain false until storage validation, required migrations, and all mandatory serving subsystems are ready to accept requests.
- **FR-016**: Readiness MUST become false when graceful termination begins and before new requests stop being accepted.
- **FR-017**: Liveness MUST not fail solely because an agent, channel connection, provider, database dependency, or object-store dependency is temporarily unavailable after startup.
- **FR-018**: Startup failure caused by invalid configuration, incompatible schema, missing storage capability, or invalid credentials MUST be reported with an actionable and credential-safe reason.
- **FR-019**: On termination, Vizier MUST stop accepting new work, allow accepted work to complete for a configurable bounded period, consistently stop remaining work at the deadline, release its workspace lock, and exit.
- **FR-020**: Deployment updates MUST prevent overlap between old and new Vizier application instances.
- **FR-021**: The deployment package MUST preserve persistent state and the encryption key across pod replacement, rescheduling, configuration updates, and supported application upgrades.
- **FR-022**: The deployment package MUST provide configurable CPU, memory, ephemeral-storage, and persistent-storage requests and limits with documented production-oriented defaults.
- **FR-023**: The deployment package MUST support operator-provided node selection, affinity, tolerations, topology constraints, priority class, annotations, labels, and image-pull secrets.
- **FR-024**: The workload MUST run without privileged mode, host networking, host process namespaces, host paths, added Linux capabilities, or a requirement for cluster API access.
- **FR-025**: The workload MUST run as a non-root user and MUST limit filesystem writes to explicitly declared workspace and temporary locations.
- **FR-026**: Automatic service-account credential mounting MUST be disabled by default.
- **FR-027**: Operators MUST be able to supply trusted certificate authorities for PostgreSQL, S3-compatible storage, model providers, and other outbound HTTPS services without rebuilding the application image.
- **FR-028**: The deployment package MUST expose the existing telemetry configuration and MUST NOT require deployment of an observability backend.
- **FR-029**: The deployment package MUST provide checks that reject contradictory or incomplete storage, persistence, routing, secret, and replica settings before rollout where the platform permits validation.
- **FR-030**: The project MUST document installation, configuration, external exposure, upgrades, pre-write rollback, backups, restore, storage migration, secret rotation, troubleshooting, and removal.
- **FR-031**: Removal instructions MUST distinguish application resources from durable state and MUST require an explicit operator action before deleting persistent data.
- **FR-032**: The supported deployment MUST be continuously validated by installing it on a clean conformant cluster, observing readiness, exercising an authenticated HTTP and WebSocket interaction, replacing the pod, and confirming persistent state.
- **FR-033**: The deployment documentation MUST state that changing from embedded to managed storage requires the supported stopped-workspace migration and is not a live configuration switch.
- **FR-034**: The deployment documentation MUST state the supported cluster versions, application-version compatibility, storage prerequisites, network requirements, and single-instance limitation.

### Key Entities

- **Deployment Release**: A versioned set of cluster resources and defaults associated with one supported Vizier application release.
- **Installation**: One named Vizier deployment in one namespace, with exactly one application instance and one selected storage profile.
- **Storage Profile**: Either embedded persistent storage or operator-supplied PostgreSQL plus S3-compatible object storage.
- **Secret Reference**: A reference to operator-controlled secret material, including its source object and key, without copying the value into ordinary configuration.
- **Persistent State Set**: The database, file objects, workspace data, and encryption key that together are required to recover an installation.
- **Workload Health Signal**: A minimal startup, readiness, or liveness result used by the cluster controller rather than by people or agents.
- **External Route**: Optional operator-controlled exposure of the internal service, including host, TLS, and WebSocket behavior.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An operator with a conformant cluster and documented prerequisites can install Vizier and reach first-user setup in under 10 minutes, excluding image-download time and external-service provisioning.
- **SC-002**: Both embedded and managed-storage installations become ready within 5 minutes when their dependencies are healthy and no data migration is pending.
- **SC-003**: A pod replacement preserves 100% of previously committed users, agents, conversations, files, memory documents, configuration, and decryptable protected values in both storage profiles.
- **SC-004**: During every tested upgrade, no observation finds more than one Vizier application process using the installation's persistent state.
- **SC-005**: On graceful termination, readiness fails within 5 seconds and the process exits within the configured grace period while preserving all work it reported as completed.
- **SC-006**: Missing secrets, unreachable dependencies, incompatible schemas, and insufficient object-store permissions produce a distinct actionable failure within 2 minutes and expose no credential values.
- **SC-007**: The deployment passes restricted workload-security admission without exceptions for privilege, host access, added capabilities, root execution, or cluster API credentials.
- **SC-008**: The supported installation and lifecycle test passes on both published Linux processor architectures and on the oldest and newest documented supported cluster versions.
- **SC-009**: An authenticated HTTP request and a WebSocket conversation succeed through the supported internal service and through the documented external-route example.
- **SC-010**: A new operator can complete install, upgrade, backup, pod-loss recovery, and removal exercises using only published documentation, with no undocumented repository commands.
- **SC-011**: Static validation reports zero plaintext credential values in non-secret rendered resources and zero deployment resources requiring cluster-wide authorization.
- **SC-012**: Every release of the deployment package is traceable to exactly one Vizier application version and can be selected without using a mutable image tag.

## Assumptions

- Kubernetes support targets server deployments; the desktop application remains outside this feature.
- Vizier remains single-instance until Fleet adds a supported multi-process topology.
- The embedded profile is intended for evaluation and small single-instance deployments; production operators are encouraged to use managed PostgreSQL and S3-compatible storage.
- PostgreSQL, S3-compatible storage, DNS, TLS issuance, external routing, secret synchronization, and observability backends are supplied and operated separately.
- The official container image remains available for Linux AMD64 and ARM64.
- Existing Vizier authentication, authorization, encryption, storage migration, and telemetry contracts remain authoritative.
- The cluster provides standard persistent volumes for the embedded profile; storage-class-specific backup and replication are operator responsibilities.
- Live switching between storage profiles, horizontal scaling, autoscaling, active-active operation, bundled stateful dependencies, and automatic disaster recovery are outside this feature.
