# Contract: Workload Health Endpoints

These endpoints are unauthenticated workload-management signals. They are not included under `/api`, not exposed in the application OpenAPI document, and contain no deployment diagnostics.

## `GET /health/startup`

- `200 OK` after storage initialization, required migrations, core runtime startup, and HTTP listener bind.
- `503 Service Unavailable` before startup completes.
- Body: `{"status":"ok"}` or `{"status":"unavailable"}`.

The Helm startup probe uses this endpoint. Liveness and readiness probes do not run until startup succeeds.

## `GET /health/ready`

- `200 OK` only while the process accepts new requests.
- `503 Service Unavailable` during startup and from the beginning of graceful shutdown.
- Body: `{"status":"ok"}` or `{"status":"unavailable"}`.

Temporary failure of an individual agent, channel, provider, or optional integration does not make the process unready. A mandatory initialization failure prevents the server from reaching the ready state.

## `GET /health/live`

- `200 OK` while the HTTP process can answer.
- Body: `{"status":"ok"}`.

The signal is intentionally independent of PostgreSQL, S3, model-provider, agent, and channel availability after startup. Process crash or event-loop failure is observed as connection failure/timeout by Kubernetes.

## Headers and disclosure

- `Content-Type: application/json`
- No authentication challenge.
- No cache-sensitive information.
- No user, agent, provider, workspace path, host identity, database, object bucket, credential, version, or internal error detail.

## Shutdown ordering

1. Set readiness false.
2. Stop accepting new HTTP connections.
3. Drain accepted work until the configured application deadline.
4. Stop runtime subsystems and release the workspace lock.
5. Exit before Kubernetes `terminationGracePeriodSeconds` expires.
