//! `ui_render`'s argument rules (contracts/ui-render-tool.md) and where it is offered (R9).

use super::*;

fn input(v: Value) -> UiRenderInput {
    serde_json::from_value(v).unwrap()
}

fn code(err: VizierError) -> (String, Value) {
    let body: Value = serde_json::from_str(&err.0).unwrap();
    (body["code"].as_str().unwrap().to_string(), body)
}

#[test]
fn the_three_shapes_are_recognised() {
    assert_eq!(
        shape(input(json!({"title": " Chess ", "html": "<p>x</p>"}))).unwrap(),
        Shape::New { title: "Chess".into(), html: "<p>x</p>".into() }
    );
    assert!(matches!(
        shape(input(json!({"page": "pg_1", "base_version": 2, "html": "<p>y</p>"}))).unwrap(),
        Shape::Replace { base: 2, title: None, .. }
    ));
    assert!(matches!(
        shape(input(json!({"page": "pg_1", "base_version": 2, "edits": [{"find": "a", "replace": "b"}], "title": "New"}))).unwrap(),
        Shape::Edits { base: 2, title: Some(_), .. }
    ));
}

#[test]
fn every_problem_is_reported_at_once() {
    let (c, body) = code(shape(input(json!({"html": " ", "base_version": 1}))).unwrap_err());
    assert_eq!(c, "invalid_input");
    let problems: Vec<String> = serde_json::from_value(body["problems"].clone()).unwrap();
    assert!(problems.iter().any(|p| p.contains("html is empty")), "{problems:?}");
    assert!(problems.iter().any(|p| p.contains("needs a title")), "{problems:?}");
    assert!(problems.iter().any(|p| p.contains("need page")), "{problems:?}");
}

#[test]
fn a_change_needs_a_base_and_one_kind_of_change() {
    assert_eq!(code(shape(input(json!({"page": "pg_1", "html": "x"}))).unwrap_err()).0, "invalid_input");
    assert_eq!(
        code(shape(input(json!({"page": "pg_1", "base_version": 1, "html": "x", "edits": [{"find": "a", "replace": "b"}]}))).unwrap_err()).0,
        "invalid_input"
    );
    assert_eq!(code(shape(input(json!({"page": "pg_1", "base_version": 1, "edits": []}))).unwrap_err()).0, "invalid_input");
    assert_eq!(code(shape(input(json!({"page": "pg_1", "base_version": 1}))).unwrap_err()).0, "invalid_input");
    let long = "t".repeat(MAX_TITLE_CHARS + 1);
    assert_eq!(code(shape(input(json!({"title": long, "html": "x"}))).unwrap_err()).0, "invalid_input");
}

#[test]
fn unknown_arguments_are_refused_by_the_schema() {
    assert!(serde_json::from_value::<UiRenderInput>(json!({"title": "a", "html": "b", "spec": {}})).is_err());
}

#[test]
fn an_oversized_version_is_refused_with_its_size() {
    let (c, body) = code(check_size(&"x".repeat(MAX_VERSION_BYTES + 1)).unwrap_err());
    assert_eq!(c, "too_large");
    assert_eq!(body["limit"], MAX_VERSION_BYTES);
}

#[test]
fn edit_failures_name_the_edit() {
    let edits = vec![Edit { find: "zzz".into(), replace: "y".into() }];
    let (c, body) = code(edit_refusal(EditError::NotFound { index: 0 }, &edits));
    assert_eq!((c.as_str(), body["edit"].as_u64()), ("edit_not_found", Some(0)));
    let (c, body) = code(edit_refusal(EditError::Ambiguous { index: 0, count: 3 }, &edits));
    assert_eq!((c.as_str(), body["count"].as_u64()), ("edit_ambiguous", Some(3)));
}

#[test]
fn the_guide_is_offered_and_names_the_codes() {
    let guide = <UiRender as VizierTool>::guide().unwrap();
    for code in ["stale_base", "edit_not_found", "problems", "vizier.worker", "requestAnimationFrame"] {
        assert!(guide.contains(code), "{code}");
    }
}

/// Spec 063 US6 (T067): offered in WebUI and project chats only, and never detached.
#[test]
fn offered_only_where_a_page_can_be_shown() {
    use crate::schema::{DreamStage, VizierChannelId as C, VizierSession};
    let web = C::HTTP("ana".into(), "vizier-webui".into());
    let every = [
        (web.clone(), true),
        (C::Project("ana".into(), "p1".into()), true),
        (C::DiscordChanel(1), false),
        (C::TelegramChannel(1), false),
        (C::Workspace("w".into()), false),
        (C::Task("t".into(), crate::time::now()), false),
        (C::InterAgent(vec!["a".into(), "b".into()]), false),
        (C::System, false),
        (C::Subagent, false),
        (C::Dream(Box::new(VizierSession("a".into(), web.clone(), None)), DreamStage::Extraction), false),
    ];
    for (channel, want) in every {
        assert_eq!(offered_in(&channel, false), want, "{channel:?}");
        assert!(!offered_in(&channel, true), "never from a detached script: {channel:?}");
    }
}
