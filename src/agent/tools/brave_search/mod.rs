use std::{marker::PhantomData, time::Duration};

use reqwest::StatusCode;
use rig::{completion::ToolDefinition, tool::Tool};
use schemars::schema_for;
use serde::{Deserialize, Serialize};

use crate::{
    config::BraveSearchConfig,
    error::{VizierError, throw_vizier_error},
};

mod request;
mod response;

pub trait SearchType {
    const NAME: &'static str;
    fn result_filter() -> String;
    fn description() -> String {
        "search the general informations on certain topic on the internet".into()
    }
}

pub struct WebOnlySearch;
impl SearchType for WebOnlySearch {
    const NAME: &'static str = "web_search";

    fn result_filter() -> String {
        "web".into()
    }
}

pub struct NewsOnlySearch;
impl SearchType for NewsOnlySearch {
    const NAME: &'static str = "news_search";

    fn result_filter() -> String {
        "news".into()
    }

    fn description() -> String {
        "find the latest news".into()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BraveSearch<T: SearchType> {
    _phantom: PhantomData<T>,
    api_key: String,
    safesearch: bool,
}

impl<T: SearchType> BraveSearch<T> {
    pub fn new(config: &BraveSearchConfig) -> Self {
        Self {
            _phantom: PhantomData,
            api_key: std::env::var("BRAVE_API_KEY").unwrap_or(config.api_key.clone().unwrap()),
            safesearch: config.safesearch,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct BraveSearchArgs {
    #[schemars(description = "Terms, keywords, or prompt to search")]
    pub query: String,
    #[schemars(description = "page of the search results, starts from 1")]
    pub page: u32,
}

const SEARCH_URL: &'static str = r"https://api.search.brave.com/res/v1/web/search";
const PAGE_SIZE: u32 = 10;

impl<T: SearchType> Tool for BraveSearch<T>
where
    Self: Send + Sync,
{
    const NAME: &'static str = T::NAME;
    type Error = VizierError;
    type Args = BraveSearchArgs;
    type Output = response::BraveResponse;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let parameters = serde_json::to_value(schema_for!(Self::Args)).unwrap();

        ToolDefinition {
            name: T::NAME.to_string(),
            description: format!(
                "{}, use intervals between the usage of these tools",
                T::description()
            ),
            parameters,
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let params = request::SearchParams {
            q: args.query,
            count: Some(PAGE_SIZE), // TODO: hardcoded for now
            offset: Some((args.page - 1) * PAGE_SIZE),
            safesearch: Some(if self.safesearch { "strict" } else { "off" }.to_string()),
            result_filter: Some(T::result_filter()),
        };

        let client = reqwest::Client::new();
        let response = client
            .get(format!("{SEARCH_URL}?{}", params.to_url()))
            .header("X-Subscription-Token", self.api_key.clone())
            .header("Content-Type", "application/json")
            .send()
            .await;

        if let Err(err) = response {
            return throw_vizier_error("brave_search: http error", err);
        }

        let response = response.unwrap();
        if response.status() != StatusCode::OK {
            return throw_vizier_error("status error:", response.error_for_status().err().unwrap());
        }

        let text = response.text().await;
        if let Err(err) = text {
            return throw_vizier_error("brave_search: text error:", err);
        }

        let text = text.unwrap();

        // throttle before return
        tokio::time::sleep(Duration::from_secs(1)).await;
        match serde_json::from_str(&text) {
            Ok(value) => Ok(value),
            Err(err) => throw_vizier_error("brave_search: parse error:", err),
        }
    }
}
