mod extract;

use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::VizierError;
use crate::utils::get_mime_type;

pub use extract::{ExtractionResult, extract_text, provider_supports_pdf};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContextFile {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
    pub path: String,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContextFileSummary {
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
    pub added_at: DateTime<Utc>,
}

impl From<&ContextFile> for ContextFileSummary {
    fn from(f: &ContextFile) -> Self {
        Self {
            filename: f.filename.clone(),
            mime_type: f.mime_type.clone(),
            size: f.size,
            added_at: f.added_at,
        }
    }
}

#[derive(Clone)]
pub struct ContextFiles {
    dir: PathBuf,
    index_path: PathBuf,
}

impl ContextFiles {
    pub fn new(workspace_path: impl Into<PathBuf>) -> Self {
        let dir = workspace_path.into();
        let index_path = dir.join(".index.json");
        fs::create_dir_all(&dir).ok();
        Self { dir, index_path }
    }

    fn load_index(&self) -> Vec<ContextFile> {
        fs::read_to_string(&self.index_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_index(&self, files: &[ContextFile]) -> Result<(), VizierError> {
        let json = serde_json::to_string_pretty(files)
            .map_err(|e| VizierError(e.to_string()))?;
        fs::write(&self.index_path, json)
            .map_err(|e| VizierError(e.to_string()))?;
        Ok(())
    }

    pub fn list(&self) -> Vec<ContextFile> {
        self.load_index()
    }

    pub fn read_bytes(&self, filename: &str) -> Result<Vec<u8>, VizierError> {
        let files = self.load_index();
        let file = files
            .iter()
            .find(|f| f.filename == filename)
            .ok_or_else(|| VizierError(format!("File not found: {}", filename)))?;

        let path = self.dir.join(&file.path);
        fs::read(&path).map_err(|e| VizierError(e.to_string()))
    }

    pub fn read_text(&self, filename: &str) -> Result<String, VizierError> {
        let bytes = self.read_bytes(filename)?;
        String::from_utf8(bytes).map_err(|e| VizierError(e.to_string()))
    }

    pub fn read_image_base64(&self, filename: &str) -> Result<(String, String), VizierError> {
        let files = self.load_index();
        let file = files
            .iter()
            .find(|f| f.filename == filename)
            .ok_or_else(|| VizierError(format!("File not found: {}", filename)))?;

        if !file.mime_type.starts_with("image/") {
            return Err(VizierError(format!(
                "File is not an image: {} (mime: {})",
                filename, file.mime_type
            )));
        }

        let bytes = self.read_bytes(filename)?;
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
        Ok((b64, file.mime_type.clone()))
    }

    pub fn extract(
        &self,
        filename: &str,
        provider_supports_pdf: bool,
    ) -> Result<ExtractionResult, VizierError> {
        let files = self.load_index();
        let file = files
            .iter()
            .find(|f| f.filename == filename)
            .ok_or_else(|| VizierError(format!("File not found: {}", filename)))?;

        let bytes = self.read_bytes(filename)?;
        Ok(extract_text(&bytes, &file.mime_type, provider_supports_pdf))
    }

    pub fn add(
        &self,
        filename: &str,
        content: Vec<u8>,
    ) -> Result<ContextFile, VizierError> {
        let mime_type = get_mime_type(filename);
        let id = nanoid::nanoid!(12);
        let stored_path = format!("{}_{}", id, filename);
        let file_path = self.dir.join(&stored_path);

        fs::write(&file_path, &content)
            .map_err(|e| VizierError(e.to_string()))?;

        let file = ContextFile {
            id,
            filename: filename.to_string(),
            mime_type,
            size: content.len() as u64,
            path: stored_path,
            added_at: Utc::now(),
        };

        let mut files = self.load_index();
        files.push(file.clone());
        self.save_index(&files)?;

        Ok(file)
    }

    pub fn remove(&self, filename: &str) -> Result<(), VizierError> {
        let mut files = self.load_index();
        let idx = files
            .iter()
            .position(|f| f.filename == filename)
            .ok_or_else(|| VizierError(format!("File not found: {}", filename)))?;

        let file = files.remove(idx);
        let path = self.dir.join(&file.path);
        fs::remove_file(&path).ok();

        self.save_index(&files)
    }
}
