use std::{collections::HashMap, fmt::Display};

use rig::{completion::ToolDefinition, tool::Tool};
use schemars::schema_for;
use serde::{Deserialize, Serialize};

use crate::{agent::tools::python::PythonInterpreter, error::VizierError};

pub struct ToolDoc {
    name: String,
    description: String,
    input_schema: String,
    output_schema: String,
}

impl Display for ToolDoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"#{}
            {}

            ## Input Schema
            {}

            ## Output Schema
            {}

            "#,
            self.name, self.description, self.input_schema, self.output_schema
        )
    }
}

pub struct PythonToolsDocs {
    docs: HashMap<String, ToolDoc>,
}

impl PythonInterpreter {
    pub async fn generate_docs_tool(&self) -> PythonToolsDocs {
        let mut docs = HashMap::new();

        for tool in self.programmatic_tools.iter() {
            let definition = tool.get_definition().await;

            let doc = ToolDoc {
                name: definition.name.clone(),
                description: definition.description,
                input_schema: tool.describe_input(),
                output_schema: tool.describe_output(),
            };

            docs.insert(definition.name, doc);
        }

        PythonToolsDocs { docs }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PythonToolsDocsArgs {
    #[schemars(description = "Optional, if filled will only show documentation for the tool")]
    tool_name: Option<String>,
}

impl PythonToolsDocs {
    fn description(&self) -> String {
        format!(
            r#"use this tools to get documentation detail of available programmatic tool.
            Calls the underlying programmatic tool with given kwargs in python_interpreter (ie. `some_tool(arg=some_val)`).
            list of available tools: {}"#,
            self.docs
                .iter()
                .map(|t| t.0.clone())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

impl Tool for PythonToolsDocs {
    const NAME: &'static str = "python_tools_docs";

    type Error = VizierError;
    type Args = PythonToolsDocsArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let parameters = serde_json::to_value(schema_for!(Self::Args)).unwrap();

        ToolDefinition {
            name: Self::NAME.to_string(),
            description: self.description(),
            parameters,
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok(match args.tool_name {
            None => {
                let docs = self
                    .docs
                    .iter()
                    .map(|(_, doc)| doc.to_string())
                    .collect::<Vec<_>>()
                    .join("\n\n");

                docs
            }
            Some(tool) => {
                let doc = self.docs.get(&tool).map(|tool| tool.to_string());

                if doc.is_none() {
                    "".into()
                } else {
                    doc.unwrap()
                }
            }
        })
    }
}
