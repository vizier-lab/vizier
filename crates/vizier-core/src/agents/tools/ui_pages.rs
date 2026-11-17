//! `ui_render`: the one generative UI tool (`specs/063-html-generative-ui`,
//! contracts/ui-render-tool.md). It draws an HTML page in the reply, or the next version of one
//! made in this conversation, from a complete document or from exact edits to the latest version.
//!
//! A successful call ends the turn. Every refusal, and a version a tab found problems in, is an
//! `Err` carrying a JSON message: a turn-ending tool ends the turn only on `Ok`, so the turn goes
//! on and the agent can fix what it is told (research R3).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    schema::{SessionHistoryContent, VizierChannelId},
    storage::{VizierStorage, ui_page::UiPageStorage},
    transport::{SessionEvent, SessionFrame, VizierTransport},
    ui_pages::{
        Made, NewVersion, UiPages,
        edits::{Edit, EditError, apply_edits},
        limits::{MAX_EDITS, MAX_TITLE_CHARS, MAX_VERSION_BYTES},
        verify,
    },
};

/// The guide `describe_tool_function("ui_render")` returns (research R2).
const GUIDE: &str = include_str!("../../../assets/ui-kit/REFERENCE.md");

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UiRenderInput {
    /// The page's title, shown over it. Required for a new page; optional with `page`, to rename.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// A complete HTML document (or fragment): a new page, or with `page` a full replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    /// The id of a page you drew in this conversation (`pg_…`), to change it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// The version your change is based on: the page's latest version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_version: Option<u32>,
    /// Exact find/replace pairs applied in order to the latest version; each `find` must occur
    /// exactly once where it is applied. Prefer this to a full replacement for small changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits: Option<Vec<Edit>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UiRenderOutput {
    /// Always `rendered`.
    pub status: String,
    pub page: String,
    pub version: u32,
    /// Whether an open tab ran the version and found no problem.
    pub verified: bool,
    /// Why it was not checked: `no_tab`, `timeout` or `closed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// What the call asked for, once its arguments are checked.
#[derive(Debug, PartialEq)]
enum Shape {
    New { title: String, html: String },
    Replace { page: String, base: u32, html: String, title: Option<String> },
    Edits { page: String, base: u32, edits: Vec<Edit>, title: Option<String> },
}

/// A refusal as the tool's error: JSON the agent (or its script) can read.
fn refuse(code: &str, extra: Value) -> VizierError {
    let mut body = json!({ "code": code });
    if let (Some(body), Value::Object(extra)) = (body.as_object_mut(), extra) {
        body.extend(extra);
    }
    VizierError(body.to_string())
}

fn check_title(title: Option<String>, required: bool, problems: &mut Vec<String>) -> Option<String> {
    match title.map(|t| t.trim().to_string()) {
        Some(t) if t.is_empty() => {
            problems.push("title is empty".into());
            None
        }
        Some(t) if t.chars().count() > MAX_TITLE_CHARS => {
            problems.push(format!("title is longer than {MAX_TITLE_CHARS} characters"));
            None
        }
        Some(t) => Some(t),
        None => {
            if required {
                problems.push("a new page needs a title".into());
            }
            None
        }
    }
}

fn check_size(html: &str) -> Result<(), VizierError> {
    if html.len() > MAX_VERSION_BYTES {
        return Err(refuse("too_large", json!({ "size": html.len(), "limit": MAX_VERSION_BYTES })));
    }
    Ok(())
}

/// The call's shape, or every problem with it at once.
fn shape(input: UiRenderInput) -> Result<Shape, VizierError> {
    let mut problems = Vec::new();
    let html = input.html.filter(|h| {
        let empty = h.trim().is_empty();
        if empty {
            problems.push("html is empty".to_string());
        }
        !empty
    });
    let shape = match (input.page, input.base_version) {
        (None, base) => {
            if base.is_some() || input.edits.is_some() {
                problems.push("base_version and edits need page: to change a page, name it".into());
            }
            let title = check_title(input.title, true, &mut problems);
            if html.is_none() && problems.iter().all(|p| p != "html is empty") {
                problems.push("a new page needs html".into());
            }
            match (title, html) {
                (Some(title), Some(html)) if problems.is_empty() => Some(Shape::New { title, html }),
                _ => None,
            }
        }
        (Some(_), None) => {
            problems.push("changing a page needs base_version: the page's latest version".into());
            None
        }
        (Some(page), Some(base)) => {
            let title = check_title(input.title, false, &mut problems);
            match (html, input.edits) {
                (Some(_), Some(_)) => {
                    problems.push("send html (a full replacement) or edits, not both".into());
                    None
                }
                (Some(html), None) if problems.is_empty() => Some(Shape::Replace { page, base, html, title }),
                (None, Some(edits)) => {
                    if edits.is_empty() || edits.len() > MAX_EDITS {
                        problems.push(format!("edits must hold 1 to {MAX_EDITS} pairs"));
                    }
                    problems.is_empty().then_some(Shape::Edits { page, base, edits, title })
                }
                (None, None) => {
                    if problems.iter().all(|p| p != "html is empty") {
                        problems.push("changing a page needs html or edits".into());
                    }
                    None
                }
                _ => None,
            }
        }
    };
    match shape {
        Some(shape) if problems.is_empty() => Ok(shape),
        _ => Err(refuse("invalid_input", json!({ "problems": problems }))),
    }
}

fn edit_refusal(error: EditError, edits: &[Edit]) -> VizierError {
    match error {
        EditError::EmptyFind { index } => refuse("invalid_input", json!({ "problems": [format!("edit {index} has an empty find")] })),
        EditError::NotFound { index } => refuse(
            "edit_not_found",
            json!({ "edit": index, "find_preview": edits[index].find.chars().take(80).collect::<String>() }),
        ),
        EditError::Ambiguous { index, count } => refuse("edit_ambiguous", json!({ "edit": index, "count": count })),
    }
}

pub struct UiRender {
    pages: UiPages,
    storage: std::sync::Arc<VizierStorage>,
    transport: VizierTransport,
}

impl UiRender {
    pub fn new(pages: UiPages, storage: std::sync::Arc<VizierStorage>, transport: VizierTransport) -> Self {
        Self { pages, storage, transport }
    }

    /// Record the version's history entry, check it in a tab, and answer.
    async fn finish(&self, ctx: &ToolContext, page_id: &str, title: &str, version: u32) -> Result<UiRenderOutput, VizierError> {
        let entry = SessionHistoryContent::UiPage {
            page_id: page_id.to_string(),
            version,
            title: title.to_string(),
            verification: None,
        };
        if let Some(uid) = crate::agents::steps::record_step(&self.storage, &self.transport, &ctx.session, entry).await
            && let Err(e) = self.storage.set_ui_page_history_uid(page_id, version, &uid).await
        {
            tracing::warn!(page_id, version, "could not link a page version to its entry: {e}");
        }

        let verification = verify::verify(&self.transport, &ctx.session, page_id, version).await;
        if let Err(e) = self.storage.set_ui_page_verification(page_id, version, &verification).await {
            tracing::warn!(page_id, version, "could not save a page version's check: {e}");
        }
        self.transport.publish_session_event(SessionEvent {
            session: ctx.session.clone(),
            frame: SessionFrame::UiPageVerified {
                page_id: page_id.to_string(),
                version,
                verification: verification.clone(),
            },
        });

        match verification["state"].as_str() {
            Some("problems") => Err(refuse(
                "problems",
                json!({
                    "page": page_id,
                    "version": version,
                    "problems": verification["problems"],
                    "more": verification.get("more"),
                    "hint": "The version is saved. Fix it with edits based on this version.",
                }),
            )),
            Some("clean") => Ok(UiRenderOutput {
                status: "rendered".into(),
                page: page_id.to_string(),
                version,
                verified: true,
                reason: None,
            }),
            _ => Ok(UiRenderOutput {
                status: "rendered".into(),
                page: page_id.to_string(),
                version,
                verified: false,
                reason: verification["reason"].as_str().map(str::to_string),
            }),
        }
    }
}

/// Where a page can be shown: a WebUI or project conversation, not a detached script (R9). By
/// channel kind only, so the listing is the same all conversation, tab open or not (FR-032).
pub fn offered(ctx: &ToolContext) -> bool {
    offered_in(&ctx.session.1, ctx.detached)
}

/// The rule of [`offered`] (FR-002): a WebUI or project chat, whether or not a tab is open, and
/// never from a detached script (nobody's reply to put the page in).
pub fn offered_in(channel: &VizierChannelId, detached: bool) -> bool {
    matches!(channel, VizierChannelId::HTTP(..) | VizierChannelId::Project(..)) && !detached
}

#[async_trait::async_trait]
impl VizierTool for UiRender {
    type Input = UiRenderInput;
    type Output = UiRenderOutput;

    fn name() -> String {
        "ui_render".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Ui)
    }

    fn ends_turn() -> bool {
        true
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        offered(ctx)
    }

    fn guide() -> Option<&'static str> {
        Some(GUIDE)
    }

    fn description(&self) -> String {
        "Draw an interactive HTML page in your reply: anything a web page can be (apps, games, \
         simulations, dashboards, forms). Plain HTML already looks like Vizier, and <vz-*> elements \
         (cards, stats, tables, charts, form fields) and the three library are ready to use. Before your first page in a conversation, call \
         describe_tool_function(\"ui_render\") and read its guide. To change a page, send edits \
         against its latest version instead of rewriting it. A successful call is your final answer."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        if !offered(ctx) {
            return Err(refuse("not_available", json!({ "reason": "pages are shown only in WebUI conversations" })));
        }
        match shape(args)? {
            Shape::New { title, html } => {
                check_size(&html)?;
                let page = self.pages.create(&ctx.session, &title, &html).await.map_err(|e| VizierError(e.to_string()))?;
                self.finish(ctx, &page.id, &page.title, 1).await
            }
            Shape::Replace { page, base, html, title } => {
                check_size(&html)?;
                self.add(ctx, &page, base, title, |_| Ok((html.clone(), Made::Full))).await
            }
            Shape::Edits { page, base, edits, title } => {
                let count = edits.len() as u32;
                self.add(ctx, &page, base, title, |base_html| {
                    let html = apply_edits(base_html, &edits).map_err(|e| edit_refusal(e, &edits))?;
                    check_size(&html)?;
                    Ok((html, Made::Edits { count }))
                })
                .await
            }
        }
    }
}

impl UiRender {
    /// A new version of `page_id` from `base`, made by `make` from the base's HTML.
    async fn add(
        &self,
        ctx: &ToolContext,
        page_id: &str,
        base: u32,
        title: Option<String>,
        make: impl FnOnce(&str) -> Result<(String, Made), VizierError>,
    ) -> Result<UiRenderOutput, VizierError> {
        let page = self.storage.get_ui_page(page_id).await.map_err(|e| VizierError(e.to_string()))?;
        let Some(page) = page.filter(|p| p.agent_id == ctx.session.0) else {
            return Err(refuse("unknown_page", json!({ "page": page_id })));
        };
        if page.session != ctx.session {
            return Err(refuse("other_conversation", json!({ "page": page_id })));
        }
        if base != page.latest_version {
            return Err(refuse("stale_base", json!({ "page": page_id, "latest": page.latest_version })));
        }
        let base_html = self
            .pages
            .html(&page, base)
            .await
            .map_err(|e| VizierError(e.to_string()))?
            .ok_or_else(|| refuse("unknown_page", json!({ "page": page_id, "version": base })))?;
        let (html, made) = make(&base_html)?;
        match self.pages.add_version(&page, base, &html, made, title.as_deref()).await.map_err(|e| VizierError(e.to_string()))? {
            NewVersion::Added(version) => {
                let title = title.unwrap_or(page.title);
                self.finish(ctx, page_id, &title, version).await
            }
            NewVersion::Stale { latest } => Err(refuse("stale_base", json!({ "page": page_id, "latest": latest }))),
            NewVersion::NotFound => Err(refuse("unknown_page", json!({ "page": page_id }))),
        }
    }
}

#[cfg(test)]
#[path = "ui_pages_tests.rs"]
mod tests;
