use std::collections::HashMap;

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::VizierTool,
    error::{VizierError, throw_vizier_error},
};

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct HttpClientArgs {
    #[schemars(description = "URL to send the request to")]
    pub url: String,
    #[schemars(description = "HTTP method (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS)")]
    pub method: String,
    #[schemars(description = "HTTP headers as key-value pairs")]
    pub headers: HashMap<String, String>,
    #[schemars(description = "Request body (optional, for POST/PUT/PATCH)")]
    pub body: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct HttpClientOutput {
    #[schemars(description = "HTTP status code")]
    pub status: u16,
    #[schemars(description = "Response body")]
    pub body: String,
    #[schemars(description = "Response headers as key-value pairs")]
    pub headers: HashMap<String, String>,
}

pub struct HttpClient;

#[async_trait::async_trait]
impl VizierTool for HttpClient {
    type Input = HttpClientArgs;
    type Output = HttpClientOutput;

    fn name() -> String {
        "http_client".to_string()
    }

    fn description(&self) -> String {
        "Make HTTP requests to interact with REST APIs. Construct your own headers and choose the appropriate HTTP method. Supports GET, POST, PUT, DELETE, PATCH, HEAD, and OPTIONS.".to_string()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let method = Method::from_bytes(args.method.to_uppercase().as_bytes())
            .map_err(|e| VizierError(format!("Invalid HTTP method: {}", e)))?;

        let client = reqwest::Client::new();
        let mut request = client.request(method, &args.url);

        for (key, value) in &args.headers {
            request = request.header(key, value);
        }

        if let Some(body) = args.body {
            request = request.body(body);
        }

        let response = request.send().await;

        if let Err(err) = response {
            return throw_vizier_error("http_client: request error", err);
        }
        let response = response.unwrap();

        let status = response.status().as_u16();
        let headers: HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let body = response.text().await;

        if let Err(err) = body {
            return throw_vizier_error("http_client: text error", err);
        }
        let body = body.unwrap();

        Ok(HttpClientOutput {
            status,
            body,
            headers,
        })
    }
}

