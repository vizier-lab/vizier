# Contract: Kubernetes Lifecycle

## Install

1. Helm validates values.
2. Kubernetes creates namespaced account/config/service/storage/workload resources.
3. The pod starts with no mounted Kubernetes API token by default.
4. Vizier validates configuration, opens storage, runs migrations, and starts mandatory subsystems.
5. HTTP binds; startup and readiness become successful.
6. The Service admits traffic.

A mandatory startup error leaves the pod not ready and writes an actionable, credential-safe log message.

## Termination

1. Kubernetes sends SIGTERM.
2. Vizier marks readiness unavailable before closing HTTP admission.
3. Axum drains accepted HTTP requests for `shutdown.gracePeriodSeconds`.
4. Vizier stops remaining runtime work; durable interrupted turns remain journaled for restart.
5. Runtime resources and the workspace lock are released.
6. Process exits before `terminationGracePeriodSeconds`; Kubernetes may SIGKILL only after that outer deadline.

## Upgrade

- Deployment strategy is `Recreate`.
- Old pod reaches termination before a new pod starts.
- Image defaults to the chart `appVersion`; digest pinning is supported.
- Storage and encryption-key references remain unchanged unless the operator deliberately migrates them.
- Database migrations may be forward-only. Rollback is supported only before the replacement accepts writes unless release notes explicitly document backward schema compatibility.

## Pod replacement

- Embedded: replacement remounts the existing PVC; single-attach volume constraints may delay scheduling but must not cause a second writer.
- Managed: replacement reconnects to PostgreSQL/S3 and receives the same encryption-key Secret; ephemeral `/data` and `/tmp` may be empty.

## Uninstall

- Helm removes application resources.
- A chart-created PVC is retained by default.
- External PostgreSQL, S3 data, and operator-created Secrets are never deleted by the chart.
- Permanent deletion requires a separate explicit operator action after backup verification.

## Unsupported transition

Changing `storage.profile` on a live installation is rejected/documented as unsupported. Use the stopped-workspace migration procedure, verify the destination, then deploy against the migrated state.
