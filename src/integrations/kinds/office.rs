//! The built-in `office` addon: read and write Word and Excel files (spec `034-office-addon`). It has no
//! settings; its tools are present when the integration is, and listed under the `office`
//! category.

use serde_json::Value;

use crate::{
    agents::tools::{
        VizierToolSet,
        office::{
            OfficeDocxRead, OfficeDocxWrite, OfficeXlsxRead, OfficeXlsxReadCsv, OfficeXlsxWrite,
        },
    },
    integrations::kind::{AgentToolContext, Contribution, IntegrationKind, KindListing},
    schema::{Integration, IntegrationFamily},
};

pub const ID: &str = "office";
/// The name of the integration seeded on every agent.
pub const DEFAULT_NAME: &str = "Office";

pub struct OfficeKind;

impl IntegrationKind for OfficeKind {
    fn id(&self) -> &'static str {
        ID
    }

    fn family(&self) -> IntegrationFamily {
        IntegrationFamily::Addon
    }

    fn listing(&self) -> KindListing {
        KindListing {
            name: "Office",
            description: "Read and create Word and Excel files: Markdown, typed cells and CSV previews",
            icon: "office",
            fields: vec![],
        }
    }

    /// No settings: `{}` and null are both fine.
    fn validate(&self, _name: &str, settings: &Value) -> Result<(), String> {
        match settings {
            Value::Null => Ok(()),
            Value::Object(map) if map.is_empty() => Ok(()),
            _ => Err("the Office addon has no settings".into()),
        }
    }

    fn contribute(&self, _integration: &Integration, ctx: &AgentToolContext) -> Contribution {
        let (storage, file_manager) = (ctx.storage.clone(), ctx.file_manager.clone());
        Contribution {
            native: VizierToolSet::new()
                .tool(OfficeDocxRead { storage: storage.clone(), file_manager: file_manager.clone() })
                .tool(OfficeXlsxRead { storage: storage.clone(), file_manager: file_manager.clone() })
                .tool(OfficeXlsxReadCsv { storage: storage.clone(), file_manager: file_manager.clone() })
                .tool(OfficeDocxWrite { storage: storage.clone(), file_manager: file_manager.clone() })
                .tool(OfficeXlsxWrite { storage, file_manager }),
            mcp: None,
            addon: Some(self.id()),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn it_has_no_settings() {
        assert!(OfficeKind.listing().fields.is_empty());
        assert!(OfficeKind.missing(&json!({})).is_empty());
        assert!(OfficeKind.missing(&Value::Null).is_empty());
        assert!(OfficeKind.validate("office", &json!({})).is_ok());
        assert!(OfficeKind.validate("office", &Value::Null).is_ok());
        assert!(OfficeKind.validate("office", &json!({"x": 1})).is_err());
    }
}
