use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    VizierError,
    agents::tools::{ToolCategory, ToolContext, VizierTool},
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
    #[schemars(description = "Output filename (optional, defaults to img_{uuid} with an extension matching the returned image format)")]
    pub filename: Option<String>,
    #[schemars(
        description = "Image size in WxH format, e.g. \"1024x1024\", \"1024x1792\", \"1792x1024\". Optional; the provider's own default is used when omitted."
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

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Media)
    }

    fn name() -> String {
        "image_generate".to_string()
    }

    fn description(&self) -> String {
        let size = match self.default_size.as_deref() {
            Some(size) => format!("Default size: \"{size}\"."),
            None => "With no size given, the provider's default size is used.".to_string(),
        };
        format!(
            "Generate an image from a text prompt. The image file is saved to the session files; its default extension follows the format the provider returns. {size} After generation, call send_attachment with the returned filename to deliver the image to the user."
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
            .unwrap_or_else(|| format!("img_{}.{}", uuid::Uuid::new_v4(), extension_for(&mime_type)));

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

/// The default filename extension for the media type a provider returned (FR-006): an image
/// saved as PNG when the provider drew a WebP would open as a broken PNG.
fn extension_for(mime: &str) -> &'static str {
    // Drop a charset or other parameters before matching.
    let base = mime.split(';').next().unwrap_or(mime).trim();
    match base.to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => "png",
    }
}

#[cfg(test)]
mod tests {
    use super::extension_for;

    #[test]
    fn the_default_extension_follows_the_returned_media_type() {
        assert_eq!(extension_for("image/png"), "png");
        assert_eq!(extension_for("image/jpeg"), "jpg");
        assert_eq!(extension_for("image/jpg"), "jpg");
        assert_eq!(extension_for("image/webp"), "webp");
        assert_eq!(extension_for("image/svg+xml"), "svg");
        assert_eq!(extension_for("image/svg+xml; charset=utf-8"), "svg");
        assert_eq!(extension_for("IMAGE/WEBP"), "webp");
        assert_eq!(extension_for("application/octet-stream"), "png");
        assert_eq!(extension_for(""), "png");
    }
}
