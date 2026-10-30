//! Reading Word documents (`.docx`) as Markdown, and writing them from Markdown.
//!
//! Pure functions on bytes (spec `036-office-addon`, FR-002): headings, bold and italic,
//! ordered and unordered nested lists, tables as pipe tables and hyperlinks. Reading skips
//! images, shapes, text boxes and footnotes; writing turns images and raw HTML into their text.

use std::collections::HashMap;

use docx_rs::{
    AbstractNumbering, AlignmentType, BreakType, DocumentChild, Docx, Hyperlink, HyperlinkData,
    HyperlinkType, IndentLevel, Level, LevelJc, LevelText, NumberFormat, Numbering, NumberingId,
    Paragraph, ParagraphChild, Run, RunChild, RunFonts, SpecialIndentType, Start, Style,
    StyleType, Table, TableCell, TableCellContent, TableCellMargins, TableChild, TableRow,
    TableRowChild,
};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

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
    if let Some(numbering) = &p.property.numbering_property
        && let Some(id) = numbering.id.as_ref().map(|i| i.id).filter(|id| *id != 0)
    {
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

/// `Docx::new()` already holds a default numbering with id 1, so the lists the writer adds are
/// numbered from 2 (research R4).
const FIRST_LIST_ID: usize = 2;
const LIST_LEVELS: usize = 9;
const LINK_COLOUR: &str = "0563C1";

/// A Word document built from Markdown. `title` becomes a Title paragraph.
///
/// Headings, paragraphs, bold, italic, strikethrough and inline code, nested ordered and
/// unordered lists, pipe tables, hyperlinks, block quotes, code blocks and rules. Images and raw
/// HTML degrade to their text.
pub fn from_markdown(markdown: &str, title: Option<&str>) -> Result<Vec<u8>, VizierError> {
    let markdown = xml_safe(markdown);
    let title = title.map(xml_safe);
    let mut writer = Writer::default();
    if let Some(title) = title.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        writer.blocks.push(Block::Paragraph(Box::new(
            Paragraph::new().style("Title").add_run(Run::new().add_text(title)),
        )));
    }
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    for event in Parser::new_ext(&markdown, options) {
        writer.event(event);
    }
    writer.finish()
}

/// Drops the characters XML 1.0 forbids (control characters other than tab, line feed and carriage
/// return, and U+FFFE/U+FFFF): `docx-rs` writes them raw, and Word then reports the file as corrupt.
fn xml_safe(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '\u{0}'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' | '\u{fffe}' | '\u{ffff}'))
        .collect()
}

enum Block {
    Paragraph(Box<Paragraph>),
    Table(Box<Table>),
}

struct ListState {
    id: usize,
    ordered: bool,
}

#[derive(Default)]
struct TableState {
    rows: Vec<TableRow>,
    cells: Vec<TableCell>,
    paragraphs: Vec<Paragraph>,
    header: bool,
}

#[derive(Default)]
struct Writer {
    blocks: Vec<Block>,
    abstracts: Vec<AbstractNumbering>,
    numberings: Vec<Numbering>,
    next_list_id: usize,
    bullet_id: Option<usize>,
    lists: Vec<ListState>,
    /// An item has started and its first paragraph carries the marker.
    item_pending: bool,
    para: Option<Paragraph>,
    bold: u32,
    italic: u32,
    strike: u32,
    quote: i32,
    code_block: bool,
    link: Option<(String, Vec<Run>)>,
    table: Option<TableState>,
}

impl Writer {
    fn event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text),
            Event::Code(code) => {
                let run = self.run(&code).fonts(RunFonts::new().ascii("Consolas").hi_ansi("Consolas"));
                self.push_run(run);
            }
            Event::Html(html) => {
                let text = strip_tags(&html);
                if !text.trim().is_empty() {
                    self.text(text.trim_end());
                    self.flush();
                }
            }
            Event::InlineHtml(html) => self.text(&strip_tags(&html)),
            Event::SoftBreak => self.text(" "),
            Event::HardBreak => {
                let run = self.run("").add_break(BreakType::TextWrapping);
                self.push_run(run);
            }
            Event::Rule => {
                self.flush();
                self.blocks.push(Block::Paragraph(Box::new(
                    Paragraph::new().align(AlignmentType::Center).add_run(Run::new().add_text("* * *")),
                )));
            }
            Event::TaskListMarker(done) => self.text(if done { "\u{2611} " } else { "\u{2610} " }),
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => self.start_para(None),
            Tag::Heading { level, .. } => self.start_para(Some(heading_style(level))),
            Tag::BlockQuote(_) => {
                self.flush();
                self.quote += 1;
            }
            Tag::CodeBlock(_) => {
                self.flush();
                self.code_block = true;
            }
            Tag::List(first) => {
                self.flush();
                self.start_list(first);
            }
            Tag::Item => {
                self.flush();
                self.item_pending = true;
            }
            Tag::Emphasis => self.italic += 1,
            Tag::Strong => self.bold += 1,
            Tag::Strikethrough => self.strike += 1,
            Tag::Link { dest_url, .. } => self.link = Some((dest_url.to_string(), Vec::new())),
            Tag::Table(_) => {
                self.flush();
                self.table = Some(TableState::default());
            }
            Tag::TableHead => {
                if let Some(t) = self.table.as_mut() {
                    t.header = true;
                }
            }
            Tag::TableCell => {
                self.flush();
                if let Some(t) = self.table.as_mut() {
                    t.paragraphs.clear();
                }
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) => self.flush(),
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.quote = (self.quote - 1).max(0);
            }
            TagEnd::CodeBlock => self.code_block = false,
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
            }
            TagEnd::Item => {
                self.flush();
                self.item_pending = false;
            }
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
            TagEnd::Link => {
                if let Some((url, runs)) = self.link.take() {
                    let mut link = Hyperlink::new(url, HyperlinkType::External);
                    for run in runs {
                        link = link.add_run(run);
                    }
                    let para = self.para_or_new().add_hyperlink(link);
                    self.para = Some(para);
                }
            }
            TagEnd::TableCell => {
                self.flush();
                if let Some(t) = self.table.as_mut() {
                    let mut cell = TableCell::new();
                    let paragraphs = std::mem::take(&mut t.paragraphs);
                    if paragraphs.is_empty() {
                        cell = cell.add_paragraph(Paragraph::new());
                    }
                    for p in paragraphs {
                        cell = cell.add_paragraph(p);
                    }
                    t.cells.push(cell);
                }
            }
            TagEnd::TableHead => {
                self.end_row();
                if let Some(t) = self.table.as_mut() {
                    t.header = false;
                }
            }
            TagEnd::TableRow => self.end_row(),
            TagEnd::Table => {
                if let Some(t) = self.table.take() {
                    // Twips: without a margin the text touches the cell borders.
                    let table = Table::new(t.rows).margins(TableCellMargins::new().margin(40, 100, 40, 100));
                    self.blocks.push(Block::Table(Box::new(table)));
                }
            }
            _ => {}
        }
    }

    fn end_row(&mut self) {
        if let Some(t) = self.table.as_mut() {
            let cells = std::mem::take(&mut t.cells);
            t.rows.push(TableRow::new(cells));
        }
    }

    fn start_list(&mut self, first: Option<u64>) {
        let ordered = first.is_some();
        let parent_ordered = self.lists.last().filter(|l| l.ordered).map(|l| l.id);
        let id = if ordered {
            // A nested ordered list continues its parent's numbering at a deeper level; any
            // other ordered list gets its own, so it counts from its own first number.
            parent_ordered.unwrap_or_else(|| {
                let id = self.new_list_id();
                self.define_list(id, true, first.unwrap_or(1) as usize);
                id
            })
        } else if let Some(id) = self.bullet_id {
            id
        } else {
            let id = self.new_list_id();
            self.define_list(id, false, 1);
            self.bullet_id = Some(id);
            id
        };
        self.lists.push(ListState { id, ordered });
    }

    fn new_list_id(&mut self) -> usize {
        let id = FIRST_LIST_ID + self.next_list_id;
        self.next_list_id += 1;
        id
    }

    fn define_list(&mut self, id: usize, ordered: bool, first: usize) {
        let mut abstract_numbering = AbstractNumbering::new(id);
        for level in 0..LIST_LEVELS {
            let (format, text) = if ordered {
                ("decimal", format!("%{}.", level + 1))
            } else {
                ("bullet", ["\u{2022}", "o", "\u{25AA}"][level % 3].to_string())
            };
            let start = if level == 0 { first } else { 1 };
            abstract_numbering = abstract_numbering.add_level(
                Level::new(level, Start::new(start), NumberFormat::new(format), LevelText::new(text), LevelJc::new("left"))
                    .indent(Some(720 * (level as i32 + 1)), Some(SpecialIndentType::Hanging(360)), None, None),
            );
        }
        self.abstracts.push(abstract_numbering);
        self.numberings.push(Numbering::new(id, id));
    }

    /// Begin a paragraph. The first one of a list item carries the item's marker.
    fn start_para(&mut self, style: Option<&str>) {
        self.flush();
        let mut p = Paragraph::new();
        if let Some(style) = style {
            p = p.style(style);
        }
        if self.item_pending {
            self.item_pending = false;
            if let Some(list) = self.lists.last() {
                let level = (self.lists.len() - 1).min(LIST_LEVELS - 1);
                p = p.numbering(NumberingId::new(list.id), IndentLevel::new(level));
            }
        } else if self.quote > 0 {
            p = p.indent(Some(720 * self.quote), None, None, None);
        }
        self.para = Some(p);
    }

    fn para_or_new(&mut self) -> Paragraph {
        if self.para.is_none() {
            self.start_para(None);
        }
        self.para.take().unwrap_or_default()
    }

    fn flush(&mut self) {
        let Some(p) = self.para.take() else { return };
        match self.table.as_mut() {
            Some(t) => t.paragraphs.push(p),
            None => self.blocks.push(Block::Paragraph(Box::new(p))),
        }
    }

    /// A run carrying the marks that are open right now.
    fn run(&self, text: &str) -> Run {
        let header = self.table.as_ref().is_some_and(|t| t.header);
        let mut run = Run::new();
        if !text.is_empty() {
            run = run.add_text(text);
        }
        if self.bold > 0 || header {
            run = run.bold();
        }
        if self.italic > 0 {
            run = run.italic();
        }
        if self.strike > 0 {
            run = run.strike();
        }
        if self.link.is_some() {
            run = run.color(LINK_COLOUR).underline("single");
        }
        run
    }

    fn push_run(&mut self, run: Run) {
        if let Some((_, runs)) = self.link.as_mut() {
            runs.push(run);
            return;
        }
        let para = self.para_or_new().add_run(run);
        self.para = Some(para);
    }

    fn text(&mut self, text: &str) {
        if self.code_block {
            for line in text.lines() {
                self.start_para(None);
                let run = self.run(line).fonts(RunFonts::new().ascii("Consolas").hi_ansi("Consolas"));
                self.push_run(run);
                self.flush();
            }
            return;
        }
        if text.is_empty() {
            return;
        }
        let run = self.run(text);
        self.push_run(run);
    }

    fn finish(mut self) -> Result<Vec<u8>, VizierError> {
        self.flush();
        // docx-rs defaults to Times New Roman; styles without their own font inherit this one.
        let mut docx = Docx::new()
            .default_fonts(RunFonts::new().ascii("Calibri").hi_ansi("Calibri").east_asia("Calibri").cs("Calibri"))
            .add_style(
            Style::new("Title", StyleType::Paragraph).name("Title").bold().size(56),
        );
        for level in 1..=6usize {
            let size = [40, 32, 28, 26, 24, 22][level - 1];
            docx = docx.add_style(
                Style::new(format!("Heading{level}"), StyleType::Paragraph)
                    .name(format!("heading {level}"))
                    .bold()
                    .size(size)
                    .outline_lvl(level - 1),
            );
        }
        for a in self.abstracts {
            docx = docx.add_abstract_numbering(a);
        }
        for n in self.numberings {
            docx = docx.add_numbering(n);
        }
        for block in self.blocks {
            docx = match block {
                Block::Paragraph(p) => docx.add_paragraph(*p),
                Block::Table(t) => docx.add_table(*t),
            };
        }
        let mut buffer = std::io::Cursor::new(Vec::new());
        docx.build()
            .pack(&mut buffer)
            .map_err(|e| VizierError(format!("Could not build the Word file: {e}")))?;
        Ok(buffer.into_inner())
    }
}

fn heading_style(level: HeadingLevel) -> &'static str {
    match level {
        HeadingLevel::H1 => "Heading1",
        HeadingLevel::H2 => "Heading2",
        HeadingLevel::H3 => "Heading3",
        HeadingLevel::H4 => "Heading4",
        HeadingLevel::H5 => "Heading5",
        HeadingLevel::H6 => "Heading6",
    }
}

/// Raw HTML as the text between its tags.
fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
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

    const SOURCE: &str = "# Plan\n\nSome **bold**, some *italic* and `code`, see [the docs](https://example.com/a?b=1).\n\n## Steps\n\n- one\n  - nested\n    - deeper\n- two\n\n1. first\n2. second\n   1. inner\n\nAfter.\n\n1. restarts\n\n| Item | Qty |\n|---|---|\n| Pens | 12 |\n| Ink | 3 |\n";

    #[test]
    fn written_markdown_reads_back_with_its_structure() {
        let bytes = from_markdown(SOURCE, Some("Quarterly report")).unwrap();
        let md = to_markdown(&bytes, 100_000).unwrap().markdown;
        assert!(md.starts_with("# Quarterly report\n"), "{md}");
        assert!(md.contains("# Plan"), "{md}");
        assert!(md.contains("## Steps"), "{md}");
        assert!(md.contains("**bold**"), "{md}");
        assert!(md.contains("*italic*"), "{md}");
        assert!(md.contains("code"), "{md}");
        assert!(md.contains("[the docs](https://example.com/a?b=1)"), "{md}");
        assert!(md.contains("- one\n  - nested\n    - deeper\n- two"), "{md}");
        assert!(md.contains("1. first\n2. second\n  1. inner"), "{md}");
        assert!(md.contains("1. restarts"), "{md}");
        // The header row is bold.
        assert!(md.contains("| **Item** | **Qty** |"), "{md}");
        assert!(md.contains("| Pens | 12 |"), "{md}");
        assert!(md.contains("| Ink | 3 |"), "{md}");
    }

    #[test]
    fn the_title_is_a_title_paragraph_and_is_optional() {
        let with = from_markdown("text", Some("  My title ")).unwrap();
        let docx = docx_rs::read_docx(&with).unwrap();
        let style = match &docx.document.children[0] {
            DocumentChild::Paragraph(p) => p.property.style.as_ref().map(|s| s.val.clone()),
            _ => None,
        };
        assert_eq!(style.as_deref(), Some("Title"));
        let without = to_markdown(&from_markdown("text", None).unwrap(), 1000).unwrap().markdown;
        assert_eq!(without.trim(), "text");
        let blank = to_markdown(&from_markdown("text", Some("  ")).unwrap(), 1000).unwrap().markdown;
        assert_eq!(blank.trim(), "text");
    }

    #[test]
    fn list_numbering_starts_at_2_so_it_does_not_collide_with_the_default() {
        let bytes = from_markdown(SOURCE, None).unwrap();
        let docx = docx_rs::read_docx(&bytes).unwrap();
        let ids: Vec<usize> = docx.numberings.numberings.iter().map(|n| n.id).collect();
        assert_eq!(ids.iter().filter(|id| **id == 1).count(), 1, "{ids:?}");
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "{ids:?}");
        assert!(ids.contains(&2), "{ids:?}");
        let abstracts: Vec<usize> = docx.numberings.abstract_nums.iter().map(|a| a.id).collect();
        let mut unique = abstracts.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), abstracts.len(), "{abstracts:?}");
    }

    #[test]
    fn images_and_html_degrade_to_text() {
        let md = to_markdown(
            &from_markdown("A ![a chart](x.png) and <b>raw</b> html.\n\n<div>block</div>\n", None).unwrap(),
            1000,
        )
        .unwrap()
        .markdown;
        assert!(md.contains("a chart"), "{md}");
        assert!(md.contains("raw"), "{md}");
        assert!(md.contains("block"), "{md}");
        assert!(!md.contains("<b>") && !md.contains("<div>") && !md.contains("x.png"), "{md}");
    }

    #[test]
    fn control_characters_never_reach_the_document_xml() {
        let bytes = from_markdown("a\u{1}b\u{b}c\td\n\ne\u{1f}", Some("t\u{2}")).unwrap();
        let md = to_markdown(&bytes, 1000).unwrap().markdown;
        assert!(md.contains("abc\td"), "{md:?}");
        assert!(!md.chars().any(|c| c.is_control() && c != '\n' && c != '\t'), "{md:?}");
    }

    #[test]
    fn empty_markdown_still_makes_a_valid_document() {
        let bytes = from_markdown("", None).unwrap();
        assert_eq!(to_markdown(&bytes, 100).unwrap().markdown.trim(), "");
    }
}
