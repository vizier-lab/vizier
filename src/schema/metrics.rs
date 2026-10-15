use std::collections::HashMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::{catalogue::data::Cost, config::provider::ProviderId, schema::VizierResponseStats};

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UsageSummary {
    pub total_tokens: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_requests: u64,
    pub avg_duration_ms: f64,
    /// `null` (not `0`) when no reply in range has a known price (FR-021).
    pub estimated_cost_usd: Option<f64>,
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
    pub estimated_cost_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DailyUsage {
    pub date: NaiveDate,
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_requests: u64,
    pub estimated_cost_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AgentUsageStats {
    pub summary: UsageSummary,
    pub by_channel_type: HashMap<String, ChannelTypeUsage>,
    pub by_day: Vec<DailyUsage>,
    #[serde(default)]
    pub by_day_and_channel_type: Vec<DailyChannelTypeUsage>,
}
/// A `(provider, model)`'s price per million tokens, and whether that provider counts cached
/// tokens outside `input_tokens` (Anthropic) rather than inside them (OpenAI and most others).
pub type Pricing<'a> = &'a (dyn Fn(&ProviderId, &str) -> Option<(Cost, bool)> + Send + Sync);

/// One reply's estimated cost in USD: uncached input at the input price, cache reads and writes
/// at their own prices (or the input price when the catalogue has none), output at the output
/// price.
pub fn estimate_cost(stats: &VizierResponseStats, cost: &Cost, cache_outside_input: bool) -> f64 {
    let cached = stats.total_cached_input_tokens as f64;
    let created = stats.total_cache_creation_input_tokens as f64;
    let input = stats.total_input_tokens as f64;
    let uncached = if cache_outside_input {
        input
    } else {
        (input - cached - created).max(0.0)
    };
    (uncached * cost.input
        + cached * cost.cache_read.unwrap_or(cost.input)
        + created * cost.cache_write.unwrap_or(cost.input)
        + stats.total_output_tokens as f64 * cost.output)
        / 1e6
}

/// Adds `value` into an estimate that stays `None` until something is priced.
pub fn add_cost(total: &mut Option<f64>, value: Option<f64>) {
    if let Some(v) = value {
        *total = Some(total.unwrap_or(0.0) + v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(input: u64, cached: u64, created: u64, output: u64) -> VizierResponseStats {
        VizierResponseStats {
            total_input_tokens: input,
            total_cached_input_tokens: cached,
            total_cache_creation_input_tokens: created,
            total_output_tokens: output,
            ..Default::default()
        }
    }

    const SONNET: Cost = Cost {
        input: 3.0,
        output: 15.0,
        cache_read: Some(0.3),
        cache_write: Some(3.75),
    };

    #[test]
    fn cost_counts_cached_tokens_once_whichever_way_the_provider_reports_them() {
        // OpenAI-style: 1M input of which 400k cached, 100k output.
        let openai = estimate_cost(&stats(1_000_000, 400_000, 0, 100_000), &SONNET, false);
        assert!((openai - (0.6 * 3.0 + 0.4 * 0.3 + 0.1 * 15.0)).abs() < 1e-9);
        // Anthropic-style: input excludes the 400k read and 100k written.
        let anthropic = estimate_cost(&stats(600_000, 400_000, 100_000, 100_000), &SONNET, true);
        assert!((anthropic - (0.6 * 3.0 + 0.4 * 0.3 + 0.1 * 3.75 + 0.1 * 15.0)).abs() < 1e-9);
    }

    #[test]
    fn missing_cache_prices_fall_back_to_the_input_price() {
        let plain = Cost {
            input: 1.0,
            output: 2.0,
            cache_read: None,
            cache_write: None,
        };
        assert!((estimate_cost(&stats(1_000_000, 500_000, 0, 0), &plain, false) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn an_unpriced_bucket_stays_none() {
        let mut total = None;
        add_cost(&mut total, None);
        assert_eq!(total, None);
        add_cost(&mut total, Some(0.5));
        add_cost(&mut total, None);
        assert_eq!(total, Some(0.5));
    }
}
