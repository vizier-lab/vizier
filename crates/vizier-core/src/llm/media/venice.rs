//! Venice images. A text-only request goes to the OpenAI-compatible `/images/generations`
//! (through [`OpenAiImages`]); a request with reference images goes to one of Venice's own
//! endpoints, chosen by the model:
//!
//! - an edit model (`…-edit`, Venice's `inpaint` list) takes the images as sources: one to
//!   `/image/edit`, several to `/image/multi-edit`. Both answer with the image's bytes.
//! - any other model takes them as `style_references` on `/image/generate`, which answers
//!   JSON with base64 `images`. Only the models Venice marks `supportsStyleReferences`
//!   accept them; the others answer Venice's own error, which is passed on.
//!
//! Venice's safety filter (`safe_mode`, which blurs what it flags) is on by default; vizier
//! turns it off on every request, as the text path turns its `moderation` to `low`.

use base64::Engine;
use serde_json::{Value, json};

use super::openai_shaped::{Endpoint, ImageApi, OpenAiImages};
use crate::image_generation::{ReferenceImage, VizierImageGenModel};
use crate::llm::http;
use crate::llm::media::{data_url, sniff_image};
use crate::{Result, VizierError};

pub struct VeniceImages(pub Endpoint);

/// Where a request with reference images goes, and its JSON body.
#[derive(Debug, PartialEq)]
enum Route {
    /// `/image/edit` or `/image/multi-edit`: the reply is the image's bytes.
    Edit(&'static str, Value),
    /// `/image/generate`: the reply is JSON with base64 `images`.
    Generate(Value),
}

/// Venice's edit models (its `inpaint` list) are the ones that take source images. Every one
/// has `edit` as a dash-separated part of its id and no generation model does; most end in
/// `-edit`, but `qwen-edit-uncensored` does not, so a suffix test sent it to `/image/generate`
/// as a style-reference model, where Venice answered `404 … did you mean qwen-image-2`.
fn is_edit_model(model: &str) -> bool {
    model.split('-').any(|part| part.eq_ignore_ascii_case("edit"))
}

/// An edit model has nothing to work on without a source image, and Venice's text endpoint
/// does not know it: it answers `404 … not found, did you mean …` with generation models,
/// which reads as a misspelt id. Say what is actually missing instead.
fn needs_no_source(model: &str) -> Result<()> {
    if is_edit_model(model) {
        return Err(VizierError(format!(
            "image generation: `{model}` is a Venice edit model and needs `reference_images`              (the image to edit). Pass the source image, or configure a text-to-image model              (one without `edit` in its name) for generating from a prompt alone."
        )));
    }
    Ok(())
}

fn base64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// How a `size` setting maps onto Venice's own fields: a tier such as `2K` is a
/// `resolution`, a ratio such as `16:9` an `aspect_ratio`, and `WxH` pixels are for `/image/
/// generate` only (the edit endpoints have no pixel size and infer the shape from the input).
fn apply_size(body: &mut Value, size: Option<&str>, pixels: bool) {
    let Some(size) = size.map(str::trim).filter(|s| !s.is_empty()) else {
        return;
    };
    let upper = size.to_ascii_uppercase();
    if upper.ends_with('K') && upper[..upper.len() - 1].parse::<f32>().is_ok() {
        body["resolution"] = json!(upper);
    } else if let Some((w, h)) = size.split_once(':')
        && w.trim().parse::<f32>().is_ok()
        && h.trim().parse::<f32>().is_ok()
    {
        body["aspect_ratio"] = json!(size);
    } else if pixels
        && let Some((w, h)) = size.split_once('x')
        && let (Ok(w), Ok(h)) = (w.trim().parse::<u32>(), h.trim().parse::<u32>())
    {
        body["width"] = json!(w);
        body["height"] = json!(h);
    }
}

fn route(model: &str, prompt: &str, size: Option<&str>, references: &[ReferenceImage]) -> Route {
    if is_edit_model(model) {
        let mut body = match references {
            // `/image/edit` documents raw base64 for a JSON image (data URIs only on
            // multi-edit), and is the one endpoint every edit model serves.
            [only] => json!({"model": model, "prompt": prompt, "image": base64(&only.data)}),
            // Multi-edit names the model `modelId`; the first image is the base.
            many => json!({
                "modelId": model,
                "prompt": prompt,
                "images": many.iter().map(|r| data_url(&r.mime, &r.data)).collect::<Vec<_>>(),
            }),
        };
        body["safe_mode"] = json!(false);
        apply_size(&mut body, size, false);
        let path = if references.len() == 1 { "/image/edit" } else { "/image/multi-edit" };
        return Route::Edit(path, body);
    }
    let mut body = json!({
        "model": model,
        "prompt": prompt,
        "style_references": references
            .iter()
            .map(|r| json!({"image": data_url(&r.mime, &r.data)}))
            .collect::<Vec<_>>(),
        "safe_mode": false,
    });
    apply_size(&mut body, size, true);
    Route::Generate(body)
}

#[async_trait::async_trait]
impl VizierImageGenModel for VeniceImages {
    async fn generate(
        &self,
        prompt: &str,
        size: Option<&str>,
        references: &[ReferenceImage],
    ) -> Result<(Vec<u8>, String)> {
        if references.is_empty() {
            needs_no_source(&self.0.model)?;
            let text = OpenAiImages {
                endpoint: self.0.clone(),
                api: ImageApi::Venice,
            };
            return text.generate(prompt, size, references).await;
        }
        let fail = |e| VizierError(format!("image generation: {e}"));
        match route(&self.0.model, prompt, size, references) {
            Route::Edit(path, body) => {
                let req = self.0.post(path).await.map_err(fail)?;
                let reply = http::send(req.json(&body)).await.map_err(fail)?;
                let mime = reply
                    .headers
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .filter(|m| m.starts_with("image/"))
                    .map(str::to_string)
                    .or_else(|| sniff_image(&reply.body).map(str::to_string))
                    .ok_or_else(|| {
                        VizierError("image generation: Venice answered something that is not an image".into())
                    })?;
                Ok((reply.body, mime))
            }
            Route::Generate(body) => {
                let req = self.0.post("/image/generate").await.map_err(fail)?;
                let reply = http::send_json(req.json(&body)).await.map_err(fail)?;
                decode_generated(&reply)
            }
        }
    }
}

/// The first of `/image/generate`'s base64 `images`, a `data:` prefix tolerated.
fn decode_generated(reply: &Value) -> Result<(Vec<u8>, String)> {
    let first = reply
        .pointer("/images/0")
        .and_then(Value::as_str)
        .ok_or_else(|| VizierError("image generation: missing image data".into()))?;
    let b64 = first.split_once(";base64,").map_or(first, |(_, data)| data);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| VizierError(format!("image generation: {e}")))?;
    // Venice's default format is WebP; the bytes say which it is.
    let mime = sniff_image(&bytes).unwrap_or("image/webp").to_string();
    Ok((bytes, mime))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(data: &[u8]) -> ReferenceImage {
        ReferenceImage { mime: "image/png".into(), data: data.to_vec() }
    }

    #[test]
    fn one_image_on_an_edit_model_goes_to_the_edit_endpoint_as_base64() {
        let Route::Edit(path, body) =
            route("nano-banana-2-edit", "make it blue", Some("2K"), &[png(&[1, 2, 3])])
        else {
            panic!("an edit model edits");
        };
        assert_eq!(path, "/image/edit");
        assert_eq!(body["model"], json!("nano-banana-2-edit"));
        assert_eq!(body["image"], json!("AQID"));
        assert_eq!(body["resolution"], json!("2K"));
        assert_eq!(body["safe_mode"], json!(false));
        assert!(body.get("modelId").is_none(), "{body}");
    }

    #[test]
    fn several_images_go_to_multi_edit_with_the_first_as_the_base() {
        let Route::Edit(path, body) =
            route("seedream-v5-5-edit", "merge them", Some("16:9"), &[png(&[1]), png(&[2])])
        else {
            panic!("an edit model edits");
        };
        assert_eq!(path, "/image/multi-edit");
        assert_eq!(body["modelId"], json!("seedream-v5-5-edit"));
        assert_eq!(
            body["images"],
            json!(["data:image/png;base64,AQ==", "data:image/png;base64,Ag=="])
        );
        assert_eq!(body["aspect_ratio"], json!("16:9"));
        assert_eq!(body["safe_mode"], json!(false));
        assert!(body.get("model").is_none(), "{body}");
    }

    /// The edit endpoints have no pixel size: `WxH` is dropped and the shape follows the input.
    #[test]
    fn an_edit_ignores_a_pixel_size() {
        let Route::Edit(_, body) = route("gpt-image-2-edit", "p", Some("1024x1024"), &[png(&[1])])
        else {
            panic!("an edit model edits");
        };
        assert!(body.get("width").is_none() && body.get("resolution").is_none(), "{body}");
    }

    #[test]
    fn another_model_takes_the_images_as_style_references() {
        let Route::Generate(body) =
            route("krea-v2-large", "a fox", Some("1280x720"), &[png(&[1, 2, 3])])
        else {
            panic!("a generation model generates");
        };
        assert_eq!(
            body["style_references"],
            json!([{"image": "data:image/png;base64,AQID"}])
        );
        assert_eq!(body["safe_mode"], json!(false));
        assert_eq!(body["width"], json!(1280));
        assert_eq!(body["height"], json!(720));
    }

    #[test]
    fn edit_models_are_told_apart_by_their_edit_segment() {
        for id in ["nano-banana-2-edit", "seedream-v5-5-edit", "qwen-edit-uncensored", "firered-image-edit"] {
            assert!(is_edit_model(id), "{id}");
        }
        for id in ["qwen-image-2-pro", "lustify-sdxl", "venice-sd35", "krea-v2-large", "credited-model"] {
            assert!(!is_edit_model(id), "{id}");
        }
        // The uncensored Qwen edit model goes to the edit endpoint, not to style references.
        assert!(matches!(
            route("qwen-edit-uncensored", "p", None, &[png(&[1])]),
            Route::Edit("/image/edit", _)
        ));
    }

    #[test]
    fn an_edit_model_without_a_source_image_is_refused_before_any_call() {
        let error = needs_no_source("qwen-edit-uncensored").unwrap_err().0;
        assert!(error.contains("edit model") && error.contains("reference_images"), "{error}");
        assert!(needs_no_source("lustify-sdxl").is_ok());
    }

    #[test]
    fn generated_images_decode_bare_or_with_a_data_prefix() {
        let jpeg = base64(&[0xff, 0xd8, 0xff, 0]);
        let (bytes, mime) = decode_generated(&json!({"images": [jpeg.clone()]})).unwrap();
        assert_eq!(bytes, vec![0xff, 0xd8, 0xff, 0]);
        assert_eq!(mime, "image/jpeg");
        let prefixed = format!("data:image/jpeg;base64,{jpeg}");
        assert_eq!(decode_generated(&json!({"images": [prefixed]})).unwrap().1, "image/jpeg");
        assert!(decode_generated(&json!({"images": []})).is_err());
    }
}
