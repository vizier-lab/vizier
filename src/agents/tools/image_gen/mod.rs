use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    VizierError,
    agents::tools::{ToolContext, VizierTool},
    file_manager::FileManager,
    image_generation::VizierImageGen,
    storage::{VizierStorage, session_file::SessionFileStorage},
};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ImageGenerateArgs {
    #[schemars(
        description = "A detailed text description of the image to generate. Be specific about subject, style, lighting, composition, etc."
    )]
    pub prompt: String,
    #[schemars(description = "Output filename (optional, defaults to img_{uuid}.png)")]
    pub filename: Option<String>,
    #[schemars(
        description = "Image size in WxH format, e.g. \"1024x1024\", \"1024x1792\", \"1792x1024\". Optional."
    )]
    pub size: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ImageGenerateOutput {
    pub filename: String,
    pub size: u64,
}

pub struct ImageGenerate {
    pub image_gen: Arc<VizierImageGen>,
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
    pub default_size: Option<String>,
}

#[async_trait::async_trait]
impl VizierTool for ImageGenerate {
    type Input = ImageGenerateArgs;
    type Output = ImageGenerateOutput;

    fn name() -> String {
        "image_generate".to_string()
    }

    fn description(&self) -> String {
        let size = self.default_size.as_deref().unwrap_or("1024x1024");
        format!(
            "Generate an image from a text prompt. The image file is saved to the session files. Default size: \"{}\". After generation, call send_attachment with the returned filename to deliver the image to the user.",
            size
        )
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let size = args.size.as_deref().or(self.default_size.as_deref());

        let (image_bytes, mime_type) = self.image_gen.generate(&args.prompt, size).await?;

        let filename = args
            .filename
            .unwrap_or_else(|| format!("img_{}.png", uuid::Uuid::new_v4()));

        let file_record = self
            .file_manager
            .upload(&filename, image_bytes)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        self.storage
            .save_session_file(
                &ctx.session,
                &filename,
                &mime_type,
                file_record.size,
                &file_record.id,
            )
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        Ok(ImageGenerateOutput {
            filename,
            size: file_record.size,
        })
    }
}
