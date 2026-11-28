# Contract: TriggerKind

```rust
pub trait TriggerKind: Send + Sync {
    fn id(&self) -> &'static str;
    fn listing(&self) -> KindListing;
    fn validate(&self, settings: &Value) -> Result<(), String>;
    /// Default: mask the fields the listing marks secret.
    fn redact(&self, settings: &Value) -> Value;
    /// A pushed request, after the route found an active trigger of a public kind. The kind checks
    /// the secret, signature and timestamp itself. Default: reject (a pull kind).
    fn receive(&self, trigger: &Trigger, request: &PushRequest) -> Result<Intake, PushRefusal>;
    fn is_public(&self) -> bool;
    fn prepare(&self, settings: &mut Value) -> Option<String>;        // a new secret, shown once
    fn replace_secret(&self, settings: &mut Value) -> Option<String>;
    fn default_scope(&self) -> Option<Vec<String>>;
    /// Narrows what an agent wrote in the settings, so it never grants itself tool access
    /// (FR-031). Default: nothing.
    fn restrict_agent_settings(&self, settings: &mut Value, previous: Option<&Value>, turn_scope: Option<&[String]>);
    /// Whether a request is from the sender, judged without its body: only then is a body too
    /// large to read answered `413`. Default: no.
    fn authenticates_without_body(&self, trigger: &Trigger, request: &PushRequest) -> bool;
    /// What a full queue does with a new event. Default: drop the oldest waiting one.
    fn when_full(&self) -> WhenFull { WhenFull::DropOldest }
    // A generic `source()` was not needed for v1: the monitor's one shared source is started by
    // the scheduler. An addon pull kind would add it back.
}

pub enum Intake {
    Fire { payload: Value, delivery_id: Option<String> },
    Ignore { reason: String },
    Reject,
}

pub enum WhenFull { DropOldest, RefuseNew }
```

- `monitor` returns `RefuseNew`: `ctx.fire` then reports `refused`, and the source keeps its state.

- A kind never sends a request to an agent. It returns `Intake`, or calls `ctx.fire`.
- `receive` checks the secret first, then the signature and timestamp; size and type are refused only
  after that.
- The monitor's source (`triggers::monitor::run_source`) runs only while the `control` gate is open,
  re-reads triggers every 5 seconds, and never returns.
- Addons contribute through `IntegrationKind::trigger_kinds()`; ids are `<addon>_<name>`
  (`TriggerKinds::with_addons`).
- A kind from an addon is valid only for agents that have the addon.

A kind's listing draws its settings form, so no page branches on a kind id. Field types `number`,
`code` and `tool_categories` say how a value is edited; `generated` fields are filled in by the
server and never asked for, and `advanced` ones may be folded away. `GET /trigger-kinds` also gives
each kind's `default_scope`.
