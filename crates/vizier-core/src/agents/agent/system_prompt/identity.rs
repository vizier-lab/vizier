use crate::constant::CORE_MD;
use crate::schema::{CORE_BUNDLE, SOUL_PATH};

/// The identity section of the system prompt: the agent's own memory, addressed
/// (specs/043-unify-core-and-memory, FR-006, FR-009).
///
/// Built from the document **alone**. No timestamp, no `updated_at`, no per-turn data — so the
/// block is byte-identical across consecutive turns and the prompt prefix carrying it stays
/// cacheable (FR-007, SC-002). That is the whole reason this is a pure function of the stored
/// text rather than a method that could reach for anything else.
///
/// `stored` is the document's content, or `None` when nothing is there. An absent or blank
/// document renders the embedded template plus a note naming the address, never an empty
/// block: an agent must never see a turn with no identity (FR-008, SC-003), and the note is
/// how it learns it can write itself back (research Decision 10).
pub fn identity_md(stored: Option<&str>) -> String {
    let address = format!(r#"<memory bundle="{CORE_BUNDLE}" path="{SOUL_PATH}">"#);

    match stored.map(str::trim).filter(|content| !content.is_empty()) {
        Some(content) => format!("{address}\n{content}\n</memory>"),
        None => format!(
            "{address}\n{}\n</memory>\n\n\
             This is the default identity, shown because no document exists at \
             {CORE_BUNDLE}/{SOUL_PATH}. It is a memory like any other: write it with \
             `memory_write` to make it yours.",
            CORE_MD.trim()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_identity_is_carried_in_an_addressed_memory_block() {
        // FR-009: the agent can tell this is a memory it can also write, without being told
        // separately and without searching for it.
        let block = identity_md(Some("# I am Oz\n\nA software engineer."));
        assert!(block.starts_with(r#"<memory bundle="core" path="soul">"#));
        assert!(block.ends_with("</memory>"));
        assert!(block.contains("I am Oz"));
    }

    #[test]
    fn the_wrapper_is_byte_identical_for_the_same_document() {
        // FR-007, SC-002: two consecutive turns carrying the same document produce the same
        // bytes, so the cacheable prefix survives. Nothing per-turn may leak in.
        let stored = "# I am Oz\n\nValues: verify before modify.";
        assert_eq!(identity_md(Some(stored)), identity_md(Some(stored)));
    }

    #[test]
    fn an_absent_document_is_a_default_identity_never_an_empty_block() {
        // FR-008, SC-003: a removed soul must not leave a turn with no identity.
        for absent in [None, Some(""), Some("   \n\t\n")] {
            let block = identity_md(absent);
            assert!(
                block.starts_with(r#"<memory bundle="core" path="soul">"#),
                "the block is still addressed"
            );
            assert!(
                block.contains("## Operating Framework"),
                "it carries the template, not nothing"
            );
            assert!(
                block.contains("no document exists at core/soul"),
                "and it says the document can be written at that address"
            );
            assert_ne!(block, identity_md(Some("# something")));
        }
    }

    #[test]
    fn the_default_identity_still_names_the_address_it_can_be_written_to() {
        // The discovery fix for the tool that used to be called `read_core` (FR-010).
        let block = identity_md(None);
        assert!(block.contains("core/soul"));
        assert!(block.contains("memory_write"));
    }

    #[test]
    fn a_document_with_surrounding_whitespace_is_not_treated_as_absent() {
        let block = identity_md(Some("  \n# Real identity\n  "));
        assert!(block.contains("# Real identity"));
        assert!(!block.contains("default identity"), "it is a real identity");
    }
}
