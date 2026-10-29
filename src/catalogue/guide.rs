//! Static in-form setup guides, as provider data on the catalogue listing (research R10,
//! contract `provider-credentials-api.md` §1). The form renders whatever guide a provider has;
//! there is no provider-id branch in the WebUI.

use serde::Serialize;

use crate::config::provider::ProviderId;

/// One numbered step of a guide.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct GuideStep {
    pub title: String,
    pub body: String,
}

/// A copyable policy or snippet.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct GuidePolicy {
    pub language: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct GuideLink {
    pub label: String,
    pub url: String,
}

/// What the connect form shows above the fields. Static metadata, never stored or editable.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ProviderGuide {
    pub intro: String,
    pub steps: Vec<GuideStep>,
    pub policy: GuidePolicy,
    pub links: Vec<GuideLink>,
}

/// The exact minimal policy: `ListFoundationModels` is the Test action's read permission, and
/// `InvokeModel` runs chat, embeddings and images. Copied verbatim by the form and the docs.
pub const BEDROCK_POLICY: &str = "{\n  \"Version\": \"2012-10-17\",\n  \"Statement\": [{\n    \"Effect\": \"Allow\",\n    \"Action\": [\"bedrock:InvokeModel\", \"bedrock:ListFoundationModels\"],\n    \"Resource\": \"*\"\n  }]\n}";

/// The guide for a provider, if it has one. Only Amazon Bedrock has one today.
pub fn for_provider(id: &ProviderId) -> Option<ProviderGuide> {
    (*id == ProviderId::AMAZON_BEDROCK).then(bedrock)
}

fn step(title: &str, body: &str) -> GuideStep {
    GuideStep {
        title: title.into(),
        body: body.into(),
    }
}

fn link(label: &str, url: &str) -> GuideLink {
    GuideLink {
        label: label.into(),
        url: url.into(),
    }
}

fn bedrock() -> ProviderGuide {
    ProviderGuide {
        intro: "Use an IAM access key pair with permission to invoke Bedrock models.".into(),
        steps: vec![
            step(
                "Create an IAM access key",
                "IAM → Users → Security credentials → Create access key. Choose “Application \
                 running outside AWS”. The secret is shown once.",
            ),
            step(
                "Attach the minimal policy",
                "Allow the actions below. bedrock:ListFoundationModels is used by Test; \
                 bedrock:InvokeModel runs chat, embeddings and images.",
            ),
            step(
                "Enable model access",
                "Bedrock → Model access: enable the chat, embedding and image models you will \
                 use. Anthropic models may need a use-case form.",
            ),
            step(
                "Choose a region",
                "Use a Bedrock region where the model is offered, e.g. us-east-1. Cross-region \
                 ids (us., eu., apac., global.) are accepted as model ids.",
            ),
            step(
                "Paste the values",
                "Enter the access key ID, secret access key and region. Add the session token \
                 only for temporary credentials.",
            ),
        ],
        policy: GuidePolicy {
            language: "json".into(),
            code: BEDROCK_POLICY.into(),
        },
        links: vec![
            link(
                "AWS IAM users",
                "https://console.aws.amazon.com/iam/home#/users",
            ),
            link(
                "Bedrock model access",
                "https://console.aws.amazon.com/bedrock/home#/modelaccess",
            ),
            link(
                "AWS supported models",
                "https://docs.aws.amazon.com/bedrock/latest/userguide/models-supported.html",
            ),
            link(
                "Vizier provider docs",
                "https://www.vizier.rs/docs/configuration/providers#amazon-bedrock",
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_bedrock_has_a_guide_and_it_carries_the_exact_policy() {
        assert!(for_provider(&ProviderId::AMAZON_BEDROCK).is_some());
        assert!(for_provider(&ProviderId::OPENAI).is_none());
        let guide = for_provider(&ProviderId::AMAZON_BEDROCK).unwrap();
        assert_eq!(guide.steps.len(), 5);
        assert_eq!(guide.policy.language, "json");
        assert_eq!(guide.policy.code, BEDROCK_POLICY);
        assert!(guide.policy.code.contains("bedrock:InvokeModel"));
        assert!(guide.policy.code.contains("bedrock:ListFoundationModels"));
        assert!(guide.policy.code.contains("\"Resource\": \"*\""));
        assert_eq!(guide.links.len(), 4);
        assert!(
            guide
                .links
                .iter()
                .any(|l| l.url.ends_with("#amazon-bedrock"))
        );
        let json = serde_json::to_value(&guide).unwrap();
        assert_eq!(json["policy"]["code"], BEDROCK_POLICY);
    }
}
