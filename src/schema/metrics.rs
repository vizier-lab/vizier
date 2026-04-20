use std::collections::HashMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UsageSummary {
    pub total_tokens: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_requests: u64,
    pub avg_duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ChannelUsage {
    pub channel_id: String,
    pub total_tokens: u64,
    pub total_requests: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ChannelTypeUsageDetail {
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_requests: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DailyChannelTypeUsage {
    pub date: NaiveDate,
    pub by_channel_type: HashMap<String, ChannelTypeUsageDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ChannelTypeUsage {
    pub total_tokens: u64,
    pub total_requests: u64,
    pub channels: Vec<ChannelUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DailyUsage {
    pub date: NaiveDate,
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_requests: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AgentUsageStats {
    pub summary: UsageSummary,
    pub by_channel_type: HashMap<String, ChannelTypeUsage>,
    pub by_day: Vec<DailyUsage>,
    #[serde(default)]
    pub by_day_and_channel_type: Vec<DailyChannelTypeUsage>,
}