use anyhow::Result;
use rig_core::OneOrMany;
use rig_core::message::{ImageMediaType, Message, MimeType, UserContent};

use crate::agents::agent::model::{VizierModel, VizierModelTrait};
use crate::config::provider::ProviderVariant;
use crate::dependencies::VizierDependencies;
use crate::schema::AgentConfig;

#[derive(Clone)]
pub struct VizierImageProcessor {
    model: VizierModel,
    is_same_as_main: bool,
}

impl VizierImageProcessor {
    pub async fn new(
        deps: &VizierDependencies,
        agent_config: &AgentConfig,
    ) -> Result<Self> {
        let (model, is_same_as_main) = if let (Some(img_provider), Some(img_model)) =
            (&agent_config.tools.filesystem.settings.image_provider, &agent_config.tools.filesystem.settings.image_model)
        {
            let same_as_main = *img_provider == agent_config.provider && img_model == &agent_config.model;
            let model = VizierModel::new_with_override(
                deps,
                agent_config,
                Some((img_provider.clone(), img_model.clone())),
            )
            .await?;
            (model, same_as_main)
        } else {
            (VizierModel::new(
                agent_config.name.clone(),
                deps.clone(),
                agent_config,
            ).await?, true)
        };

        Ok(Self { model, is_same_as_main })
    }

    pub fn is_same_as_main_model(&self) -> bool {
        self.is_same_as_main
    }

    pub async fn describe(
        &self,
        image_base64: &str,
        mime_type: &str,
        prompt: &str,
    ) -> Result<String> {
        let media_type = ImageMediaType::from_mime_type(mime_type)
            .unwrap_or(ImageMediaType::PNG);

        let message = Message::User {
            content: OneOrMany::many(vec![
                UserContent::Text(prompt.into()),
                UserContent::image_base64(image_base64.to_string(), Some(media_type), None),
            ]).unwrap(),
        };

        let (output, _, _) = self.model.completion(message, vec![], vec![]).await?;
        Ok(output.unwrap_or_default())
    }
}
