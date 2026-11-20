//! The document an agent page runs in (`specs/063-html-generative-ui` research R7a, R8).
//!
//! A page is not handed to the browser as `srcdoc`: a `srcdoc` document inherits the CSP of the
//! page that embeds it, and the desktop app's CSP allows no inline script. It is served from
//! `/api/v1/ui-frames/{ticket}` instead, as a document of its own with its own policy, sent as a
//! header so it can carry `sandbox allow-scripts` (an opaque origin: no cookies, no storage, no
//! reach into the WebUI).
//!
//! The frame loads nothing over the network: the kit's stylesheet, its runtime (the bridge, the
//! error hooks, the library loader), its elements and any library the page imports are inlined
//! here from the WebUI build (`ui-kit/manifest.json`). A build without the kit still serves the
//! page, without them.

use serde::Deserialize;

/// The frame's policy. Scripts may run inline or from `blob:` (the library loader and
/// `vizier.worker` make blob URLs); nothing may be fetched; images, audio and video may come from
/// any `https` address (spec FR-015a). `sandbox allow-scripts` gives the document an opaque
/// origin whatever the embedding element says.
pub const FRAME_CSP: &str = "default-src 'none'; script-src 'unsafe-inline' blob:; \
     style-src 'unsafe-inline'; img-src data: blob: https:; media-src data: blob: https:; \
     font-src data:; connect-src 'none'; worker-src blob:; frame-src 'none'; \
     form-action 'none'; base-uri 'none'; sandbox allow-scripts";

/// `ui-kit/manifest.json`, as the kit build writes it.
#[derive(Debug, Default, Deserialize)]
pub struct KitManifest {
    #[serde(default)]
    pub files: KitFiles,
    /// Import name to file under `ui-kit/`, e.g. `three` → `lib/three-0.186.1.js`.
    #[serde(default)]
    pub libraries: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct KitFiles {
    pub css: Option<String>,
    pub js: Option<String>,
    pub runtime: Option<String>,
}

/// The kit's files, read through `read` (a path under `ui-kit/`).
pub struct Kit<'a> {
    pub manifest: KitManifest,
    pub read: &'a dyn Fn(&str) -> Option<String>,
}

/// Text that is safe inside a `<script>` or `<style>` element: `</script` and `</style` would end
/// it early. `<\/` means the same inside a JS string, regex or comment, which is the only place
/// such text appears in built code.
fn inline_safe(text: &str) -> String {
    text.replace("</script", "<\\/script")
        .replace("</SCRIPT", "<\\/SCRIPT")
        .replace("</style", "<\\/style")
}

/// Whether the page imports `name` as a module (`from "three"`, `import("three")`,
/// `import "three"`).
fn imports(html: &str, name: &str) -> bool {
    let quoted = [format!("\"{name}\""), format!("'{name}'")];
    quoted.iter().any(|q| {
        html.match_indices(q.as_str()).any(|(at, _)| {
            let before = html[..at].trim_end();
            before.ends_with("from") || before.ends_with("import") || before.ends_with("import(")
        })
    })
}

/// JSON that is safe inside a `<script>` element: every `<` is escaped, so no `</script` can appear.
fn script_json(value: &serde_json::Value) -> String {
    value.to_string().replace('<', "\\u003c")
}

/// The whole frame document for `html`.
/// `boot` is what the runtime starts with (`window.__vizierBoot`: the page id, the version and the
/// saved state), so `vizier.state.get()` has the state before any page script asks.
pub fn document(html: &str, kit: Option<&Kit>, boot: &serde_json::Value) -> String {
    let mut head = String::from(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">",
    );
    head.push_str("<script>window.__vizierBoot=");
    head.push_str(&script_json(boot));
    head.push_str("</script>");
    if let Some(kit) = kit {
        if let Some(css) = kit.manifest.files.css.as_deref().and_then(|f| (kit.read)(f)) {
            head.push_str("<style data-vz-kit>");
            head.push_str(&inline_safe(&css));
            head.push_str("</style>");
        }
        // Libraries before the runtime: it turns these blocks into blob URLs and an import map
        // before the page's own module scripts are parsed.
        for (name, file) in &kit.manifest.libraries {
            if !imports(html, name) {
                continue;
            }
            if let Some(source) = (kit.read)(file) {
                head.push_str(&format!("<script type=\"text/plain\" data-vz-lib=\"{name}\">"));
                head.push_str(&inline_safe(&source));
                head.push_str("</script>");
            }
        }
        if let Some(runtime) = kit.manifest.files.runtime.as_deref().and_then(|f| (kit.read)(f)) {
            head.push_str("<script data-vz-runtime>");
            head.push_str(&inline_safe(&runtime));
            head.push_str("</script>");
        }
        if html.contains("<vz-")
            && let Some(js) = kit.manifest.files.js.as_deref().and_then(|f| (kit.read)(f))
        {
            head.push_str("<script data-vz-kit>");
            head.push_str(&inline_safe(&js));
            head.push_str("</script>");
        }
    }
    // A page that is a full document merges into this head and body, as the HTML parser does.
    format!("{head}</head><body>{html}</body></html>")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kit(read: &dyn Fn(&str) -> Option<String>) -> Kit<'_> {
        Kit {
            manifest: serde_json::from_str(
                r#"{"files":{"css":"kit.css","js":"kit.js","runtime":"rt.js"},"libraries":{"three":"lib/three.js"}}"#,
            )
            .unwrap(),
            read,
        }
    }

    fn files(path: &str) -> Option<String> {
        Some(match path {
            "kit.css" => "body{color:red}".into(),
            "kit.js" => "customElements.define('vz-x', class extends HTMLElement{})".into(),
            "rt.js" => "window.vizier={version:1};var s='</script>'".into(),
            "lib/three.js" => "export const REVISION='186'".into(),
            _ => return None,
        })
    }

    #[test]
    fn the_policy_sandboxes_and_blocks_the_network() {
        for part in ["sandbox allow-scripts", "connect-src 'none'", "img-src data: blob: https:", "default-src 'none'"] {
            assert!(FRAME_CSP.contains(part), "{part}");
        }
        assert!(!FRAME_CSP.contains("http:"), "plain http media is refused");
        assert!(!FRAME_CSP.contains("allow-same-origin"));
    }

    #[test]
    fn a_plain_page_gets_the_stylesheet_and_runtime_only() {
        let doc = document("<h1>Hi</h1>", Some(&kit(&files)), &boot());
        assert!(doc.contains("<style data-vz-kit>body{color:red}</style>"));
        assert!(doc.contains("<script data-vz-runtime>"));
        assert!(!doc.contains("customElements.define"), "no <vz-*> tag, no element script");
        assert!(!doc.contains("data-vz-lib"), "no import, no library");
        assert!(doc.ends_with("<body><h1>Hi</h1></body></html>"));
    }

    #[test]
    fn kit_tags_and_imports_bring_their_scripts() {
        let page = "<vz-stat label=a></vz-stat><script type=module>import * as THREE from \"three\"</script>";
        let doc = document(page, Some(&kit(&files)), &boot());
        assert!(doc.contains("customElements.define"));
        assert!(doc.contains("<script type=\"text/plain\" data-vz-lib=\"three\">export const REVISION='186'</script>"));
        assert!(doc.find("data-vz-lib").unwrap() < doc.find("data-vz-runtime").unwrap(), "libraries precede the runtime");
    }

    #[test]
    fn inlined_code_cannot_close_its_element() {
        let doc = document("<p>x</p>", Some(&kit(&files)), &boot());
        assert!(doc.contains("var s='<\\/script>'"));
    }

    #[test]
    fn import_detection_needs_an_import() {
        assert!(imports("import * as T from 'three'", "three"));
        assert!(imports("await import(\"three\")", "three"));
        assert!(imports("import \"three\"", "three"));
        assert!(!imports("<p>I like \"three\" cats</p>", "three"));
    }

    fn boot() -> serde_json::Value {
        serde_json::json!({ "pageId": "pg_1", "version": 2, "state": { "note": "</script><b>" } })
    }

    #[test]
    fn without_a_kit_the_page_is_still_served_with_its_boot_data() {
        let doc = document("<p>x</p>", None, &boot());
        assert!(doc.contains("<script>window.__vizierBoot={"));
        assert!(!doc.contains("</script><b>"), "the state cannot close its script: {doc}");
        assert!(doc.ends_with("<body><p>x</p></body></html>"));
    }

}
