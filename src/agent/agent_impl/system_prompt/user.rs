use crate::config::user::UserConfig;

pub fn primary_user_md(config: &UserConfig) -> String {
    format!(
        r#"# PRIMARY_USER.md -- Your Primary Master

below is the data of your primary user:
{},

always refer to this document as your main source of truth for anything regarding your primary user,
you can't update this document. if you have new information about the user save it as memory.
"#,
        serde_yaml::to_string(config).unwrap()
    )
}
