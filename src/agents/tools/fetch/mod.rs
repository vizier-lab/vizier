use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::VizierTool,
    error::{throw_vizier_error, VizierError},
};

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct FetchArgs {
    #[schemars(description = "URL of the webpage to fetch")]
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct FetchOutput {
    #[schemars(description = "The webpage content converted to markdown")]
    pub content: String,
    #[schemars(description = "Title of the webpage if found")]
    pub title: Option<String>,
}

pub struct FetchWebpage;

#[async_trait::async_trait]
impl VizierTool for FetchWebpage {
    type Input = FetchArgs;
    type Output = FetchOutput;

    fn name() -> String {
        "fetch".to_string()
    }

    fn description(&self) -> String {
        "Fetch a webpage and convert its HTML content to markdown. Use this when you need to read content from a URL. Returns the markdown content and page title if available.".to_string()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let response = reqwest::get(&args.url).await;

        if let Err(err) = response {
            return throw_vizier_error("fetch: http error", err);
        }
        let response = response.unwrap();

        if !response.status().is_success() {
            return throw_vizier_error("fetch: status error", response.error_for_status().err().unwrap());
        }

        let html = response.text().await;

        if let Err(err) = html {
            return throw_vizier_error("fetch: text error", err);
        }
        let html = html.unwrap();

        let title = extract_title(&html);
        let markdown = html2md::parse_html(&html);

        Ok(FetchOutput {
            content: markdown,
            title,
        })
    }
}

fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    if let Some(start) = lower.find("<title") {
        if let Some(tag_end) = lower[start..].find('>') {
            let content_start = start + tag_end + 1;
            if let Some(end) = lower[content_start..].find("</title>") {
                let title = &html[content_start..content_start + end];
                let title = title.trim();
                if !title.is_empty() {
                    return Some(title.to_string());
                }
            }
        }
    }
    None
}