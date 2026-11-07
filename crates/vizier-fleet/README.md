# vizier-fleet

The Fleet roles (vizier-lab/vizier#159): the *same* engine, run as a cluster.

Each role is one piece of the server that can live on its own node — the HTTP gateway, the gRPC
gateway, the workers that run agents, the channel readers, and control. A role is built from the
bus traits, the storage traits and its own config, and never imports another role's code.

**The bus traits live in `vizier-schema`** (#162); the roles are still the skeleton
#161 lands: the role types, their constructors and the table of which sub-issue wires each one.
Nothing calls it yet, and `vizier run` refuses every role but `all`.

| Role | Sub-issue |
|---|---|
| `gateway-http` | #169 |
| `gateway-grpc` | #170 |
| `worker` | #168 |
| `channels` | #167 |
| `control` | #166 |

## Testing a role

Every role takes the bus first, so one can be built and tested alone with an in-process bus,
embedded storage and a scripted peer. The guide is the module documentation of
[`src/testing.rs`](src/testing.rs) (`cargo doc -p vizier-fleet --open`), and
[`tests/roles.rs`](tests/roles.rs) is a worked example.
