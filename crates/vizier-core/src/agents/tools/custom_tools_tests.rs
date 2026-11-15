//! End to end through the real bridge: a script saves a tool, a later script calls it
//! (`specs/061-agent-custom-tools` stories 1–3 and 6, SC-001).

use std::{sync::Arc, time::Duration};

use serde_json::json;

use super::*;
use crate::{
    agents::tools::{ToolRouter, VizierToolSet, python::run_script},
    sandbox::{ExecutionReport, SandboxLimits},
    schema::{VizierChannelId, VizierSession},
    storage::{VizierStorage, document::BlobDocumentStore, sqlite::SqlStorage},
};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct AddOneInput {
    n: i64,
}

struct AddOne;

#[async_trait::async_trait]
impl VizierTool for AddOne {
    type Input = AddOneInput;
    type Output = i64;
    fn name() -> String {
        "add_one".into()
    }
    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Time)
    }
    fn description(&self) -> String {
        "Add one.".into()
    }
    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        Ok(args.n + 1)
    }
}

/// Offered only in a turn somebody can answer, like `ask_question`.
struct AskSomebody;

#[async_trait::async_trait]
impl VizierTool for AskSomebody {
    type Input = serde_json::Value;
    type Output = String;
    fn name() -> String {
        "ask_somebody".into()
    }
    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Questions)
    }
    fn available(&self, ctx: &ToolContext) -> bool {
        ctx.interactive
    }
    fn description(&self) -> String {
        "Ask.".into()
    }
    async fn call(&self, _args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        Ok("answered".into())
    }
}

struct Fixture {
    router: ToolRouter,
    ctx: ToolContext,
    _dir: tempfile::TempDir,
}

fn fixture(interactive: bool) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::tests_support::test_store(|| {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_custom_tool_schema(&conn).unwrap();
        crate::storage::sql::SqlStore::sqlite(conn)
    });
    let storage = Arc::new(VizierStorage::new(SqlStorage::new(
        store,
        Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
    )));
    let service = CustomTools::new(storage, None);
    let router = ToolRouter {
        default_toolset: VizierToolSet::new()
            .tool(AddOne)
            .tool(AskSomebody)
            .tool(CreateCustomTool::new(service.clone()))
            .tool(UpdateCustomTool::new(service.clone()))
            .tool(ListCustomTools::new(service.clone())),
        ..ToolRouter::empty()
    };
    let ctx = ToolContext {
        session: VizierSession(
            "agent-1".into(),
            VizierChannelId::HTTP("alice".into(), "webui".into()),
            Some("t".into()),
        ),
        pending_attachments: Default::default(),
        hooks: None,
        background_depth: 0,
        subagent_depth: 0,
        loaded_images: Default::default(),
        deadline: None,
        client: None,
        questions: crate::questions::QuestionsHandle::new(Default::default()),
        interactive,
        secret_redactor: Arc::new(crate::secrets::SecretRedactor::default()),
        dream_sources: vec![],
        project: None,
        detached: false,
        parent_detached_id: None,
        custom: crate::custom_tools::CustomTurn { tools: Some(service), ..Default::default() },
    };
    Fixture { router, ctx, _dir: dir }
}

async fn run(f: &Fixture, code: &str, timeout: Duration) -> ExecutionReport {
    let limits = SandboxLimits { timeout, tools_enabled: true };
    run_script(&f.router, code, limits, timeout, &f.ctx, None, None).await.0
}

async fn script(f: &Fixture, code: &str) -> ExecutionReport {
    run(f, code, Duration::from_secs(10)).await
}

/// A `create_custom_tool(...)` call as a script line.
fn create(name: &str, schema: serde_json::Value, body: &str, extra: &str) -> String {
    let schema = schema.to_string().replace("true", "True").replace("false", "False");
    format!("create_custom_tool(name={name:?}, description='test tool', input_schema={schema}, script={body:?}{extra})")
}

#[tokio::test]
async fn a_saved_tool_runs_with_its_sample_and_is_callable_from_the_next_script() {
    let f = fixture(true);
    let schema = json!({"type": "object", "properties": {"n": {"type": "integer"}}, "required": ["n"]});
    let report =
        script(&f, &create("double_next", schema, "add_one(n=input['n']) * 2", ", sample_input={'n': 3}")).await;
    assert!(report.ok, "{report:?}");
    assert_eq!(report.result["status"], "created", "{}", report.result);
    assert_eq!(report.result["sample_run"]["result"], 8);
    assert_eq!(report.result["sample_run"]["took_effect"], true);

    let report = script(&f, "double_next(n=1)").await;
    assert!(report.ok, "{report:?}");
    assert_eq!(report.result, json!(4));
    let call = &report.tool_calls[0];
    assert_eq!(call.name, "double_next");
    assert_eq!(call.calls.len(), 1);
    assert_eq!(call.calls[0].name, "add_one");
}

#[tokio::test]
async fn bad_input_and_a_failing_step_come_back_as_errors_naming_the_tool() {
    let f = fixture(true);
    let schema = json!({"type": "object", "properties": {"n": {"type": "integer"}}, "required": ["n"]});
    assert!(script(&f, &create("plus", schema, "add_one(n=input['n'])", "")).await.ok);

    let report = script(&f, "plus(n='2')").await;
    assert!(!report.ok);
    let message = report.error.unwrap().message;
    assert!(message.contains("plus: invalid input: $.n: expected integer, got string"), "{message}");
    assert!(report.tool_calls[0].calls.is_empty(), "nothing ran");

    let fails = json!({"type": "object"});
    assert!(script(&f, &create("fails", fails, "add_one(n=1)\nadd_one(n='x')", "")).await.ok);
    let report = script(&f, "fails()").await;
    let message = report.error.unwrap().message;
    assert!(message.contains("fails failed at step 2 (add_one)"), "{message}");
    assert!(message.contains("1 add_one ok"), "{message}");
}

#[tokio::test]
async fn an_inner_call_gets_the_answer_a_direct_call_gets_in_this_turn() {
    let f = fixture(false);
    let report = script(&f, &create("ask", json!({"type": "object"}), "ask_somebody()", "")).await;
    assert_eq!(report.result["status"], "created", "{}", report.result);

    let direct = script(&f, "ask_somebody()").await;
    let wrapped = script(&f, "ask()").await;
    assert!(!direct.ok && !wrapped.ok);
    let direct = direct.error.unwrap().message;
    let wrapped = wrapped.error.unwrap().message;
    assert!(wrapped.contains(&direct), "direct: {direct}\nwrapped: {wrapped}");
}

#[tokio::test]
async fn a_tool_that_never_ends_is_stopped_by_the_callers_timeout() {
    let f = fixture(true);
    let created = script(&f, &create("slow", json!({"type": "object"}), "while True:\n  pass", "")).await;
    assert_eq!(created.result["status"], "created", "{}", created.result);
    let report = run(&f, "slow()", Duration::from_secs(2)).await;
    assert!(!report.ok);
    assert_eq!(report.tool_calls[0].name, "slow");
    assert!(!report.tool_calls[0].ok);
}

/// A script can only reach a tool by calling it, so the save check sees every custom call; the
/// call-time chain is the backstop for anything it cannot (a chain already running the tool).
#[tokio::test]
async fn a_tool_already_on_the_call_chain_is_refused_when_called() {
    let mut f = fixture(true);
    let object = json!({"type": "object"});
    assert!(script(&f, &create("a", object.clone(), "1", "")).await.ok);
    let report = script(&f, &create("b", object, "a()", "")).await;
    assert_eq!(report.result["status"], "created", "{}", report.result);
    f.ctx.custom.chain = vec!["a".into()];
    let report = script(&f, "b()").await;
    let message = report.error.unwrap().message;
    assert!(message.contains("a: cycle a → b → a"), "{message}");
}

/// `SANDBOX.md` changes only when a category gains its first enabled tool or loses its last
/// (SC-007, SC-010): a save into an existing category leaves the lines as they were, and a
/// switched-off tool is in no listing.
#[tokio::test]
async fn the_prompt_lines_change_only_with_a_category() {
    use crate::storage::custom_tool::{CustomToolActor, CustomToolStorage};
    let f = fixture(true);
    let service = f.ctx.custom.tools.clone().unwrap();
    assert!(service.turn_categories("agent-1", &f.router).await.is_empty(), "no tools, no lines");

    let in_maths = |name: &str| {
        format!(
            "create_custom_tool(name='{name}', description='x', input_schema={{'type':'object'}}, script='1',              category='maths', category_description='Arithmetic')"
        )
    };
    assert!(script(&f, &in_maths("a")).await.ok);
    let first = service.turn_categories("agent-1", &f.router).await;
    assert_eq!(first.len(), 1);
    assert!(script(&f, &in_maths("b")).await.ok);
    assert_eq!(service.turn_categories("agent-1", &f.router).await, first, "same category, same lines");
    assert!(script(&f, &create("c", json!({"type": "object"}), "1", "")).await.ok);
    let both = service.turn_categories("agent-1", &f.router).await;
    let names: Vec<&str> = both.iter().map(|line| line.name.as_str()).collect();
    assert_eq!(names, vec!["maths", "custom"], "one line more, the default last");

    // The snapshot is what the turn shows, after every other line.
    let mut ctx = f.ctx.clone();
    ctx.custom.categories = Arc::new(both.clone());
    let base = vec![crate::agents::tools::CategorySummary { name: "time".into(), description: "t".into() }];
    let lines = crate::agents::tools::turn_categories(&base, &f.router, &ctx);
    assert_eq!(&lines[lines.len() - 2..], &both[..]);

    // Switched off: gone from the entries the scripts see.
    let tool = service.storage().get_custom_tool("agent-1", "c").await.unwrap().unwrap();
    service.storage().set_custom_tool_enabled("agent-1", &tool.id, false, &CustomToolActor::Agent).await.unwrap();
    let entries = crate::agents::tools::turn_catalogue(&f.router, &f.ctx).await;
    assert!(entries.iter().any(|entry| entry.name == "a"));
    assert!(!entries.iter().any(|entry| entry.name == "c"));
    let names: Vec<String> =
        service.turn_categories("agent-1", &f.router).await.into_iter().map(|line| line.name).collect();
    assert_eq!(names, vec!["maths".to_string()]);
}

/// A dream saves directly (no script): checked against the full toolset, never sample-run, and
/// authored by the cycle (quickstart §8 mechanics).
#[tokio::test]
async fn a_dream_save_is_authored_by_the_cycle_and_never_sample_run() {
    use crate::agents::tools::VizierToolDyn;
    let mut f = fixture(false);
    f.ctx.custom.dream_cycle = Some("cycle-7".into());
    f.ctx.custom.script_router = Some(f.router.clone());
    let service = f.ctx.custom.tools.clone().unwrap();
    let tool = CreateCustomTool::new(service.clone());
    let args = json!({
        "name": "nightly",
        "description": "x",
        "input_schema": {"type": "object", "properties": {"n": {"type": "integer"}}},
        "script": "add_one(n=input['n'])",
        "sample_input": {"n": 1},
    });
    let out: serde_json::Value =
        serde_json::from_str(&tool.tool_call(args.to_string(), &f.ctx).await.unwrap()).unwrap();
    assert_eq!(out["status"], "created", "{out}");
    assert!(out.get("sample_run").is_none(), "a dream never sample-runs: {out}");
    let saved = service.get("agent-1", None, "nightly", None).await.unwrap();
    assert_eq!(saved["author"], "a dream (cycle-7)");

    // Checked against the full toolset: a typo is still refused.
    let typo = json!({"name": "typo", "description": "x", "input_schema": {"type": "object"}, "script": "add_onee(n=1)"});
    let out: serde_json::Value =
        serde_json::from_str(&tool.tool_call(typo.to_string(), &f.ctx).await.unwrap()).unwrap();
    assert_eq!(out["problems"][0]["code"], "unknown_tool", "{out}");
}

/// A tool attached after a custom tool was saved keeps its name: the custom tool is not offered
/// and reads as needing attention until it is renamed (FR-011).
#[tokio::test]
async fn a_tool_attached_later_wins_the_name_and_the_custom_tool_needs_attention() {
    use crate::storage::custom_tool::{
        CustomToolActor, CustomToolDefinition, CustomToolStorage, NewVersion, VersionTrigger,
    };
    let f = fixture(true);
    let service = f.ctx.custom.tools.clone().unwrap();
    // Saved before `add_one` existed: written straight to storage, as the save checks would
    // refuse it now.
    service
        .storage()
        .save_custom_tool_version(
            "agent-1",
            None,
            None,
            NewVersion {
                definition: CustomToolDefinition {
                    name: "add_one".into(),
                    category: "maths".into(),
                    description: "an older add_one".into(),
                    input_schema: json!({"type": "object"}),
                    output_description: None,
                    script: "99".into(),
                    calls: vec![],
                    sample_input: None,
                },
                trigger: VersionTrigger::Create,
                restored_from: None,
                reason: None,
            },
            &CustomToolActor::Agent,
            Some("Arithmetic"),
        )
        .await
        .unwrap();

    let entries = crate::agents::tools::turn_catalogue(&f.router, &f.ctx).await;
    let add_one: Vec<_> = entries.iter().filter(|entry| entry.name == "add_one").collect();
    assert_eq!(add_one.len(), 1, "one tool per name");
    assert_eq!(add_one[0].target, crate::agents::tools::ToolTarget::Native("add_one".into()), "the attached tool keeps it");
    assert_eq!(script(&f, "add_one(n=1)").await.result, json!(2));

    let names = crate::custom_tools::router_names(&f.router, &f.ctx).await;
    let listed = service.list("agent-1", Some(&names), None, true).await.unwrap();
    assert_eq!(listed["tools"][0]["needs_attention"]["name_taken"], true, "{listed}");
}
