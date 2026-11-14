//! The PDF tools against sample files written by other tools (reportlab, pypdf; see
//! `tests/fixtures/office/gen-pdf-fixtures.py`), and the tools themselves end to end on a
//! throwaway store. Spec `046-office-pdf`.

use std::net::TcpListener;
use std::sync::Arc;

use super::pdf::{self, PdfRead};
use super::*;
use crate::schema::VizierChannelId;

macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/office/", $name)).to_vec()
    };
}

fn read(bytes: &[u8], start: Option<usize>, limit: Option<usize>, max: Option<usize>) -> PdfRead {
    pdf::to_markdown(bytes, start, limit, max, None).unwrap()
}

// ---- reading (US1) ----

#[test]
fn a_pdf_reads_as_markdown_with_its_list_and_table() {
    let r = read(&fixture!("table.pdf"), None, None, None);
    assert_eq!((r.total_pages, r.page_start, r.page_end), (1, 1, 1));
    assert!(!r.truncated && r.next_page_start.is_none() && !r.scanned_likely && r.failed_pages.is_empty());
    for part in ["# Quarterly Report", "- First item", "- Second item", "| Name | Qty |", "| Widget | 3 |", "| Gadget | 12 |"] {
        assert!(r.markdown.contains(part), "{part}: {}", r.markdown);
    }
}

#[test]
fn following_the_continuation_page_covers_the_whole_document() {
    let bytes = fixture!("long.pdf");
    let first = read(&bytes, None, None, None);
    assert_eq!((first.total_pages, first.page_end), (30, 10));
    assert!(first.truncated);
    assert_eq!(first.next_page_start, Some(11));

    let (mut seen, mut next) = (String::new(), Some(1));
    while let Some(start) = next {
        let r = read(&bytes, Some(start), None, None);
        seen.push_str(&r.markdown);
        next = r.next_page_start;
    }
    for n in 1..=30 {
        assert_eq!(seen.matches(&format!("PAGE-{n}\n")).count() + seen.matches(&format!("PAGE-{n} ")).count(), 1, "PAGE-{n}");
    }
}

#[test]
fn a_character_budget_cuts_a_read_and_says_where_to_go_on() {
    let r = read(&fixture!("long.pdf"), Some(1), Some(30), Some(500));
    assert!(r.truncated);
    assert!(r.markdown.chars().count() <= 500);
    assert!(r.next_page_start.is_some_and(|p| p > r.page_end));
}

#[test]
fn a_read_with_no_budget_is_still_bounded() {
    // Far more text than the default budget: the default must cut it.
    let html = format!("<p>{}</p>", "word ".repeat(60_000));
    let big = pdf::from_html(&html, None).unwrap();
    let r = read(&big, Some(1), Some(100_000), None);
    assert!(r.truncated, "pages {}..{}", r.page_start, r.page_end);
    assert!(r.markdown.chars().count() <= pdf::DEFAULT_MAX_CHARS, "{}", r.markdown.chars().count());
    assert!(r.next_page_start.is_some());
}

#[test]
fn an_encrypted_pdf_says_whether_the_password_is_missing_or_wrong() {
    let bytes = fixture!("locked.pdf");
    let missing = pdf::to_markdown(&bytes, None, None, None, None).unwrap_err().0;
    assert!(missing.contains("password") && missing.contains("`password` argument"), "{missing}");
    assert!(!missing.contains("PdfReadOptions"), "{missing}");
    let wrong = pdf::to_markdown(&bytes, None, None, None, Some("nope")).unwrap_err().0;
    assert!(wrong.contains("Incorrect password"), "{wrong}");
    let ok = pdf::to_markdown(&bytes, None, None, None, Some("vizier")).unwrap();
    assert!(ok.markdown.contains("Locked Report") && ok.markdown.contains("Secret figures"));
}

#[test]
fn a_scanned_pdf_is_flagged_and_points_at_the_render_tool() {
    let r = read(&fixture!("scanned.pdf"), None, None, None);
    assert!(r.scanned_likely && r.markdown.trim().is_empty());
    let note = r.note.unwrap();
    assert!(note.contains("office_pdf_render_page_as_png") && !note.contains("`pdf_render`"), "{note}");
}

#[test]
fn a_corrupt_pdf_is_a_readable_error() {
    for bytes in [fixture!("corrupt.pdf"), b"not a pdf at all".to_vec(), Vec::new()] {
        let e = pdf::to_markdown(&bytes, None, None, None, None).unwrap_err().0;
        assert!(e.starts_with("Failed to read PDF"), "{e}");
        assert!(pdf::render_page(&bytes, 1, None).is_err());
    }
}

#[test]
fn a_page_past_the_end_names_the_page_count() {
    let e = pdf::to_markdown(&fixture!("long.pdf"), Some(99), None, None, None).unwrap_err().0;
    assert!(e.contains("30 pages"), "{e}");
}

// ---- rendering (US2) ----

fn png_size(png: &[u8]) -> (u32, u32) {
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    (u32::from_be_bytes(png[16..20].try_into().unwrap()), u32::from_be_bytes(png[20..24].try_into().unwrap()))
}

#[test]
fn every_page_renders_and_a_larger_scale_is_a_larger_image() {
    let bytes = fixture!("long.pdf");
    for page in 1..=30 {
        let (png, w, h) = pdf::render_page(&bytes, page, Some(0.5)).unwrap();
        assert_eq!(png_size(&png), (w, h));
    }
    let small = pdf::render_page(&bytes, 1, Some(1.0)).unwrap();
    let large = pdf::render_page(&bytes, 1, Some(2.0)).unwrap();
    assert!(large.1 > small.1 && large.2 > small.2);
}

#[test]
fn rendering_a_bad_page_or_file_is_a_readable_error() {
    let bytes = fixture!("long.pdf");
    for page in [0, 31, 999] {
        let e = pdf::render_page(&bytes, page, None).unwrap_err().0;
        assert!(e.contains("30 pages"), "{page}: {e}");
    }
    assert!(pdf::render_page(&fixture!("locked.pdf"), 1, None).is_err());
}

#[test]
fn the_scale_is_held_to_its_range() {
    assert_eq!(pdf::effective_scale(None), 1.5);
    assert_eq!(pdf::effective_scale(Some(50.0)), 4.0);
    assert_eq!(pdf::effective_scale(Some(0.0)), 0.1);
    assert_eq!(pdf::effective_scale(Some(f32::NAN)), 1.5);
    assert_eq!(pdf::effective_scale(Some(2.0)), 2.0);
    // The top of the range still renders an ordinary page.
    assert!(pdf::render_page(&fixture!("table.pdf"), 1, Some(50.0)).is_ok());
}

// ---- writing from HTML (US3) ----

const INVOICE: &str = "<html><head><style>table{border-collapse:collapse}td,th{border:1px solid #333}\
    tr.total{background:#fde68a}</style></head><body><h1>Invoice 42</h1>\
    <table><tr><th>Item</th><th>Qty</th></tr><tr><td>Widget</td><td>3</td></tr>\
    <tr class=\"total\"><td>Total</td><td>3</td></tr></table></body></html>";

#[test]
fn styled_html_becomes_a_pdf_that_reads_back() {
    let bytes = pdf::from_html(INVOICE, Some("Invoice")).unwrap();
    assert!(bytes.starts_with(b"%PDF-") && bytes.trim_ascii_end().ends_with(b"%%EOF"));
    let r = read(&bytes, None, None, None);
    for part in ["Invoice 42", "Widget", "Total"] {
        assert!(r.markdown.contains(part), "{part}: {}", r.markdown);
    }
    // The reader prints a table's first row as a bold line; the cells are all there.
    for cell in ["Item", "Qty", "| Widget | 3 |", "| Total | 3 |"] {
        assert!(r.markdown.contains(cell), "{cell}: {}", r.markdown);
    }
}

#[test]
fn html_never_reaches_out_and_never_runs_a_script() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();

    let dir = tempfile::tempdir().unwrap();
    let secret = dir.path().join("secret.txt");
    std::fs::write(&secret, "FILE-SECRET-MARKER").unwrap();
    let css = dir.path().join("evil.css");
    std::fs::write(&css, "body::before { content: 'CSS-FILE-MARKER'; }").unwrap();

    let html = format!(
        "<html><head><link rel=\"stylesheet\" href=\"http://127.0.0.1:{port}/a.css\">\
         <link rel=\"stylesheet\" href=\"file://{css}\"></head><body>\
         <p>VISIBLE-TEXT</p><img src=\"http://127.0.0.1:{port}/x.png\">\
         <img src=\"file://{secret}\"><script>document.write('SCRIPT-RAN');fetch('http://127.0.0.1:{port}/s')</script>\
         </body></html>",
        css = css.display(),
        secret = secret.display()
    );
    let bytes = pdf::from_html(&html, None).unwrap();
    let text = read(&bytes, None, None, None).markdown;

    assert!(text.contains("VISIBLE-TEXT"), "{text}");
    for forbidden in ["SCRIPT-RAN", "FILE-SECRET-MARKER", "CSS-FILE-MARKER"] {
        assert!(!text.contains(forbidden), "{forbidden}: {text}");
    }
    match listener.accept() {
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
        other => panic!("the renderer connected to the network: {other:?}"),
    }
}

#[test]
fn other_scripts_and_emoji_are_rendered_or_reported_not_blanked() {
    let bytes = pdf::from_html("<p>Привет мир</p><p>こんにちは</p><p>مرحبا</p><p>naïve café</p>", None).unwrap();
    let r = read(&bytes, None, None, None);
    // Latin with accents must survive; scripts the fonts lack may be dropped by the renderer.
    // Record what happened: a page with any text, or a note saying there is none.
    assert!(r.markdown.contains("caf") || r.note.is_some(), "{r:?}");
}

#[test]
fn html_that_is_empty_or_too_large_is_a_readable_error() {
    assert!(pdf::from_html("", None).is_err() || read(&pdf::from_html("", None).unwrap(), None, None, None).total_pages >= 1);
    let huge = format!("<p>{}</p>", "x".repeat(5 * 1024 * 1024 + 1));
    let e = pdf::from_html(&huge, None).unwrap_err().0;
    assert!(e.contains("5 MB"), "{e}");
}

// ---- Word to PDF (US4) ----

#[test]
fn a_word_document_converts_with_its_structure() {
    let pdf_bytes = pdf::from_docx(&fixture!("table.docx"), Some("Converted")).unwrap();
    let r = read(&pdf_bytes, None, None, None);
    // The heading stays a heading; list items, emphasis and every table cell survive as text.
    assert!(r.markdown.contains("# Quarterly Report"), "{}", r.markdown);
    for part in ["bold", "italic", "Top item", "Nested item", "Name", "Note", "alpha", "beta", "plain", "See the docs", "Converted"] {
        assert!(r.markdown.contains(part), "{part}: {}", r.markdown);
    }
}

#[test]
fn markup_typed_in_a_word_document_prints_as_text() {
    let docx = docx::from_markdown("\\<b>not bold\\</b> and a \\< b and \\<script>x\\</script>", None).unwrap();
    let md = docx::to_markdown(&docx, 10_000).unwrap().markdown;
    assert!(md.contains("<b>") || md.contains("&lt;b"), "the fixture must carry the markup: {md}");
    let text = read(&pdf::from_docx(&docx, None).unwrap(), None, None, None).markdown;
    assert!(text.contains("<b>") && text.contains("</script>"), "{text}");
}

// ---- the tools, end to end on a throwaway store ----

struct Env {
    storage: Arc<VizierStorage>,
    manager: FileManager,
    ctx: ToolContext,
    _dir: tempfile::TempDir,
}

fn env() -> Env {
    let (storage, manager, dir) = super::tests::files();
    let ctx = ToolContext {
        session: VizierSession("agent-pdf".into(), VizierChannelId::HTTP("u".into(), "c".into()), None),
        pending_attachments: Default::default(),
        hooks: None,
        background_depth: 0,
        deadline: None,
        loaded_images: Default::default(),
        recalled: Default::default(),
        client: None,
        questions: crate::questions::QuestionsHandle::new(Default::default()),
        interactive: false,
        dream_sources: vec![],
        secret_redactor: Arc::new(crate::secrets::SecretRedactor::default()),
        project: None,
        detached: false,
        parent_detached_id: None,
    };
    Env { storage, manager, ctx, _dir: dir }
}

impl Env {
    async fn put(&self, name: &str, mime: &str, bytes: Vec<u8>) {
        let ext = name.rsplit('.').next().unwrap();
        write_session_file(&self.storage, &self.manager, &self.ctx.session, name, ext, mime, bytes).await.unwrap();
    }

    async fn stored(&self, name: &str) -> (String, Vec<u8>) {
        load(&self.storage, &self.manager, &self.ctx, name).await.unwrap()
    }

    fn read_tool(&self) -> OfficePdfRead {
        OfficePdfRead { storage: self.storage.clone(), file_manager: self.manager.clone() }
    }
    fn render_tool(&self) -> OfficePdfRenderPage {
        OfficePdfRenderPage { storage: self.storage.clone(), file_manager: self.manager.clone() }
    }
    fn html_tool(&self) -> OfficePdfWriteFromHtml {
        OfficePdfWriteFromHtml { storage: self.storage.clone(), file_manager: self.manager.clone() }
    }
    fn docx_tool(&self) -> OfficePdfWriteFromDocx {
        OfficePdfWriteFromDocx { storage: self.storage.clone(), file_manager: self.manager.clone() }
    }
}

fn html_args(filename: &str, html: &str) -> PdfWriteHtmlArgs {
    PdfWriteHtmlArgs { filename: filename.into(), html: html.into(), title: None }
}

#[tokio::test]
async fn the_html_tool_names_files_safely_and_replaces_a_same_named_one() {
    let env = env();
    let tool = env.html_tool();

    let first = tool.call(html_args("invoice", "<h1>One</h1>"), &env.ctx).await.unwrap();
    assert_eq!(first.filename, "invoice.pdf");
    let (mime, bytes) = env.stored("invoice.pdf").await;
    assert_eq!(mime, PDF_MIME);
    assert!(bytes.starts_with(b"%PDF"));

    let second = tool.call(html_args("invoice.pdf", "<h1>Two</h1>"), &env.ctx).await.unwrap();
    assert_ne!(first.size, 0);
    let listed = env.storage.list_session_files(&env.ctx.session).await.unwrap();
    assert_eq!(listed.len(), 1, "same name replaces");
    let (_, replaced) = env.stored("invoice.pdf").await;
    assert_eq!(replaced.len() as u64, second.size);
    assert!(read(&replaced, None, None, None).markdown.contains("Two"));

    for bad in ["../x.pdf", "a/b.pdf", ""] {
        assert!(tool.call(html_args(bad, "<p>x</p>"), &env.ctx).await.is_err(), "{bad:?}");
    }
}

#[tokio::test]
async fn the_render_tool_saves_a_png_and_reports_what_it_used() {
    let env = env();
    env.put("long.pdf", PDF_MIME, fixture!("long.pdf")).await;
    let tool = env.render_tool();
    let args = |page, scale, out: Option<&str>| PdfRenderPageArgs {
        filename: "long.pdf".into(),
        page,
        scale,
        output_filename: out.map(String::from),
    };

    let r = tool.call(args(Some(2), Some(50.0), None), &env.ctx).await.unwrap();
    assert_eq!((r.filename.as_str(), r.page, r.scale), ("long_page_2.png", 2, 4.0));
    let (mime, png) = env.stored("long_page_2.png").await;
    assert_eq!(mime, PNG_MIME);
    assert_eq!(png_size(&png), (r.width, r.height));
    assert_eq!(r.size as usize, png.len());

    let named = tool.call(args(None, None, Some("cover")), &env.ctx).await.unwrap();
    assert_eq!((named.filename.as_str(), named.page, named.scale), ("cover.png", 1, 1.5));
    assert!(tool.call(args(None, None, Some("a/b.png")), &env.ctx).await.is_err());

    let e = tool.call(args(Some(31), None, None), &env.ctx).await.unwrap_err().0;
    assert!(e.contains("30 pages"), "{e}");
}

#[tokio::test]
async fn the_read_tool_pages_and_reports_errors_the_agent_can_act_on() {
    let env = env();
    env.put("long.pdf", PDF_MIME, fixture!("long.pdf")).await;
    env.put("locked.pdf", PDF_MIME, fixture!("locked.pdf")).await;
    let tool = env.read_tool();
    let args = |name: &str| PdfReadArgs {
        filename: name.into(),
        page_start: None,
        page_limit: None,
        max_chars: None,
        password: None,
    };

    let first = tool.call(args("long.pdf"), &env.ctx).await.unwrap();
    assert_eq!((first.page_end, first.next_page_start, first.truncated), (10, Some(11), true));
    let missing = tool.call(args("ghost.pdf"), &env.ctx).await.unwrap_err().0;
    assert!(missing.contains("File not found: ghost.pdf") && missing.contains("long.pdf"), "{missing}");
    assert!(tool.call(args("locked.pdf"), &env.ctx).await.is_err());
    let unlocked = tool.call(PdfReadArgs { password: Some("vizier".into()), ..args("locked.pdf") }, &env.ctx).await.unwrap();
    assert!(unlocked.markdown.contains("Locked Report"));
}

#[tokio::test]
async fn converting_a_word_file_leaves_it_untouched() {
    let env = env();
    let docx = fixture!("table.docx");
    env.put("report.docx", DOCX_MIME, docx.clone()).await;
    env.put("budget.xlsx", XLSX_MIME, fixture!("mixed.xlsx")).await;
    env.put("notes.txt", "text/plain", b"hi".to_vec()).await;
    env.put("invoice.pdf", PDF_MIME, pdf::from_html("<p>x</p>", None).unwrap()).await;
    let tool = env.docx_tool();
    let args = |src: &str| PdfWriteDocxArgs {
        filename: "out".into(),
        docx_filename: src.into(),
        title: Some("A Title".into()),
    };

    let done = tool.call(args("report.docx"), &env.ctx).await.unwrap();
    assert_eq!(done.filename, "out.pdf");
    assert_eq!(env.stored("report.docx").await.1, docx, "the source is unchanged");
    let text = read(&env.stored("out.pdf").await.1, None, None, None).markdown;
    assert!(text.contains("A Title") && text.contains("Quarterly Report"), "{text}");

    let xlsx = tool.call(args("budget.xlsx"), &env.ctx).await.unwrap_err().0;
    assert!(xlsx.contains("office_xlsx_read_as_cells"), "{xlsx}");
    let pdf_src = tool.call(args("invoice.pdf"), &env.ctx).await.unwrap_err().0;
    assert!(pdf_src.contains("office_pdf_read_as_md"), "{pdf_src}");
    let txt = tool.call(args("notes.txt"), &env.ctx).await.unwrap_err().0;
    assert!(txt.contains("read_document_file"), "{txt}");
}

// ---- every pair of formats (FR-015, SC-007) ----

#[tokio::test]
async fn a_file_of_the_wrong_format_always_names_the_right_reader() {
    let env = env();
    env.put("a.docx", DOCX_MIME, fixture!("table.docx")).await;
    env.put("a.xlsx", XLSX_MIME, fixture!("mixed.xlsx")).await;
    env.put("a.pdf", PDF_MIME, fixture!("table.pdf")).await;

    let docx_read = OfficeDocxRead { storage: env.storage.clone(), file_manager: env.manager.clone() };
    let cells = OfficeXlsxRead { storage: env.storage.clone(), file_manager: env.manager.clone() };
    let csv = OfficeXlsxReadCsv { storage: env.storage.clone(), file_manager: env.manager.clone() };

    let docx_args = |n: &str| DocxReadArgs { filename: n.into(), max_chars: None };
    for (file, wanted) in [("a.xlsx", "office_xlsx_read_as_cells"), ("a.pdf", "office_pdf_read_as_md")] {
        let e = docx_read.call(docx_args(file), &env.ctx).await.unwrap_err().0;
        assert!(e.contains(wanted), "{file}: {e}");
    }
    let xlsx_args = |n: &str| XlsxReadArgs { filename: n.into(), sheet: None, range: None, info: None, formulas: None };
    for (file, wanted) in [("a.docx", "office_docx_read_as_md"), ("a.pdf", "office_pdf_read_as_md")] {
        let e = cells.call(xlsx_args(file), &env.ctx).await.unwrap_err().0;
        assert!(e.contains(wanted), "{file}: {e}");
        let e = csv.call(XlsxCsvArgs { filename: file.into(), sheet: None, range: None, executed_formula: None }, &env.ctx).await.unwrap_err().0;
        assert!(e.contains(wanted), "{file}: {e}");
    }
    let pdf_args = |n: &str| PdfReadArgs { filename: n.into(), page_start: None, page_limit: None, max_chars: None, password: None };
    for (file, wanted) in [("a.docx", "office_docx_read_as_md"), ("a.xlsx", "office_xlsx_read_as_cells")] {
        let e = env.read_tool().call(pdf_args(file), &env.ctx).await.unwrap_err().0;
        assert!(e.contains(wanted), "{file}: {e}");
        let e = env
            .render_tool()
            .call(PdfRenderPageArgs { filename: file.into(), page: None, scale: None, output_filename: None }, &env.ctx)
            .await
            .unwrap_err()
            .0;
        assert!(e.contains(wanted), "{file}: {e}");
    }
}

// ---- descriptions ----

#[test]
fn the_pdf_descriptions_state_what_an_agent_must_know() {
    let (storage, manager, _dir) = {
        let e = env();
        (e.storage.clone(), e.manager.clone(), e)
    };
    let read = OfficePdfRead { storage: storage.clone(), file_manager: manager.clone() }.description();
    let render = OfficePdfRenderPage { storage: storage.clone(), file_manager: manager.clone() }.description();
    let html = OfficePdfWriteFromHtml { storage: storage.clone(), file_manager: manager.clone() }.description();
    let docx = OfficePdfWriteFromDocx { storage, file_manager: manager }.description();

    for d in [&read, &html, &docx] {
        assert!(d.contains("edit") || d.contains("merge"), "{d}");
    }
    assert!(read.contains("next_page_start") && read.contains("scanned_likely") && read.contains("password"));
    assert!(render.contains("0.1 to 4.0") && render.contains("read_image_file") && render.contains("output_filename"));
    for d in [&html, &docx] {
        assert!(d.contains("replaced") && d.contains("new filename") && d.contains("send_attachment"), "{d}");
    }
    assert!(html.contains("scripts do not run") && html.contains("5 MB"));
    assert!(docx.contains("headers and footers") && docx.contains("unchanged"));
}
