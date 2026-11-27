//! The public address people reach this vizier at (`base_url`).
//!
//! The bots' `/connect` reply builds its one-click link from it (spec 069); sign-in redirects
//! and push notifications would need the same. The server takes it from `--base-url`,
//! `VIZIER_BASE_URL` or `vizier.base_url` in the seed config; the desktop app from its Desktop
//! settings page. Both go through [`normalize`], so a value is judged the same way everywhere.

/// Checks and tidies a base URL.
///
/// Empty (or only spaces) means "not set" and gives `Ok(None)`. Otherwise it must be an absolute
/// `http` or `https` URL with a host and no query or fragment; a trailing `/` is removed, so a
/// path can be joined onto it with `format!("{base}/...")`.
pub fn normalize(raw: &str) -> Result<Option<String>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let url = url::Url::parse(trimmed)
        .map_err(|e| format!("base URL `{trimmed}` is not a valid URL: {e}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(format!("base URL `{trimmed}` must start with http:// or https://"));
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(format!("base URL `{trimmed}` has no host"));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(format!("base URL `{trimmed}` must not have a query (`?`) or a fragment (`#`)"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(format!("base URL `{trimmed}` must not carry a user name or password"));
    }
    Ok(Some(url.as_str().trim_end_matches('/').to_string()))
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn empty_is_not_set() {
        assert_eq!(normalize("").unwrap(), None);
        assert_eq!(normalize("   ").unwrap(), None);
    }

    #[test]
    fn a_trailing_slash_is_removed() {
        assert_eq!(normalize("https://vizier.example.com/").unwrap().as_deref(), Some("https://vizier.example.com"));
        assert_eq!(normalize(" http://host:9999/vizier/ ").unwrap().as_deref(), Some("http://host:9999/vizier"));
    }

    #[test]
    fn anything_but_a_plain_http_url_is_refused() {
        for bad in [
            "vizier.example.com",
            "ftp://vizier.example.com",
            "https://vizier.example.com/?a=1",
            "https://vizier.example.com/#top",
            "https://user:pw@vizier.example.com",
            "file:///tmp",
        ] {
            assert!(normalize(bad).is_err(), "{bad} should be refused");
        }
    }
}
