use serde::Serialize;

#[derive(Serialize, Debug, Default)]
pub struct SearchParams {
    /// The search query (Required)
    pub q: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub safesearch: Option<String>,

    /// values: web, images, news, videos
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_filter: Option<String>,
}

impl SearchParams {
    pub fn to_url(&self) -> String {
        serde_urlencoded::to_string(self).unwrap()
    }
}
