//! `ui_describe_elements` and `ui_examples`: the parts of the page guide an agent reads only when
//! it needs them (`specs/063-html-generative-ui`). The `ui_render` guide keeps the page model, the
//! rules, the look and the bridge, plus a one-line index of the elements and the examples; these
//! two return the details, so a page pays for the elements it uses and nothing else.
//!
//! Both read markdown that ships with the binary (`assets/ui-kit/ELEMENTS.md`, `EXAMPLES.md`),
//! split on its `### ` headings, and are offered exactly where `ui_render` is. A name that is not
//! there is data in the result (`unknown`, `did_you_mean`), never an error.

use std::sync::LazyLock;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool, ui_pages},
    error::VizierError,
    sandbox::docs::did_you_mean,
};

const ELEMENTS: &str = include_str!("../../../assets/ui-kit/ELEMENTS.md");
const EXAMPLES: &str = include_str!("../../../assets/ui-kit/EXAMPLES.md");

/// `(name, section)` for every `### \`<name>\`` or `### \`<vz-x>\`` heading, in file order.
fn sections(markdown: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in markdown.lines() {
        if let Some(rest) = line.strip_prefix("### `") {
            if let Some(done) = current.take() {
                out.push(done);
            }
            let name = rest.trim_end_matches('`').trim_start_matches('<').trim_end_matches('>').to_string();
            current = Some((name, String::new()));
            continue;
        }
        if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    out.extend(current);
    out.into_iter().map(|(name, body)| (name, body.trim().to_string())).collect()
}

static ELEMENT_SECTIONS: LazyLock<Vec<(String, String)>> = LazyLock::new(|| sections(ELEMENTS));
static EXAMPLE_SECTIONS: LazyLock<Vec<(String, String)>> = LazyLock::new(|| sections(EXAMPLES));

fn names(of: &[(String, String)]) -> Vec<String> {
    of.iter().map(|(n, _)| n.clone()).collect()
}

/// `card`, `<vz-card>` and `VZ-Card` all mean `vz-card`.
fn element_name(raw: &str) -> String {
    let name = raw.trim().trim_start_matches('<').trim_end_matches('>').to_lowercase();
    if name.starts_with("vz-") { name } else { format!("vz-{name}") }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DescribeElementsInput {
    /// The elements to describe, e.g. `["vz-stat", "vz-chart"]` (the `vz-` is optional).
    pub names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ElementDoc {
    pub name: String,
    /// Its attributes (a `*` marks a required one), children, events and an example, as markdown.
    pub doc: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DescribeElementsOutput {
    pub elements: Vec<ElementDoc>,
    /// Names that are not kit elements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unknown: Vec<String>,
    /// Close element names, when something was unknown.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub did_you_mean: Vec<String>,
    /// Every element of the kit, when something was unknown.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available: Vec<String>,
}

pub fn describe_elements(requested: &[String]) -> DescribeElementsOutput {
    let known = names(&ELEMENT_SECTIONS);
    let mut out = DescribeElementsOutput { elements: vec![], unknown: vec![], did_you_mean: vec![], available: vec![] };
    for raw in requested {
        let name = element_name(raw);
        if out.elements.iter().any(|e| e.name == name) {
            continue;
        }
        match ELEMENT_SECTIONS.iter().find(|(n, _)| *n == name) {
            Some((n, doc)) => out.elements.push(ElementDoc { name: n.clone(), doc: doc.clone() }),
            None => {
                for close in did_you_mean(&name, &known) {
                    if !out.did_you_mean.contains(&close) {
                        out.did_you_mean.push(close);
                    }
                }
                out.unknown.push(raw.clone());
            }
        }
    }
    if !out.unknown.is_empty() || requested.is_empty() {
        out.available = known;
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExamplesInput {
    /// Which example: `dashboard`, `form`, `game`, `worker` or `three`. Leave it out to list them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExampleSummary {
    pub kind: String,
    pub what: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExamplesOutput {
    /// The example asked for: a line on what it shows, then the whole page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
    /// Every example, when none (or an unknown one) was asked for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available: Vec<ExampleSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub did_you_mean: Vec<String>,
}

pub fn examples(kind: Option<&str>) -> ExamplesOutput {
    let list = || {
        EXAMPLE_SECTIONS
            .iter()
            .map(|(kind, body)| ExampleSummary { kind: kind.clone(), what: body.lines().next().unwrap_or_default().to_string() })
            .collect()
    };
    let Some(kind) = kind.map(|k| k.trim().to_lowercase()).filter(|k| !k.is_empty()) else {
        return ExamplesOutput { example: None, available: list(), did_you_mean: vec![] };
    };
    match EXAMPLE_SECTIONS.iter().find(|(k, _)| *k == kind) {
        Some((_, body)) => ExamplesOutput { example: Some(body.clone()), available: vec![], did_you_mean: vec![] },
        None => ExamplesOutput {
            example: None,
            available: list(),
            did_you_mean: did_you_mean(&kind, &names(&EXAMPLE_SECTIONS)),
        },
    }
}

pub struct UiDescribeElements;

#[async_trait::async_trait]
impl VizierTool for UiDescribeElements {
    type Input = DescribeElementsInput;
    type Output = DescribeElementsOutput;

    fn name() -> String {
        "ui_describe_elements".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Ui)
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        ui_pages::offered(ctx)
    }

    fn description(&self) -> String {
        "The attributes, events and an example of the <vz-*> elements you name, for a ui_render \
         page. Ask for every element the page uses in one call."
            .into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        Ok(describe_elements(&args.names))
    }
}

pub struct UiExamples;

#[async_trait::async_trait]
impl VizierTool for UiExamples {
    type Input = ExamplesInput;
    type Output = ExamplesOutput;

    fn name() -> String {
        "ui_examples".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Ui)
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        ui_pages::offered(ctx)
    }

    fn description(&self) -> String {
        "A whole example ui_render page by kind (dashboard, form, game, worker, three); with no \
         kind, the list."
            .into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        Ok(examples(args.kind.as_deref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_element_has_a_section_with_an_example() {
        let names = names(&ELEMENT_SECTIONS);
        assert_eq!(names.len(), 10, "{names:?}");
        for (name, doc) in ELEMENT_SECTIONS.iter() {
            assert!(name.starts_with("vz-"), "{name}");
            assert!(doc.contains(&format!("<{name}")), "{name} has an example");
        }
    }

    #[test]
    fn names_are_read_loosely_and_unknowns_suggested() {
        let out = describe_elements(&["stat".into(), "<vz-chart>".into(), "VZ-Stat".into(), "vz-chrat".into()]);
        assert_eq!(out.elements.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["vz-stat", "vz-chart"]);
        assert!(out.elements[0].doc.contains("| `label` * |"), "{}", out.elements[0].doc);
        assert_eq!(out.unknown, ["vz-chrat"]);
        assert!(out.did_you_mean.contains(&"vz-chart".to_string()));
        assert_eq!(out.available.len(), 10);

        let clean = describe_elements(&["vz-card".into()]);
        assert!(clean.unknown.is_empty() && clean.available.is_empty());
    }

    #[test]
    fn examples_list_and_fetch() {
        let list = examples(None);
        let kinds: Vec<_> = list.available.iter().map(|e| e.kind.as_str()).collect();
        assert_eq!(kinds, ["dashboard", "form", "game", "worker", "three"]);
        assert!(list.available.iter().all(|e| !e.what.is_empty() && !e.what.starts_with("```")));

        let game = examples(Some("Game")).example.unwrap();
        assert!(game.contains("requestAnimationFrame") && game.contains("```html"));

        let miss = examples(Some("gmae"));
        assert!(miss.example.is_none() && miss.did_you_mean.contains(&"game".to_string()));
    }

    /// The guide's index names every element and every example, so the agent knows what to ask for.
    #[test]
    fn the_guide_indexes_both() {
        let guide = <ui_pages::UiRender as VizierTool>::guide().unwrap();
        for name in names(&ELEMENT_SECTIONS) {
            assert!(guide.contains(&format!("| `{name}` |")), "{name}");
        }
        for kind in names(&EXAMPLE_SECTIONS) {
            assert!(guide.contains(&format!("`{kind}`")), "{kind}");
        }
        assert!(guide.contains("ui_describe_elements") && guide.contains("ui_examples"));
    }
}
