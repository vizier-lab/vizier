/// Model registry for detecting context window sizes from model names.
///
/// Used as a fallback when the provider doesn't support the ModelListing API.
/// Detects context window size from a model name using pattern matching.
///
/// Returns `Some(context_window)` if the model is recognized, `None` otherwise.
pub fn detect_context_window(model_name: &str) -> Option<u64> {
    let model = model_name.to_lowercase();

    // OpenAI models
    if model.starts_with("gpt-4o") {
        return Some(128_000);
    }
    if model.starts_with("gpt-4-turbo") {
        return Some(128_000);
    }
    if model.starts_with("gpt-3.5") {
        return Some(16_384);
    }

    // Anthropic models
    if model.contains("claude-3.5-sonnet") || model.contains("claude-3-5-sonnet") {
        return Some(200_000);
    }
    if model.contains("claude-3-opus") {
        return Some(200_000);
    }
    if model.contains("claude-3-haiku") {
        return Some(200_000);
    }
    if model.contains("claude-3-sonnet") {
        return Some(200_000);
    }
    if model.contains("claude-3.5-haiku") || model.contains("claude-3-5-haiku") {
        return Some(200_000);
    }

    // Google models
    if model.contains("gemini-1.5-pro") {
        return Some(2_000_000);
    }
    if model.contains("gemini-1.5-flash") {
        return Some(1_000_000);
    }
    if model.contains("gemini-2.0") {
        return Some(1_000_000);
    }
    if model.contains("gemini-2.5") {
        return Some(1_000_000);
    }

    // DeepSeek models
    if model.contains("deepseek-chat") || model.contains("deepseek-v3") {
        return Some(64_000);
    }
    if model.contains("deepseek-coder") {
        return Some(64_000);
    }
    if model.contains("deepseek-reasoner") || model.contains("deepseek-r1") {
        return Some(64_000);
    }

    // Meta Llama models
    if model.contains("llama-3.1") || model.contains("llama3.1") {
        return Some(128_000);
    }
    if model.contains("llama-3") || model.contains("llama3") {
        return Some(8_192);
    }

    // Mistral models
    if model.contains("mistral-large") {
        return Some(128_000);
    }
    if model.contains("mistral-medium") {
        return Some(32_000);
    }
    if model.contains("mistral-small") {
        return Some(32_000);
    }
    if model.contains("mixtral") {
        return Some(32_000);
    }

    // Qwen models
    if model.contains("qwen-2.5") || model.contains("qwen2.5") {
        return Some(128_000);
    }
    if model.contains("qwen-2") || model.contains("qwen2") {
        return Some(128_000);
    }

    // Cohere models
    if model.contains("command-r") {
        return Some(128_000);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_gpt4o() {
        assert_eq!(detect_context_window("gpt-4o"), Some(128_000));
        assert_eq!(detect_context_window("gpt-4o-2024-08-06"), Some(128_000));
        assert_eq!(detect_context_window("gpt-4o-mini"), Some(128_000));
    }

    #[test]
    fn test_detect_claude() {
        assert_eq!(
            detect_context_window("claude-3-5-sonnet-20241022"),
            Some(200_000)
        );
        assert_eq!(
            detect_context_window("claude-3-opus-20240229"),
            Some(200_000)
        );
        assert_eq!(
            detect_context_window("claude-3-haiku-20240307"),
            Some(200_000)
        );
    }

    #[test]
    fn test_detect_gemini() {
        assert_eq!(
            detect_context_window("gemini-1.5-pro"),
            Some(2_000_000)
        );
        assert_eq!(
            detect_context_window("gemini-1.5-flash"),
            Some(1_000_000)
        );
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(detect_context_window("custom-model-v1"), None);
        assert_eq!(detect_context_window("my-fine-tuned-model"), None);
    }
}
