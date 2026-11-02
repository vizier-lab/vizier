//! Static in-form setup guides, as provider data on the catalogue listing (research R10,
//! contract `provider-credentials-api.md` §1). The form renders whatever guide a provider has;
//! there is no provider-id branch in the WebUI.

use serde::Serialize;

use crate::config::provider::ProviderId;

/// One numbered step of a guide.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GuideStep {
    pub title: String,
    pub body: String,
}

/// A copyable policy or snippet.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GuidePolicy {
    pub language: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GuideLink {
    pub label: String,
    pub url: String,
}

/// What the connect form shows above the fields. Static metadata, never stored or editable.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProviderGuide {
    pub intro: String,
    pub steps: Vec<GuideStep>,
    pub policy: GuidePolicy,
    pub links: Vec<GuideLink>,
}

/// The exact minimal policy: `ListFoundationModels` is the Test action's read permission, and
/// `InvokeModel` runs chat, embeddings and images. Copied verbatim by the form and the docs.
pub const BEDROCK_POLICY: &str = "{\n  \"Version\": \"2012-10-17\",\n  \"Statement\": [{\n    \"Effect\": \"Allow\",\n    \"Action\": [\"bedrock:InvokeModel\", \"bedrock:ListFoundationModels\"],\n    \"Resource\": \"*\"\n  }]\n}";

/// The SSO connection's policy: `bedrock:InvokeModel` is all a signed-in role needs, and the
/// listing permission is optional here because the portal already lists accounts and roles.
/// Copied verbatim by the form and the docs.
pub const BEDROCK_SSO_POLICY: &str = "{\n  \"Version\": \"2012-10-17\",\n  \"Statement\": [{\n    \"Effect\": \"Allow\",\n    \"Action\": [\"bedrock:InvokeModel\"],\n    \"Resource\": \"*\"\n  }]\n}";

/// The guide for a provider, if it has one: the static-key guide for `amazon-bedrock`, its own
/// SSO guide for `amazon-bedrock-sso`, nothing else (FR-001a, FR-038). Each Bedrock id answers
/// with its own guide — never the other's, never none.
pub fn for_provider(id: &ProviderId) -> Option<ProviderGuide> {
    if *id == ProviderId::AMAZON_BEDROCK {
        return Some(bedrock());
    }
    if *id == ProviderId::AMAZON_BEDROCK_SSO {
        return Some(bedrock_sso());
    }
    None
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

fn bedrock_sso() -> ProviderGuide {
    ProviderGuide {
        intro: "Sign in with your organisation's IAM Identity Center account instead of an access \
                key pair. Vizier asks the portal for a role credential and renews it itself."
            .into(),
        steps: vec![
            step(
                "Find the Identity Center start URL",
                "Use the portal URL your organisation gave you. It looks like \
                 https://<subdomain>.awsapps.com/start.",
            ),
            step(
                "Choose the SSO region",
                "This is the region your Identity Center instance lives in, e.g. us-east-1. It \
                 is often not the region your models run in.",
            ),
            step(
                "Sign in and pick account + role",
                "Press Sign in, confirm the code in a browser on any machine, then choose the \
                 account and the role you were given. Vizier never sees your password.",
            ),
            step(
                "Attach the minimal policy",
                "Allow bedrock:InvokeModel on the role. Listing permissions are optional: the \
                 model list comes from the catalogue, not from your account.",
            ),
            step(
                "Add your inference profiles, and price them",
                "Paste each ARN and name it, then pick the model it runs from the searchable \
                 Bedrock list so its usage is costed. This is optional: an entry with nothing \
                 picked still chats, shown as \u{201c}pricing not set\u{201d}.",
            ),
            step(
                "Test the connection",
                "Press Test: one small billable call to the model or profile the test names, so \
                 a bad policy or a missing model shows up here rather than mid-conversation.",
            ),
        ],
        policy: GuidePolicy {
            language: "json".into(),
            code: BEDROCK_SSO_POLICY.into(),
        },
        links: vec![
            link(
                "IAM Identity Center",
                "https://console.aws.amazon.com/singlesignon/home",
            ),
            link(
                "Bedrock model access",
                "https://console.aws.amazon.com/bedrock/home#/modelaccess",
            ),
            link(
                "Application inference profiles",
                "https://docs.aws.amazon.com/bedrock/latest/userguide/inference-profiles.html",
            ),
            link(
                "Vizier provider docs",
                "https://www.vizier.rs/docs/configuration/providers#amazon-bedrock-sso",
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

    #[test]
    fn each_bedrock_id_answers_with_its_own_guide() {
        let sso = for_provider(&ProviderId::AMAZON_BEDROCK_SSO).expect("the SSO id has a guide");
        assert_eq!(sso.steps.len(), 6);
        assert_eq!(sso.steps[0].title, "Find the Identity Center start URL");
        assert_eq!(sso.policy.language, "json");
        assert_eq!(sso.policy.code, BEDROCK_SSO_POLICY);
        assert!(sso.policy.code.contains("bedrock:InvokeModel"));
        assert!(
            !sso.policy.code.contains("ListFoundationModels"),
            "the SSO policy is InvokeModel only"
        );
        assert_eq!(sso.links.len(), 4);
        assert!(sso.links.iter().any(|l| l.url.ends_with("#amazon-bedrock-sso")));

        // Neither id reports the other's guide (FR-001a, FR-038).
        let static_key = for_provider(&ProviderId::AMAZON_BEDROCK).unwrap();
        assert_ne!(static_key.intro, sso.intro);
        assert_ne!(static_key.policy.code, sso.policy.code);
        assert_ne!(static_key.steps[0].title, sso.steps[0].title);
        assert!(!static_key.links.iter().any(|l| l.url.ends_with("#amazon-bedrock-sso")));
        assert!(!sso.links.iter().any(|l| l.url.ends_with("#amazon-bedrock")));

        // Every other provider still has none.
        for id in [ProviderId::OLLAMA, ProviderId::OPENAI, ProviderId::CUSTOM] {
            assert!(for_provider(&id).is_none(), "{id} has no guide");
        }
    }
}
