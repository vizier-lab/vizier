//! Reading Word documents (`.docx`) as Markdown.
//!
//! A pure function on bytes (spec `034-office-addon`, FR-002): headings, bold and italic,
//! ordered and unordered nested lists, tables as pipe tables and hyperlinks. Images, shapes,
//! text boxes and footnotes are skipped.

use std::collections::HashMap;

use docx_rs::{
    DocumentChild, Docx, Hyperlink, HyperlinkData, Paragraph, ParagraphChild, Run, RunChild,
    Table, TableCellContent, TableChild, TableRowChild,
};

use super::DocxRead;
use crate::error::VizierError;

/// Word's document as Markdown, cut at `max_chars` characters.
pub fn to_markdown(bytes: &[u8], max_chars: usize) -> Result<DocxRead, VizierError> {
    let docx = docx_rs::read_docx(bytes).map_err(|e| {
        VizierError(format!(
            "This is not a valid Word file (it may be corrupt or password-protected): {e}"
        ))
    })?;
    let links: HashMap<&str, &str> = docx
        .hyperlinks
        .iter()
        .map(|(id, target, _)| (id.as_str(), target.as_str()))
        .collect();
    let lists = Lists::new(&docx);

    let mut counters = Counters::default();
    let mut blocks: Vec<String> = Vec::new();
    for child in &docx.document.children {
        match child {
            DocumentChild::Paragraph(p) => {
                if let Some(block) = paragraph_block(p, &links, &lists, &mut counters) {
                    blocks.push(block);
                }
            }
            DocumentChild::Table(table) => {
                counters = Counters::default();
                blocks.push(table_block(table, &links));
            }
            _ => {}
        }
    }
    Ok(cut(join_blocks(&blocks), max_chars))
}

/// Blocks are separated by a blank line, except consecutive list items, which stay together.
fn join_blocks(blocks: &[String]) -> String {
    let mut out = String::new();
    let mut previous_was_item = false;
    for block in blocks {
        let is_item = block.trim_start().starts_with("- ")
            || block.trim_start().chars().next().is_some_and(|c| c.is_ascii_digit())
                && block.trim_start().split_once(". ").is_some_and(|(n, _)| n.chars().all(|c| c.is_ascii_digit()));
        if !out.is_empty() {
            out.push_str(if is_item && previous_was_item { "\n" } else { "\n\n" });
        }
        out.push_str(block);
        previous_was_item = is_item;
    }
    out.push('\n');
    out
}

fn cut(markdown: String, max_chars: usize) -> DocxRead {
    let total = markdown.chars().count();
    if total <= max_chars {
        return DocxRead { markdown, truncated: false, note: None };
    }
    let kept: String = markdown.chars().take(max_chars).collect();
    DocxRead {
        markdown: kept,
        truncated: true,
        note: Some(format!(
            "The document was cut at {max_chars} characters of {total}. Ask for a larger `max_chars` (up to the tool's limit) to read more."
        )),
    }
}

/// How a paragraph's list numbering is described in the file: the abstract numbering's format
/// per level, keyed by the paragraph's numbering id.
struct Lists {
    /// `num id` to the abstract numbering id.
    nums: HashMap<usize, usize>,
    /// `(abstract id, level)` to the format name (`bullet`, `decimal`, ...).
    formats: HashMap<(usize, usize), String>,
}

impl Lists {
    fn new(docx: &Docx) -> Self {
        let nums = docx
            .numberings
            .numberings
            .iter()
            .map(|n| (n.id, n.abstract_num_id))
            .collect();
        let formats = docx
            .numberings
            .abstract_nums
            .iter()
            .flat_map(|a| a.levels.iter().map(move |l| ((a.id, l.level), l.format.val.clone())))
            .collect();
        Self { nums, formats }
    }

    fn is_bullet(&self, num_id: usize, level: usize) -> bool {
        self.nums
            .get(&num_id)
            .and_then(|abs| self.formats.get(&(*abs, level)))
            .is_none_or(|format| format == "bullet" || format == "none")
    }
}

/// Item counters for ordered lists, per numbering id and level; a shallower item restarts the
/// deeper levels.
#[derive(Default)]
struct Counters(HashMap<(usize, usize), usize>);

impl Counters {
    fn next(&mut self, num_id: usize, level: usize) -> usize {
        self.0.retain(|(id, l), _| *id != num_id || *l <= level);
        let n = self.0.entry((num_id, level)).or_insert(0);
        *n += 1;
        *n
    }
}

fn heading_level(style: &str) -> Option<usize> {
    let compact: String = style.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
    if compact == "title" {
        return Some(1);
    }
    let digits = compact.strip_prefix("heading")?;
    digits.parse::<usize>().ok().filter(|n| (1..=6).contains(n))
}

fn paragraph_block(
    p: &Paragraph,
    links: &HashMap<&str, &str>,
    lists: &Lists,
    counters: &mut Counters,
) -> Option<String> {
    let text = inline(&p.children, links);
    let style = p.property.style.as_ref().map(|s| s.val.as_str()).unwrap_or("");
    if let Some(numbering) = &p.property.numbering_property {
        if let Some(id) = numbering.id.as_ref().map(|i| i.id).filter(|id| *id != 0) {
            let level = numbering.level.as_ref().map(|l| l.val).unwrap_or(0);
            if text.trim().is_empty() {
                return None;
            }
            let indent = "  ".repeat(level);
            let marker = if lists.is_bullet(id, level) {
                "-".to_string()
            } else {
                format!("{}.", counters.next(id, level))
            };
            return Some(format!("{indent}{marker} {}", text.trim()));
        }
    }
    if text.trim().is_empty() {
        return None;
    }
    match heading_level(style) {
        Some(n) => Some(format!("{} {}", "#".repeat(n), text.trim())),
        None => Some(text.trim().to_string()),
    }
}

fn inline(children: &[ParagraphChild], links: &HashMap<&str, &str>) -> String {
    let mut out = String::new();
    for child in children {
        match child {
            ParagraphChild::Run(run) => out.push_str(&run_text(run)),
            ParagraphChild::Hyperlink(link) => out.push_str(&link_text(link, links)),
            _ => {}
        }
    }
    out
}

fn link_text(link: &Hyperlink, links: &HashMap<&str, &str>) -> String {
    let label = inline(&link.children, links);
    let target = match &link.link {
        HyperlinkData::External { rid, path } => links
            .get(rid.as_str())
            .map(|t| t.to_string())
            .or_else(|| (!path.is_empty()).then(|| path.clone())),
        HyperlinkData::Anchor { anchor } => Some(format!("#{anchor}")),
    };
    match target {
        Some(url) if !label.trim().is_empty() => format!("[{}]({url})", label.trim()),
        _ => label,
    }
}

/// A run's text with its bold and italic marks. The marks wrap the text without its edge
/// spaces, so Markdown still reads them as emphasis.
fn run_text(run: &Run) -> String {
    let mut text = String::new();
    for child in &run.children {
        match child {
            RunChild::Text(t) => text.push_str(&t.text),
            RunChild::Tab(_) => text.push(' '),
            RunChild::Break(_) | RunChild::CarriageReturn(_) => text.push(' '),
            _ => {}
        }
    }
    if text.trim().is_empty() {
        return text;
    }
    let bold = run.run_property.bold.is_some();
    let italic = run.run_property.italic.is_some();
    let mark = match (bold, italic) {
        (true, true) => "***",
        (true, false) => "**",
        (false, true) => "*",
        (false, false) => return text,
    };
    let lead = text.len() - text.trim_start().len();
    let trail = text.len() - text.trim_end().len();
    format!("{}{mark}{}{mark}{}", &text[..lead], text.trim(), &text[text.len() - trail..])
}

fn table_block(table: &Table, links: &HashMap<&str, &str>) -> String {
    let rows: Vec<Vec<String>> = table
        .rows
        .iter()
        .map(|TableChild::TableRow(row)| {
            row.cells
                .iter()
                .map(|TableRowChild::TableCell(cell)| cell_text(&cell.children, links))
                .collect()
        })
        .collect();
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    if width == 0 {
        return String::new();
    }
    let line = |row: &Vec<String>| {
        let mut cells: Vec<String> = row.iter().map(|c| c.replace('|', "\\|")).collect();
        cells.resize(width, String::new());
        format!("| {} |", cells.join(" | "))
    };
    let mut out = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        out.push(line(row));
        if i == 0 {
            out.push(format!("|{}|", vec!["---"; width].join("|")));
        }
    }
    out.join("\n")
}

/// A cell's text on one line; paragraphs are joined with a space, nested tables flattened.
fn cell_text(children: &[TableCellContent], links: &HashMap<&str, &str>) -> String {
    let mut parts: Vec<String> = Vec::new();
    for child in children {
        match child {
            TableCellContent::Paragraph(p) => {
                let text = inline(&p.children, links);
                if !text.trim().is_empty() {
                    parts.push(text.trim().to_string());
                }
            }
            TableCellContent::Table(inner) => {
                for TableChild::TableRow(row) in &inner.rows {
                    for TableRowChild::TableCell(cell) in &row.cells {
                        let text = cell_text(&cell.children, links);
                        if !text.is_empty() {
                            parts.push(text);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use docx_rs::{
        AbstractNumbering, Docx, Hyperlink, HyperlinkType, IndentLevel, Level, LevelJc, LevelText,
        NumberFormat, Numbering, NumberingId, Paragraph, Run, Start, Table, TableCell, TableRow,
    };

    use super::*;

    fn pack(docx: Docx) -> Vec<u8> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        docx.build().pack(&mut buffer).unwrap();
        buffer.into_inner()
    }

    fn numbered(id: usize, level: usize) -> Paragraph {
        Paragraph::new().numbering(NumberingId::new(id), IndentLevel::new(level))
    }

    fn sample() -> Vec<u8> {
        let bullets = AbstractNumbering::new(2)
            .add_level(Level::new(0, Start::new(1), NumberFormat::new("bullet"), LevelText::new("•"), LevelJc::new("left")))
            .add_level(Level::new(1, Start::new(1), NumberFormat::new("bullet"), LevelText::new("o"), LevelJc::new("left")));
        let decimals = AbstractNumbering::new(3)
            .add_level(Level::new(0, Start::new(1), NumberFormat::new("decimal"), LevelText::new("%1."), LevelJc::new("left")));
        let table = Table::new(vec![
            TableRow::new(vec![
                TableCell::new().add_paragraph(Paragraph::new().add_run(Run::new().add_text("Item"))),
                TableCell::new().add_paragraph(Paragraph::new().add_run(Run::new().add_text("Qty"))),
            ]),
            TableRow::new(vec![
                TableCell::new().add_paragraph(Paragraph::new().add_run(Run::new().add_text("Pens"))),
                TableCell::new().add_paragraph(Paragraph::new().add_run(Run::new().add_text("12 | dozen"))),
            ]),
        ]);
        let docx = Docx::new()
            .add_abstract_numbering(bullets)
            .add_abstract_numbering(decimals)
            .add_numbering(Numbering::new(2, 2))
            .add_numbering(Numbering::new(3, 3))
            .add_paragraph(Paragraph::new().style("Heading1").add_run(Run::new().add_text("Summary")))
            .add_paragraph(
                Paragraph::new()
                    .add_run(Run::new().add_text("Plain "))
                    .add_run(Run::new().add_text("bold").bold())
                    .add_run(Run::new().add_text(" and ").italic())
                    .add_run(Run::new().add_text("both").bold().italic()),
            )
            .add_paragraph(numbered(2, 0).add_run(Run::new().add_text("one")))
            .add_paragraph(numbered(2, 1).add_run(Run::new().add_text("nested")))
            .add_paragraph(numbered(2, 0).add_run(Run::new().add_text("two")))
            .add_paragraph(numbered(3, 0).add_run(Run::new().add_text("first")))
            .add_paragraph(numbered(3, 0).add_run(Run::new().add_text("second")))
            .add_paragraph(
                Paragraph::new().add_hyperlink(
                    Hyperlink::new("https://example.com", HyperlinkType::External)
                        .add_run(Run::new().add_text("docs")),
                ),
            )
            .add_table(table);
        pack(docx)
    }

    #[test]
    fn the_structure_comes_out_as_markdown() {
        let read = to_markdown(&sample(), 100_000).unwrap();
        let md = &read.markdown;
        assert!(md.contains("# Summary"), "{md}");
        assert!(md.contains("**bold**"), "{md}");
        assert!(md.contains("*and*"), "{md}");
        assert!(md.contains("***both***"), "{md}");
        assert!(md.contains("- one\n  - nested\n- two"), "{md}");
        assert!(md.contains("1. first\n2. second"), "{md}");
        assert!(md.contains("[docs](https://example.com)"), "{md}");
        assert!(md.contains("| Item | Qty |"), "{md}");
        assert!(md.contains("|---|---|"), "{md}");
        assert!(md.contains("| Pens | 12 \\| dozen |"), "{md}");
        assert!(!md.contains("[table content]"));
        assert!(!read.truncated);
    }

    #[test]
    fn long_text_is_cut_and_says_so() {
        let read = to_markdown(&sample(), 20).unwrap();
        assert_eq!(read.markdown.chars().count(), 20);
        assert!(read.truncated);
        assert!(read.note.unwrap().contains("20 characters"));
    }

    #[test]
    fn a_corrupt_file_is_an_error() {
        assert!(to_markdown(b"not a docx", 100).is_err());
    }
}
