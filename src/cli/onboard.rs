use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use clap::Args;
use inquire::{Confirm, Password, Select, Text};
use termimad::print_text;

use crate::config::{
    ChannelsConfig, HTTPChannelConfig, VizierConfig,
    embedding::{EmbeddingConfig, LocalEmbeddingModelVariant},
    provider::ProviderConfig,
    storage::{DocumentIndexerConfig, StorageConfig},
    tools::ToolsConfig,
};

#[derive(Debug, Args, Clone)]
pub struct OnboardArgs {
    #[arg(short, long, value_name = "PATH", help = "path to workspace")]
    pub path: Option<String>,
}

pub fn onboard(args: OnboardArgs) -> Result<()> {
    crate::utils::logo::print_logo();

    let d = "\x1b[2m";
    let r = "\x1b[0m";

    println!("  Welcome! Let's set up your agent workspace.");
    println!("  This will generate a {d}.vizier.yaml{r} config.");
    println!("  You can create your user account later via the WebUI.");
    println!();
    let workspace = match &args.path {
        Some(p) => p.clone(),
        None => Text::new("Workspace path:")
            .with_default(&std::env::current_dir()?.to_string_lossy())
            .prompt()?,
    };

    let workspace_path = if workspace.starts_with('~') {
        dirs::home_dir()
            .unwrap()
            .join(workspace.strip_prefix('~').unwrap())
    } else {
        PathBuf::from(&workspace)
    };

    let port: u32 = Text::new("HTTP port:")
        .with_default("9999")
        .prompt()?
        .parse()
        .unwrap_or(9999);

    let default_jwt = nanoid::nanoid!(32);
    let jwt_secret = Text::new("JWT secret:")
        .with_default(&default_jwt)
        .prompt()?;

    let mut providers = ProviderConfig::default();

    print_text("\n> **Note:** You can add more providers later in the WebUI settings.\n");

    let provider_type = Select::new(
        "Select primary provider:",
        vec![
            "mistralrs",
            "ollama",
            "deepseek",
            "openrouter",
            "anthropic",
            "openai",
            "gemini",
            "mimo",
            "llama_cpp",
        ],
    )
    .prompt()?;

    match provider_type {
        "mistralrs" => {
            providers.mistralrs = Some(crate::config::provider::MistralrsProviderConfig {
                enabled: true,
            });
        }
        "ollama" => {
            let base_url = Text::new("Ollama base URL:")
                .with_default("http://localhost:11434")
                .prompt()?;
            providers.ollama = Some(crate::config::provider::OllamaProviderConfig { base_url });
        }
        "deepseek" => {
            let api_key = Password::new("Deepseek API key:").prompt()?;
            providers.deepseek = Some(crate::config::provider::DeepseekProviderConfig { api_key });
        }
        "openrouter" => {
            let api_key = Password::new("OpenRouter API key:").prompt()?;
            providers.openrouter =
                Some(crate::config::provider::OpenRouterProviderConfig { api_key });
        }
        "anthropic" => {
            let api_key = Password::new("Anthropic API key:").prompt()?;
            providers.anthropic = Some(crate::config::provider::AnthropicProviderConfig { api_key });
        }
        "openai" => {
            let api_key = Password::new("OpenAI API key:").prompt()?;
            providers.openai = Some(crate::config::provider::OpenAIProviderConfig { api_key });
        }
        "gemini" => {
            let api_key = Password::new("Gemini API key:").prompt()?;
            providers.gemini = Some(crate::config::provider::GeminiProviderConfig { api_key });
        }
        "mimo" => {
            let api_key = Password::new("Xiaomi MiMo API key:").prompt()?;
            providers.mimo = Some(crate::config::provider::MimoProviderConfig { api_key });
        }
        "llama_cpp" => {
            let base_url = Text::new("Llama.cpp base URL:")
                .with_default("http://localhost:8080")
                .prompt()?;
            providers.llama_cpp =
                Some(crate::config::provider::LlamaCppProviderConfig { base_url });
        }
        _ => unreachable!(),
    }

    let embedding_type = Select::new(
        "Embedding type:",
        vec!["local", "openrouter", "ollama", "openai", "gemini"],
    )
    .prompt()?;

    let embedding = match embedding_type {
        "local" => {
            let models: Vec<(&str, &str, LocalEmbeddingModelVariant)> = vec![
                (
                    "all_mini_lml6_v2",
                    "lightweight",
                    LocalEmbeddingModelVariant::AllMiniLml6V2,
                ),
                (
                    "all_mini_lml6_v2q",
                    "lightweight",
                    LocalEmbeddingModelVariant::AllMiniLml6V2Q,
                ),
                (
                    "all_mini_lml12_v2",
                    "performance",
                    LocalEmbeddingModelVariant::AllMiniLml12V2,
                ),
                (
                    "all_mini_lml12_v2q",
                    "balanced",
                    LocalEmbeddingModelVariant::AllMiniLml12V2Q,
                ),
                (
                    "bge_base_env15",
                    "balanced",
                    LocalEmbeddingModelVariant::BgeBaseEnv15,
                ),
                (
                    "bge_base_env15q",
                    "balanced",
                    LocalEmbeddingModelVariant::BgeBaseEnv15Q,
                ),
                (
                    "bge_large_env15",
                    "performance",
                    LocalEmbeddingModelVariant::BgeLargeEnv15,
                ),
                (
                    "bge_large_env15q",
                    "balanced",
                    LocalEmbeddingModelVariant::BgeLargeEnv15Q,
                ),
                (
                    "bge_small_env15",
                    "lightweight",
                    LocalEmbeddingModelVariant::BgeSmallEnv15,
                ),
                (
                    "bge_small_env15q",
                    "lightweight",
                    LocalEmbeddingModelVariant::BgeSmallEnv15Q,
                ),
                (
                    "nomic_embed_text_v1",
                    "balanced",
                    LocalEmbeddingModelVariant::NomicEmbedTextV1,
                ),
                (
                    "nomic_embed_text_v15",
                    "balanced",
                    LocalEmbeddingModelVariant::NomicEmbedTextV15,
                ),
                (
                    "nomic_embed_text_v15q",
                    "balanced",
                    LocalEmbeddingModelVariant::NomicEmbedTextV15Q,
                ),
                (
                    "paraphrase_ml_mini_lml12_v2",
                    "lightweight",
                    LocalEmbeddingModelVariant::ParaphraseMlMiniLML12V2,
                ),
                (
                    "paraphrase_ml_mini_lml12_v2q",
                    "lightweight",
                    LocalEmbeddingModelVariant::ParaphraseMlMiniLML12V2Q,
                ),
                (
                    "paraphrase_ml_mpnet_base_v2",
                    "balanced",
                    LocalEmbeddingModelVariant::ParaphraseMlMpnetBaseV2,
                ),
                (
                    "bge_small_zh_v15",
                    "lightweight",
                    LocalEmbeddingModelVariant::BgeSmallZhv15,
                ),
                (
                    "bge_large_zh_v15",
                    "performance",
                    LocalEmbeddingModelVariant::BgeLargeZhv15,
                ),
                (
                    "modernbert_embed_large",
                    "performance",
                    LocalEmbeddingModelVariant::ModernBertEmbedLarge,
                ),
                (
                    "multilingual_e5_small",
                    "lightweight",
                    LocalEmbeddingModelVariant::MultilingualE5Small,
                ),
                (
                    "multilingual_e5_base",
                    "balanced",
                    LocalEmbeddingModelVariant::MultilingualE5Base,
                ),
                (
                    "multilingual_e5_large",
                    "performance",
                    LocalEmbeddingModelVariant::MultilingualE5Large,
                ),
                (
                    "mxbai_embed_large_v1",
                    "performance",
                    LocalEmbeddingModelVariant::MxbaiEmbedLargeV1,
                ),
                (
                    "mxbai_embed_large_v1q",
                    "balanced",
                    LocalEmbeddingModelVariant::MxbaiEmbedLargeV1Q,
                ),
                (
                    "gte_base_env15",
                    "balanced",
                    LocalEmbeddingModelVariant::GteBaseEnv15,
                ),
                (
                    "gte_base_env15q",
                    "balanced",
                    LocalEmbeddingModelVariant::GteBaseEnv15Q,
                ),
                (
                    "gte_large_env15",
                    "performance",
                    LocalEmbeddingModelVariant::GteLargeEnv15,
                ),
                (
                    "gte_large_env15q",
                    "balanced",
                    LocalEmbeddingModelVariant::GteLargeEnv15Q,
                ),
                (
                    "clip_vit_b32",
                    "balanced",
                    LocalEmbeddingModelVariant::ClipVitB32,
                ),
                (
                    "jina_embeddings_v2_base_code",
                    "balanced",
                    LocalEmbeddingModelVariant::JinaEmbeddingsV2BaseCode,
                ),
            ];

            print_text("**Recommended models:**\n");
            println!("  🟢 **lightweight**  →  `all_mini_lml6_v2`       _(fastest, smallest)_");
            println!(
                "  🔵 **balanced**     →  `nomic_embed_text_v15q`  _(best quality/size tradeoff)_"
            );
            println!(
                "  🟡 **performance**  →  `bge_large_env15`        _(highest retrieval quality)_\n"
            );

            let model_names: Vec<&str> = models.iter().map(|(name, _, _)| *name).collect();
            let selected = Select::new("Select embedding model:", model_names).prompt()?;
            let variant = models
                .into_iter()
                .find(|(name, _, _)| *name == selected)
                .unwrap()
                .2;
            EmbeddingConfig::Local { model: variant }
        }
        "openrouter" => {
            let api_key = Password::new("OpenRouter API key:").prompt()?;
            let model = Text::new("Model name:")
                .with_default("nomic-ai/nomic-embed-text-v1.5")
                .prompt()?;
            EmbeddingConfig::Openrouter { model }
        }
        "ollama" => {
            let base_url = Text::new("Ollama base URL:")
                .with_default("http://localhost:11434")
                .prompt()?;
            let model = Text::new("Model name:")
                .with_default("nomic-embed-text")
                .prompt()?;
            EmbeddingConfig::Ollama { model }
        }
        "openai" => {
            let api_key = Password::new("OpenAI API key:").prompt()?;
            let model = Text::new("Model name:")
                .with_default("text-embedding-3-small")
                .prompt()?;
            EmbeddingConfig::Openai { model }
        }
        "gemini" => {
            let api_key = Password::new("Gemini API key:").prompt()?;
            let model = Text::new("Model name:")
                .with_default("text-embedding-004")
                .prompt()?;
            EmbeddingConfig::Gemini { model }
        }
        _ => unreachable!(),
    };

    let storage_type = Select::new("Storage type:", vec!["Filesystem", "Surreal"]).prompt()?;

    let storage = if storage_type == "Surreal" {
        StorageConfig::Surreal
    } else {
        StorageConfig::Filesystem(DocumentIndexerConfig::InMem)
    };

    let worker_threads: usize = Text::new("Worker threads:")
        .with_default("4")
        .prompt()?
        .parse()
        .unwrap_or(4);

    let ws_idle_timeout_secs: u64 = Text::new("WebSocket idle timeout (seconds):")
        .with_default("300")
        .prompt()?
        .parse()
        .unwrap_or(300);

    let mut config_path = workspace_path.clone();
    config_path.push(".vizier.yaml");

    let embedding_model = match &embedding {
        EmbeddingConfig::Local { model } => format!("{:?}", model),
        EmbeddingConfig::Openrouter { model } => model.clone(),
        EmbeddingConfig::Ollama { model } => model.clone(),
        EmbeddingConfig::Openai { model } => model.clone(),
        EmbeddingConfig::Gemini { model } => model.clone(),
    };

    print_text(&format!(
        "\n---\n### Configuration Preview\n\n\
        | | |\n\
        |---|---|\n\
        | **Workspace** | `{}` |\n\
        | **Config file** | `{}` |\n\
        | **HTTP port** | `{}` |\n\
        | **Provider** | `{}` |\n\
        | **Embedding** | `{} ({})` |\n\
        | **Storage** | `{}` |\n\
        | **Worker threads** | `{}` |\n\
        | **WS idle timeout** | `{}s` |\n\
        ---\n",
        workspace_path.display(),
        config_path.display(),
        port,
        provider_type,
        embedding_type,
        embedding_model,
        storage_type,
        worker_threads,
        ws_idle_timeout_secs,
    ));

    let confirmed = Confirm::new("Save configuration?")
        .with_default(true)
        .prompt()?;

    if !confirmed {
        println!("Onboarding cancelled.");
        return Ok(());
    }

    let config = VizierConfig {
        workspace: workspace.clone(),
        embedding: Some(embedding),
        providers,
        storage,
        channels: ChannelsConfig {
            discord: None,
            http: Some(HTTPChannelConfig {
                port,
                jwt_secret,
                jwt_expiry_hours: 720,
                ws_idle_timeout_secs,
            }),
            telegram: None,
        },
        tools: ToolsConfig {
            brave_search: None,
        },
        worker_threads,
    };

    config.save(config_path.clone(), "".into())?;

    print_text(&format!(
        "\n### Onboarding complete!\n\n\
        Configuration saved to `{}`\n\n\
        | | |\n\
        |---|---|\n\
        | **Start server** | `vizier run` |\n\
        | **WebUI** | `http://localhost:{}` |\n\n\
        Open the WebUI and follow the onboarding flow to create your first user account.\n",
        config_path.display(),
        port,
    ));

    Ok(())
}
