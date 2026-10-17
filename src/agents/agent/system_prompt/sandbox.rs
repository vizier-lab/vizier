use crate::agents::tools::CategorySummary;

/// The `SANDBOX.md` briefing: how tools are reached, and the agent's tool categories.
/// Built only from configuration, so it is byte-identical across turns.
pub fn sandbox_md(categories: &[CategorySummary], tools_timeout: &str) -> String {
    let categories = categories
        .iter()
        .map(|category| format!("- {}: {}", category.name, category.description))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"# SANDBOX.md - Tools

You work through scripts: your tools are not offered to you directly. Each tool is a plain
Python function you call inside an `execute_python` script, with keyword arguments, and it
returns plain data. Only `think`, `execute_python`, `list_tool_functions` and
`describe_tool_function` are called directly.

## Tool categories

{categories}

## Finding and calling a tool

1. Pick the category that fits what you need from the list above.
2. `list_tool_functions(category="…")` lists its functions. Leave `category` out to list
   every function with its category, if you are unsure which one fits.
3. `describe_tool_function(name="…")` shows a function's parameters, return shape and an
   example.
4. Call it inside an `execute_python` script.

You can skip steps you don't need: call a function you already know directly from a script.
Inside a script the same lookups are `list_tools(category=…)` and `describe_tool("name")`.

- Prefer ONE script that loops, filters and aggregates over many small round-trips; only the
  script's final `result` (and `stdout`) enters your context.
- A tool error is raised inside the script as `RuntimeError("<tool>: ...")`. Catch it if partial
  results are acceptable; uncaught, it ends the run and you get the traceback.
- The whole script, including every tool call it makes, must finish within {tools_timeout}.
- Return only what you need. Everything you return or print is delivered to you verbatim."#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn category(name: &str, description: &str) -> CategorySummary {
        CategorySummary {
            name: name.into(),
            description: description.into(),
        }
    }

    #[test]
    fn lists_categories_in_the_given_order_without_counts() {
        let categories = [
            category("memory", "Search, read, write and organise long-term memory"),
            category("identity", "Read and update your CORE identity document"),
            category("github", "Tools from the github MCP server"),
        ];
        let md = sandbox_md(&categories, "30s");

        assert!(md.contains(
            "## Tool categories\n\n\
             - memory: Search, read, write and organise long-term memory\n\
             - identity: Read and update your CORE identity document\n\
             - github: Tools from the github MCP server\n\n"
        ));
        assert!(md.contains("must finish within 30s"));
        assert_eq!(md, sandbox_md(&categories, "30s"));
    }

    /// The client's categories as `turn_categories` gives them for one declaration.
    fn with_client(base: &[CategorySummary]) -> Vec<CategorySummary> {
        use crate::agents::client::{
            ClientBinding, ClientCategoryDeclaration, ClientConnection, ClientToolDeclaration, ClientTools,
        };
        let tool = |name: &str, category: &str| ClientToolDeclaration {
            name: name.into(),
            category: Some(category.into()),
            description: "d".into(),
            input_schema: serde_json::json!({ "type": "object" }),
            output_schema: None,
            timeout_secs: None,
            deferrable: false,
        };
        let (connection, _rx) = ClientConnection::for_tests();
        let binding = ClientBinding {
            connection,
            tools: vec![tool("render_interactive_ui", "ui"), tool("pick_color", "palette"), tool("x", "memory")],
            categories: vec![ClientCategoryDeclaration {
                name: "ui".into(),
                description: Some("Show interactive pages in the conversation".into()),
            }],
        };
        let client = ClientTools::resolve(
            &binding,
            &[],
            &[],
            crate::schema::VizierSession("a".into(), crate::schema::VizierChannelId::System, None),
            crate::agents::client::test_storage(),
            crate::transport::VizierTransport::new(),
        );
        let mut categories = base.to_vec();
        categories.extend(client.categories().iter().cloned());
        categories
    }

    /// Client categories follow the agent's own, labelled, in declaration order
    /// (contracts/agent-surface.md), and the same declaration gives the same prompt (SC-007).
    #[test]
    fn client_categories_are_listed_after_the_agents_and_labelled() {
        let base = [category("memory", "Search, read, write and organise long-term memory")];
        let md = sandbox_md(&with_client(&base), "30s");
        assert!(md.contains(
            "- memory: Search, read, write and organise long-term memory\n\
             - ui: (provided by the person's client) Show interactive pages in the conversation\n\
             - palette: (provided by the person's client) Tools provided by the person's client\n\
             - memory_client: (provided by the person's client) Tools provided by the person's client\n\n"
        ), "{md}");
        assert_eq!(md, sandbox_md(&with_client(&base), "30s"));
        // Without a client the prompt is what it was.
        assert_eq!(sandbox_md(&base, "30s"), sandbox_md(&base.to_vec(), "30s"));
        assert!(!sandbox_md(&base, "30s").contains("provided by the person's client"));
    }
}
