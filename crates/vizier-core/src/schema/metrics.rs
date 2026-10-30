use crate::{catalogue::data::Cost, schema::VizierResponseStats};

/// Token counts of one call, or of a group of calls, as the provider reported them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenCounts {
    pub input: u64,
    pub cached: u64,
    pub cache_write: u64,
    pub output: u64,
}

impl TokenCounts {
    pub fn total(&self) -> u64 {
        self.input + self.output
    }
}

impl std::ops::AddAssign for TokenCounts {
    fn add_assign(&mut self, rhs: Self) {
        self.input += rhs.input;
        self.cached += rhs.cached;
        self.cache_write += rhs.cache_write;
        self.output += rhs.output;
    }
}

impl std::ops::Add for TokenCounts {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self {
        self += rhs;
        self
    }
}

impl From<&VizierResponseStats> for TokenCounts {
    fn from(stats: &VizierResponseStats) -> Self {
        Self {
            input: stats.total_input_tokens,
            cached: stats.total_cached_input_tokens,
            cache_write: stats.total_cache_creation_input_tokens,
            output: stats.total_output_tokens,
        }
    }
}

/// Estimated cost in USD: uncached input at the input price, cache reads and writes at their
/// own prices (or the input price when the catalogue has none), output at the output price.
pub fn estimate_cost(tokens: &TokenCounts, cost: &Cost, cache_outside_input: bool) -> f64 {
    let cached = tokens.cached as f64;
    let created = tokens.cache_write as f64;
    let input = tokens.input as f64;
    let uncached = if cache_outside_input {
        input
    } else {
        (input - cached - created).max(0.0)
    };
    (uncached * cost.input
        + cached * cost.cache_read.unwrap_or(cost.input)
        + created * cost.cache_write.unwrap_or(cost.input)
        + tokens.output as f64 * cost.output)
        / 1e6
}

/// The share of input served from cache. `cache_outside_input` as for [`estimate_cost`]: when
/// the provider counts cached tokens outside `input` (Anthropic), they are added back in.
pub fn cache_hit_rate(tokens: &TokenCounts, cache_outside_input: bool) -> Option<f64> {
    let whole = if cache_outside_input {
        tokens.input + tokens.cached + tokens.cache_write
    } else {
        tokens.input
    };
    (whole > 0).then(|| tokens.cached as f64 / whole as f64)
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
        let openai = estimate_cost(&TokenCounts::from(&stats(1_000_000, 400_000, 0, 100_000)), &SONNET, false);
        assert!((openai - (0.6 * 3.0 + 0.4 * 0.3 + 0.1 * 15.0)).abs() < 1e-9);
        // Anthropic-style: input excludes the 400k read and 100k written.
        let anthropic = estimate_cost(&TokenCounts::from(&stats(600_000, 400_000, 100_000, 100_000)), &SONNET, true);
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
        assert!((estimate_cost(&TokenCounts::from(&stats(1_000_000, 500_000, 0, 0)), &plain, false) - 1.0).abs() < 1e-9);
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

    #[test]
    fn cache_hit_rate_adds_cached_tokens_back_when_the_provider_counts_them_outside_input() {
        let inside = TokenCounts { input: 1000, cached: 400, cache_write: 0, output: 10 };
        assert_eq!(cache_hit_rate(&inside, false), Some(0.4));
        let outside = TokenCounts { input: 600, cached: 300, cache_write: 100, output: 10 };
        assert_eq!(cache_hit_rate(&outside, true), Some(0.3));
        assert_eq!(cache_hit_rate(&TokenCounts::default(), false), None);
    }
}
