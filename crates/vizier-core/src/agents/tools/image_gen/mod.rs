use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    VizierError,
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    file_manager::FileManager,
    image_generation::{ReferenceImage, VizierImageGen},
    schema::VizierSession,
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
    #[schemars(
        description = "Optional session image filenames to use as references, in order of importance. Turns the request into an image-to-image edit or style transfer instead of text-to-image: pass an image the user sent, one from read_image_file, or one a previous image_generate produced. Not supported by every provider."
    )]
    pub reference_images: Option<Vec<String>>,
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
            "Generate an image from a text prompt. The image file is saved to the session files; its default extension follows the format the provider returns. {size} Pass `reference_images` with session image filenames to edit, restyle, or take a reference from existing images instead of generating from text alone. After generation, call send_attachment with the returned filename to deliver the image to the user."
        )
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let size = args.size.as_deref().or(self.default_size.as_deref());
        let references = load_references(
            &self.storage,
            &self.file_manager,
            &ctx.session,
            &args.reference_images,
        )
        .await?;

        let (image_bytes, mime_type) = self.image_gen.generate(&args.prompt, size, &references).await?;

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

/// The most reference images any supported provider accepts in one request.
const MAX_REFERENCE_IMAGES: usize = 16;

/// The images an `image_generate` call named, read from the conversation's session files in the
/// order given. The media type is the one recorded when the file was saved, never a guess from
/// its name.
async fn load_references(
    storage: &VizierStorage,
    file_manager: &FileManager,
    session: &VizierSession,
    filenames: &Option<Vec<String>>,
) -> Result<Vec<ReferenceImage>, VizierError> {
    let Some(filenames) = filenames.as_ref().filter(|f| !f.is_empty()) else {
        return Ok(vec![]);
    };
    if filenames.len() > MAX_REFERENCE_IMAGES {
        return Err(VizierError(format!(
            "image_generate accepts at most {MAX_REFERENCE_IMAGES} reference images, got {}",
            filenames.len()
        )));
    }
    let mut references = Vec::with_capacity(filenames.len());
    for filename in filenames {
        let file = storage
            .get_session_file(session, filename)
            .await
            .map_err(|e| VizierError(e.to_string()))?
            .ok_or_else(|| VizierError(format!("File not found: {filename}")))?;
        if !file.mime_type.starts_with("image/") {
            return Err(VizierError(format!(
                "image_generate reference images must be image/*, but '{filename}' is '{}'",
                file.mime_type
            )));
        }
        let (_, data) = file_manager
            .get(&file.file_id)
            .await
            .map_err(|e| VizierError(e.to_string()))?;
        references.push(ReferenceImage {
            mime: file.mime_type,
            data,
        });
    }
    Ok(references)
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
    use std::sync::Arc;

    use super::*;

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

    fn files() -> (Arc<VizierStorage>, FileManager, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE session_file (id TEXT PRIMARY KEY, session_slug TEXT NOT NULL, agent_id TEXT NOT NULL, data TEXT NOT NULL);",
        )
        .unwrap();
        let storage = VizierStorage::new(crate::storage::sqlite::SqlStorage::new(
            crate::storage::sql::SqlStore::sqlite(conn),
            Arc::new(crate::storage::document::BlobDocumentStore::local(
                dir.path().to_path_buf(),
            )),
        ));
        let manager = FileManager::new(
            dir.path(),
            Arc::new(crate::storage::blob::LocalBlobStore::new(dir.path())),
        );
        (Arc::new(storage), manager, dir)
    }

    async fn save(
        storage: &VizierStorage,
        manager: &FileManager,
        session: &VizierSession,
        filename: &str,
        bytes: &[u8],
    ) {
        let record = manager.upload(filename, bytes.to_vec()).await.unwrap();
        storage
            .save_session_file(session, filename, &record.mime_type, record.size, &record.id)
            .await
            .unwrap();
    }

    fn session() -> VizierSession {
        VizierSession(
            "agent-1".into(),
            crate::schema::VizierChannelId::HTTP("u".into(), "c".into()),
            None,
        )
    }

    #[tokio::test]
    async fn reference_images_are_read_in_order_with_their_recorded_mime() {
        let (storage, manager, _dir) = files();
        let session = session();
        save(&storage, &manager, &session, "first.png", b"one").await;
        save(&storage, &manager, &session, "second.jpg", b"two").await;

        let names = Some(vec!["second.jpg".into(), "first.png".into()]);
        let references = load_references(&storage, &manager, &session, &names)
            .await
            .unwrap();
        assert_eq!(references.len(), 2);
        assert_eq!(references[0].mime, "image/jpeg");
        assert_eq!(references[0].data, b"two");
        assert_eq!(references[1].mime, "image/png");
        assert_eq!(references[1].data, b"one");
    }

    #[tokio::test]
    async fn no_or_empty_reference_names_mean_a_text_only_request() {
        let (storage, manager, _dir) = files();
        let session = session();
        for names in [None, Some(vec![])] {
            assert!(
                load_references(&storage, &manager, &session, &names)
                    .await
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[tokio::test]
    async fn a_missing_or_non_image_file_is_named_in_the_error() {
        let (storage, manager, _dir) = files();
        let session = session();
        save(&storage, &manager, &session, "notes.docx", b"text").await;

        let Err(VizierError(missing)) = load_references(
            &storage,
            &manager,
            &session,
            &Some(vec!["nope.png".into()]),
        )
        .await
        else {
            panic!("a missing file must be refused");
        };
        assert_eq!(missing, "File not found: nope.png");

        let Err(VizierError(wrong_type)) = load_references(
            &storage,
            &manager,
            &session,
            &Some(vec!["notes.docx".into()]),
        )
        .await
        else {
            panic!("a non-image must be refused");
        };
        assert!(wrong_type.contains("notes.docx"), "{wrong_type}");
        assert!(wrong_type.contains("image/*"), "{wrong_type}");
    }

    #[tokio::test]
    async fn too_many_reference_images_are_refused_before_any_read() {
        let (storage, manager, _dir) = files();
        let names = Some(vec!["a.png".into(); MAX_REFERENCE_IMAGES + 1]);
        let Err(VizierError(error)) = load_references(&storage, &manager, &session(), &names).await
        else {
            panic!("too many references must be refused");
        };
        assert!(error.contains("at most 16"), "{error}");
    }
}
