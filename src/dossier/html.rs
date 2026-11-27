//! A conservative HTML-to-text conversion for a saved program page.
//!
//! This is not a browser and not a general HTML parser. It exists so a program
//! page saved as `.html` yields roughly the Markdown a careful copy would:
//!
//! - `<h1>`-`<h6>` become `#`-`######` heading lines;
//! - `<li>` becomes a `- ` bullet;
//! - a `<table>` becomes one bullet per body row (see [`to_text_with_base`]);
//! - an `<a href>` with no text, or text that is not the URL, appends the URL;
//! - `<p>`, `<br>`, `<div>` and the other block elements become line breaks;
//! - every other tag is dropped and its text kept;
//! - `<script>` and `<style>` contents are dropped, never quoted;
//! - common named entities and numeric character references are decoded;
//! - whitespace is collapsed the way a browser collapses it, except in `<pre>`.
//!
//! What it does not do: CSS layout, JavaScript rendering, text carried in
//! attributes other than `href` (`alt`, `title`, `aria-label`), images, or
//! content a page loads from elsewhere. `docs/dossier.md` says so.

use std::path::Path;

/// True when the path names a file this module converts.
pub fn is_html_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
}

/// Convert one HTML document to text. It never panics. A malformed but
/// terminated tag loses only its attributes; an unterminated tag or quoted
/// attribute drops the remainder of the document, because everything after it
/// is attribute state a browser would not display either.
///
/// Relative links are left relative. Pass [`to_text_with_base`] when the page
/// was fetched and its final URL is known.
pub fn to_text(html: &str) -> String {
    to_text_with_base(html, None)
}

/// [`to_text`], resolving relative `href`s against `base` when `base` is set.
///
/// `base` is the fetched page's final URL. Without it, a relative href is
/// copied as written. `utm_*` query keys are removed from every emitted URL.
pub fn to_text_with_base(html: &str, base: Option<&str>) -> String {
    let mut conv = Converter {
        base,
        out: String::with_capacity(html.len()),
        pending_space: false,
        heading: None,
        heading_started: false,
        pre_depth: 0,
        link: None,
        tables: Vec::new(),
        after_close: false,
    };
    conv.convert(html);
    conv.out.replace("\r\n", "\n").replace('\r', "\n")
}

struct LinkOpen {
    href: String,
    start: usize,
}

struct CellBuf {
    text: String,
    header: bool,
    pending_space: bool,
}

struct RowBuf {
    cells: Vec<(String, bool)>,
    header: bool,
}

struct TableBuf {
    rows: Vec<RowBuf>,
    row: Vec<(String, bool)>,
    cell: Option<CellBuf>,
    in_head: bool,
    row_header: bool,
}

struct Converter<'a> {
    base: Option<&'a str>,
    out: String,
    pending_space: bool,
    heading: Option<usize>,
    heading_started: bool,
    pre_depth: usize,
    link: Option<LinkOpen>,
    tables: Vec<TableBuf>,
    /// The last thing read was a closing tag, with no text after it.
    after_close: bool,
}

impl Converter<'_> {
    fn convert(&mut self, html: &str) {
        let bytes = html.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] != b'<' {
                let start = i;
                while i < bytes.len() && bytes[i] != b'<' {
                    i += 1;
                }
                self.push_text(&html[start..i]);
                continue;
            }
            if matches_ci(bytes, i, b"<!--") {
                i = find_from(bytes, i + 4, b"-->").map_or(bytes.len(), |p| p + 3);
                continue;
            }
            if matches_ci(bytes, i, b"<![CDATA[") {
                let start = i + 9;
                let end = find_from(bytes, start, b"]]>").unwrap_or(bytes.len());
                self.push_text(&html[start..end]);
                i = end.saturating_add(3).min(bytes.len());
                continue;
            }
            if bytes.get(i + 1).is_some_and(|c| *c == b'!' || *c == b'?') {
                i = find_from(bytes, i + 2, b">").map_or(bytes.len(), |p| p + 1);
                continue;
            }
            let mut j = i + 1;
            let closing = bytes.get(j) == Some(&b'/');
            if closing {
                j += 1;
            }
            let name_start = j;
            while j < bytes.len() && is_tag_name_byte(bytes[j]) {
                j += 1;
            }
            let name: String = html[name_start..j].to_ascii_lowercase();
            if name.is_empty() {
                self.push_char('<');
                i += 1;
                continue;
            }
            let Some(end) = tag_end(bytes, j) else {
                break;
            };
            let tag = &html[i..end];
            i = end;
            if !closing && (name == "script" || name == "style") {
                i = skip_raw_text(bytes, i, &name);
                self.block_boundary();
                continue;
            }
            if !closing && self.after_close {
                // Two sibling elements with nothing between them, such as
                // `<span>$2,000,000</span><span>Min</span>`, are separate
                // words. Text right after a closing tag (`<b>Loan</b>Book`)
                // still joins, so an identifier split by markup stays whole.
                self.soft_space();
            }
            self.after_close = closing;
            self.on_tag(&name, closing, tag);
        }
        if self.heading.is_some() {
            trim_trailing_space(&mut self.out);
            block_break(&mut self.out, &mut self.pending_space);
        }
    }

    fn on_tag(&mut self, name: &str, closing: bool, tag: &str) {
        match name {
            "table" => {
                if closing {
                    self.close_table();
                } else {
                    if self.heading.is_some() {
                        trim_trailing_space(&mut self.out);
                        block_break(&mut self.out, &mut self.pending_space);
                        self.heading = None;
                        self.heading_started = false;
                    }
                    self.tables.push(TableBuf {
                        rows: Vec::new(),
                        row: Vec::new(),
                        cell: None,
                        in_head: false,
                        row_header: false,
                    });
                }
            }
            "thead" | "tbody" | "tfoot" if !self.tables.is_empty() => {
                if let Some(table) = self.tables.last_mut() {
                    table.in_head = !closing && name == "thead";
                }
            }
            "tr" if !self.tables.is_empty() => {
                // A new row closes the previous one. Real pages often omit
                // </tr>, and an empty close is a no-op.
                self.close_row();
                if !closing {
                    if let Some(table) = self.tables.last_mut() {
                        table.row_header = table.in_head;
                    }
                }
            }
            "td" | "th" if !self.tables.is_empty() => {
                if closing {
                    self.close_cell();
                } else if let Some(table) = self.tables.last_mut() {
                    table.cell = Some(CellBuf {
                        text: String::new(),
                        header: name == "th" || table.in_head,
                        pending_space: false,
                    });
                }
            }
            "a" => {
                if closing {
                    self.close_link();
                } else if let Some(href) = attr_value(tag, "href") {
                    self.link = Some(LinkOpen {
                        href,
                        start: self.sink_len(),
                    });
                }
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" if self.tables.is_empty() => {
                if closing {
                    if self.heading.is_some() {
                        trim_trailing_space(&mut self.out);
                        block_break(&mut self.out, &mut self.pending_space);
                        self.heading = None;
                        self.heading_started = false;
                    }
                } else if self.heading.is_none() {
                    block_break(&mut self.out, &mut self.pending_space);
                    let level = name.as_bytes()[1] - b'0';
                    for _ in 0..level {
                        self.out.push('#');
                    }
                    self.out.push(' ');
                    self.heading = Some(level as usize);
                    self.heading_started = false;
                }
            }
            "li" => {
                if self.in_cell() {
                    if !closing {
                        self.separate_cell_item();
                    }
                } else if self.heading.is_none() {
                    if closing {
                        block_break(&mut self.out, &mut self.pending_space);
                    } else {
                        block_break(&mut self.out, &mut self.pending_space);
                        self.out.push_str("- ");
                    }
                }
            }
            "pre" if self.tables.is_empty() => {
                if closing {
                    self.pre_depth = self.pre_depth.saturating_sub(1);
                    block_break(&mut self.out, &mut self.pending_space);
                } else {
                    block_break(&mut self.out, &mut self.pending_space);
                    self.pre_depth += 1;
                }
            }
            "br" | "hr" => {
                if self.in_cell() {
                    self.cell_space();
                } else if self.heading.is_none() {
                    block_break(&mut self.out, &mut self.pending_space);
                }
            }
            "p" | "div" | "ul" | "ol" | "dl" | "dt" | "dd" | "section" | "article" | "header"
            | "footer" | "nav" | "main" | "aside" | "blockquote" | "figure" | "figcaption"
            | "option" | "address" | "fieldset" | "form" | "title"
                if self.heading.is_none() || self.in_cell() =>
            {
                self.block_boundary();
            }
            _ => {}
        }
    }

    fn in_cell(&self) -> bool {
        self.tables.last().is_some_and(|t| t.cell.is_some())
    }

    fn block_boundary(&mut self) {
        if self.in_cell() {
            self.cell_space();
        } else if self.tables.is_empty() {
            block_break(&mut self.out, &mut self.pending_space);
        }
    }

    fn cell_space(&mut self) {
        if let Some(table) = self.tables.last_mut() {
            if let Some(cell) = table.cell.as_mut() {
                cell.pending_space = true;
            }
        }
    }

    fn separate_cell_item(&mut self) {
        if let Some(table) = self.tables.last_mut() {
            if let Some(cell) = table.cell.as_mut() {
                let trimmed = cell.text.trim_end();
                if !trimmed.is_empty() && !trimmed.ends_with(';') {
                    cell.text = format!("{trimmed}; ");
                }
                cell.pending_space = false;
            }
        }
    }

    fn close_cell(&mut self) {
        let Some(table) = self.tables.last_mut() else {
            return;
        };
        if let Some(cell) = table.cell.take() {
            let text = cell.text.split_whitespace().collect::<Vec<_>>().join(" ");
            if cell.header {
                table.row_header = true;
            }
            table.row.push((text, cell.header));
        }
    }

    fn close_row(&mut self) {
        self.close_cell();
        let Some(table) = self.tables.last_mut() else {
            return;
        };
        let header = table.row_header && table.row.iter().all(|(_, h)| *h);
        let cells = std::mem::take(&mut table.row);
        if cells.iter().any(|(t, _)| !t.is_empty()) {
            table.rows.push(RowBuf { cells, header });
        }
        table.row_header = table.in_head;
    }

    fn close_table(&mut self) {
        self.close_row();
        let Some(table) = self.tables.pop() else {
            return;
        };
        let rendered = render_table(&table);
        if rendered.is_empty() {
            return;
        }
        if self.in_cell() {
            self.push_text(&rendered.replace('\n', " "));
        } else {
            block_break(&mut self.out, &mut self.pending_space);
            self.out.push_str(&rendered);
            block_break(&mut self.out, &mut self.pending_space);
            // A blank line so the row bullets do not absorb the next paragraph.
            if !self.out.ends_with("\n\n") {
                self.out.push('\n');
            }
        }
    }

    fn sink_len(&self) -> usize {
        if let Some(table) = self.tables.last() {
            if let Some(cell) = &table.cell {
                return cell.text.len();
            }
        }
        self.out.len()
    }

    fn text_since(&self, start: usize) -> String {
        if let Some(table) = self.tables.last() {
            if let Some(cell) = &table.cell {
                let start = start.min(cell.text.len());
                return cell.text[start..].to_string();
            }
        }
        let start = start.min(self.out.len());
        self.out[start..].to_string()
    }

    fn close_link(&mut self) {
        let Some(link) = self.link.take() else {
            return;
        };
        let Some(url) = crate::fetch::url::resolve_link(self.base, &link.href) else {
            return;
        };
        let written = self.text_since(link.start);
        let trimmed = written.trim();
        if trimmed.is_empty() {
            self.push_text(&url);
        } else if trimmed != url && !trimmed.contains(&url) {
            self.push_text(&format!(" {url}"));
        }
    }

    fn push_char(&mut self, c: char) {
        if let Some(table) = self.tables.last_mut() {
            if let Some(cell) = table.cell.as_mut() {
                push_cell_char(cell, c);
                return;
            }
            // Whitespace between cells is not page text.
            return;
        }
        push_char(
            &mut self.out,
            c,
            &mut self.pending_space,
            self.heading,
            &mut self.heading_started,
            self.pre_depth > 0,
        );
    }

    /// A word break that adds no blank line: a space before the next text.
    fn soft_space(&mut self) {
        if let Some(table) = self.tables.last_mut() {
            if let Some(cell) = table.cell.as_mut() {
                cell.pending_space = true;
            }
            return;
        }
        self.pending_space = true;
    }

    fn push_text(&mut self, raw: &str) {
        if !raw.is_empty() {
            self.after_close = false;
        }
        let mut rest = raw;
        while let Some(pos) = rest.find('&') {
            for c in rest[..pos].chars() {
                self.push_char(c);
            }
            let tail = &rest[pos..];
            if let Some((c, consumed)) = entity_at(tail) {
                self.push_char(c);
                rest = &tail[consumed..];
            } else {
                self.push_char('&');
                rest = &tail[1..];
            }
        }
        for c in rest.chars() {
            self.push_char(c);
        }
    }
}

fn push_cell_char(cell: &mut CellBuf, c: char) {
    if c.is_control() && c != '\n' && c != '\t' {
        return;
    }
    if c.is_whitespace() {
        cell.pending_space = true;
        return;
    }
    if cell.pending_space && !cell.text.is_empty() && !cell.text.ends_with(' ') {
        cell.text.push(' ');
    }
    cell.pending_space = false;
    cell.text.push(c);
}

fn render_table(table: &TableBuf) -> String {
    let mut header: Option<Vec<String>> = None;
    let mut lines = Vec::new();
    for row in &table.rows {
        let cells: Vec<String> = row.cells.iter().map(|(t, _)| t.clone()).collect();
        let is_header = row.header && header.is_none();
        if is_header {
            header = Some(cells);
            continue;
        }
        if let Some(names) = &header {
            let mut parts = Vec::new();
            for (i, cell) in cells.iter().enumerate() {
                if cell.is_empty() {
                    continue;
                }
                match names.get(i).map(|s| s.as_str()).filter(|s| !s.is_empty()) {
                    Some(name) => parts.push(format!("{name}: {cell}")),
                    None => parts.push(cell.clone()),
                }
            }
            if !parts.is_empty() {
                lines.push(format!("- {}", parts.join("; ")));
            }
        } else {
            let parts: Vec<&str> = cells
                .iter()
                .filter(|s| !s.is_empty())
                .map(String::as_str)
                .collect();
            if !parts.is_empty() {
                lines.push(format!("- {}", parts.join(" | ")));
            }
        }
    }
    lines.join("\n")
}

fn attr_value(tag: &str, want: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut i = 1usize;
    if bytes.get(i) == Some(&b'/') {
        return None;
    }
    while i < bytes.len() && is_tag_name_byte(bytes[i]) {
        i += 1;
    }
    while i < bytes.len() && bytes[i] != b'>' {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] == b'>' || bytes[i] == b'/' {
            break;
        }
        let start = i;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'-' | b':' | b'_'))
        {
            i += 1;
        }
        let name = &tag[start..i];
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b'=') {
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let value = if bytes.get(i).is_some_and(|c| *c == b'"' || *c == b'\'') {
            let quote = bytes[i];
            i += 1;
            let start = i;
            while i < bytes.len() && bytes[i] != quote {
                i += 1;
            }
            let v = tag[start..i].to_string();
            if i < bytes.len() {
                i += 1;
            }
            v
        } else {
            let start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
                i += 1;
            }
            tag[start..i].to_string()
        };
        if name.eq_ignore_ascii_case(want) {
            return Some(decode_attr(&value));
        }
    }
    None
}

fn decode_attr(raw: &str) -> String {
    let mut out = String::new();
    let mut rest = raw;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        if let Some((c, consumed)) = entity_at(tail) {
            out.push(c);
            rest = &tail[consumed..];
        } else {
            out.push('&');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// Is this byte allowed in a tag name? ASCII is enough for HTML in practice.
fn is_tag_name_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-' || c == b':' || c == b'_'
}

/// The index just past the `>` that ends a tag.
///
/// A quote starts an attribute value only when it follows `=` (whitespace
/// allowed). Stray apostrophes are common in real pages (`<support's team>`,
/// `<a href=/don't>`); treating every quote as an attribute quote swallowed
/// the rest of the document. A quoted value may contain `>` and the other
/// quote character. An unterminated quote returns None, and the caller drops
/// the remainder because the document ends inside attribute state.
fn tag_end(bytes: &[u8], mut i: usize) -> Option<usize> {
    let mut quote = 0u8;
    let mut after_equals = false;
    while i < bytes.len() {
        let c = bytes[i];
        if quote != 0 {
            if c == quote {
                quote = 0;
            }
            i += 1;
            continue;
        }
        if c == b'>' {
            return Some(i + 1);
        }
        if c == b'"' || c == b'\'' {
            if after_equals {
                quote = c;
            }
            after_equals = false;
        } else if c == b'=' {
            after_equals = true;
        } else if !c.is_ascii_whitespace() {
            after_equals = false;
        }
        i += 1;
    }
    None
}

/// Skip the raw contents of a `<script>` or `<style>` element. `start` is just
/// past the open tag. Returns the index just past the close tag, or the end of
/// the document when there is no close tag.
fn skip_raw_text(bytes: &[u8], start: usize, name: &str) -> usize {
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == b'<'
            && bytes.get(i + 1) == Some(&b'/')
            && matches_ci(bytes, i + 2, name.as_bytes())
        {
            let after = i + 2 + name.len();
            let boundary = bytes
                .get(after)
                .is_none_or(|c| c.is_ascii_whitespace() || *c == b'>' || *c == b'/');
            if boundary {
                return tag_end(bytes, after).unwrap_or(bytes.len());
            }
        }
        i += 1;
    }
    bytes.len()
}

fn matches_ci(bytes: &[u8], at: usize, needle: &[u8]) -> bool {
    bytes.len() >= at + needle.len()
        && bytes[at..at + needle.len()]
            .iter()
            .zip(needle)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

fn find_from(bytes: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from > bytes.len() {
        return None;
    }
    bytes[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| from + p)
}

/// End the current line. A break at the start of the document or right after
/// another break is ignored, so blocks do not multiply blank lines.
fn block_break(out: &mut String, pending_space: &mut bool) {
    *pending_space = false;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
}

fn trim_trailing_space(out: &mut String) {
    while out.ends_with(' ') {
        out.pop();
    }
}

/// Append one decoded character, collapsing whitespace outside `<pre>`.
#[allow(clippy::too_many_arguments)]
fn push_char(
    out: &mut String,
    c: char,
    pending_space: &mut bool,
    heading: Option<usize>,
    heading_started: &mut bool,
    pre: bool,
) {
    if c.is_control() && c != '\n' && c != '\t' {
        return;
    }
    if pre {
        *pending_space = false;
        if c == '\n' {
            out.push('\n');
        } else if heading.is_some() {
            push_heading_char(out, c, heading_started);
        } else {
            out.push(c);
        }
        return;
    }
    if c.is_whitespace() {
        *pending_space = true;
        return;
    }
    if heading.is_some() {
        if *heading_started {
            if *pending_space && !out.ends_with(' ') {
                out.push(' ');
            }
            out.push(c);
        } else {
            // Skip the indentation that HTML source puts before heading text.
            out.push(c);
            *heading_started = true;
        }
        *pending_space = false;
        return;
    }
    if *pending_space && !out.is_empty() && !out.ends_with('\n') && !out.ends_with(' ') {
        out.push(' ');
    }
    *pending_space = false;
    out.push(c);
}

fn push_heading_char(out: &mut String, c: char, heading_started: &mut bool) {
    if c.is_whitespace() {
        if *heading_started && !out.ends_with(' ') {
            out.push(' ');
        }
        return;
    }
    out.push(c);
    *heading_started = true;
}

/// Decode one character reference at the start of `tail` (which begins with
/// `&`). Returns the character and how many bytes the reference spans.
fn entity_at(tail: &str) -> Option<(char, usize)> {
    // The scan is bounded: no reference this build decodes is longer than 32
    // characters, and a page with many bare `&` characters must not make the
    // conversion quadratic.
    let semi = tail
        .char_indices()
        .skip(1)
        .take(33)
        .find(|(_, c)| *c == ';')
        .map(|(at, _)| at)?;
    let body = &tail[1..semi];
    if body.is_empty() || body.len() > 32 {
        return None;
    }
    let decoded = if let Some(number) = body.strip_prefix('#') {
        let (radix, digits) = if let Some(hex) = number
            .strip_prefix('x')
            .or_else(|| number.strip_prefix('X'))
        {
            (16, hex)
        } else {
            (10, number)
        };
        if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
            return None;
        }
        let value = u32::from_str_radix(digits, radix).ok()?;
        // A numeric reference to a control character is dropped: it is never
        // page text, and a dossier must not carry a terminal escape.
        char::from_u32(value).map_or(Some(' '), |c| {
            if c.is_control() && c != '\n' && c != '\t' {
                Some(' ')
            } else {
                Some(c)
            }
        })?
    } else {
        named_entity(body)?
    };
    Some((decoded, semi + 1))
}

/// The named character references this build decodes. Anything else is left
/// as written: inventing a replacement would fabricate page text.
fn named_entity(name: &str) -> Option<char> {
    const NAMED: [(&str, char); 31] = [
        ("amp", '&'),
        ("lt", '<'),
        ("gt", '>'),
        ("quot", '"'),
        ("apos", '\''),
        ("nbsp", ' '),
        ("ensp", ' '),
        ("emsp", ' '),
        ("thinsp", ' '),
        ("shy", ' '),
        ("ndash", '\u{2013}'),
        ("mdash", '\u{2014}'),
        ("lsquo", '\u{2018}'),
        ("rsquo", '\u{2019}'),
        ("ldquo", '\u{201c}'),
        ("rdquo", '\u{201d}'),
        ("hellip", '\u{2026}'),
        ("bull", '\u{2022}'),
        ("middot", '\u{00b7}'),
        ("copy", '\u{00a9}'),
        ("reg", '\u{00ae}'),
        ("trade", '\u{2122}'),
        ("times", '\u{00d7}'),
        ("plusmn", '\u{00b1}'),
        ("frac12", '\u{00bd}'),
        ("sect", '\u{00a7}'),
        ("deg", '\u{00b0}'),
        ("laquo", '\u{00ab}'),
        ("raquo", '\u{00bb}'),
        ("euro", '\u{20ac}'),
        ("pound", '\u{00a3}'),
    ];
    NAMED.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_bullets_and_blocks_become_markdown() {
        let html = "<!doctype html><html><head><title>Program</title></head><body>\
<h1>Example Program</h1><h2>Rewards</h2><ul><li>Critical: &amp; 50,000</li>\
<li>High: 10,000</li></ul><div>Assets in scope</div><p>src/Vault.sol<br>\
src/FeeLedger.sol</p></body></html>";
        let text = to_text(html);
        assert!(text.contains("\n# Example Program\n"), "{text}");
        assert!(text.contains("\n## Rewards\n"), "{text}");
        assert!(text.contains("- Critical: & 50,000\n"), "{text}");
        assert!(text.contains("- High: 10,000\n"), "{text}");
        assert!(text.contains("\nsrc/Vault.sol\n"), "{text}");
        assert!(text.contains("\nsrc/FeeLedger.sol\n"), "{text}");
        assert!(!text.contains('<'), "a tag survived: {text}");
    }

    #[test]
    fn script_and_style_contents_are_never_text() {
        let html = "<p>before</p><SCRIPT type=\"text/javascript\">\
var secret = 'not page text';</SCRIPT><style>.x{color:red}</style>\
<p>after</p><script>unterminated()";
        let text = to_text(html);
        assert!(text.contains("before"), "{text}");
        assert!(text.contains("after"), "{text}");
        assert!(!text.contains("secret"), "{text}");
        assert!(!text.contains("color:red"), "{text}");
        assert!(!text.contains("unterminated"), "{text}");
    }

    #[test]
    fn named_and_numeric_entities_are_decoded() {
        let html = "<li>a &amp; b &lt;c&gt; &quot;d&quot; &#39;e&#39; &#x2F;f &hellip;</li>\
<li>unknown &notanentity; stays</li><li>esc &#27;[31m</li>";
        let text = to_text(html);
        assert!(text.contains("a & b <c> \"d\" 'e' /f \u{2026}"), "{text}");
        assert!(text.contains("unknown &notanentity; stays"), "{text}");
        assert!(!text.contains('\u{1b}'), "an escape survived: {text:?}");
    }

    #[test]
    fn inline_tags_do_not_split_an_identifier() {
        let text = to_text("<p>src/<code>Vault</code>.sol and <b>Loan</b>Book</p>");
        assert!(text.contains("src/Vault.sol and LoanBook"), "{text}");
    }

    #[test]
    fn adjacent_sibling_elements_are_separate_words() {
        let text = to_text("<p><span>$2,000,000</span><span>Minimum reward</span></p>");
        assert!(text.contains("$2,000,000 Minimum reward"), "{text}");
        let cell = to_text(
            "<table><tr><td><span>Critical</span></td><td><span>$2,000,000</span><span>Min</span></td></tr></table>",
        );
        assert!(cell.contains("$2,000,000 Min"), "{cell}");
        assert!(!cell.contains("$2,000,000Min"), "{cell}");
        let heading = to_text("<h3><span>Rewards</span><span>by threat level</span></h3>");
        assert!(heading.contains("### Rewards by threat level"), "{heading}");
    }

    #[test]
    fn heading_text_spanning_lines_is_one_heading() {
        let text = to_text("<h2>\n   Out of\n   scope\n</h2><ul><li>x</li></ul>");
        assert_eq!(text.trim(), "## Out of scope\n- x");
    }

    #[test]
    fn attributes_comments_and_doctype_are_not_text() {
        let html = "<!-- hidden comment --><!doctype html>\
<p><a href=\"https://example.invalid/secret\" title=\"tooltip\">link</a></p>";
        let text = to_text(html);
        assert!(text.contains("link"), "{text}");
        // The visible word is not the href, so the URL is appended. The title
        // attribute and the comment are still not text.
        assert!(text.contains("https://example.invalid/secret"), "{text}");
        assert!(!text.contains("tooltip"), "{text}");
        assert!(!text.contains("hidden"), "{text}");
    }

    #[test]
    fn tables_become_one_bullet_per_row() {
        let text = to_text("<table><tr><td>src/A.sol</td><td>src/B.sol</td></tr></table>");
        assert!(text.contains("- src/A.sol | src/B.sol"), "{text}");
    }

    #[test]
    fn a_header_row_names_each_cell() {
        let html = "<table><tr><th>Severity</th><th>Title</th></tr>\
<tr><td>Critical</td><td>Direct theft</td></tr>\
<tr><td></td><td>Skipped empty</td></tr></table>";
        let text = to_text(html);
        assert!(
            text.contains("- Severity: Critical; Title: Direct theft"),
            "{text}"
        );
        assert!(text.contains("- Title: Skipped empty"), "{text}");
        assert!(!text.contains("Severity: ;"), "{text}");
    }

    #[test]
    fn an_empty_anchor_appends_its_url_and_utm_is_stripped() {
        let html = "<table><tr><td><a href=\"https://github.com/acme/vault?utm_source=x\"><span></span></a></td>\
<td>Vault</td></tr></table>";
        let text = to_text(html);
        assert!(text.contains("https://github.com/acme/vault"), "{text}");
        assert!(!text.contains("utm_source"), "{text}");
    }

    #[test]
    fn a_relative_href_stays_relative_without_a_base_and_resolves_with_one() {
        let html = "<a href=\"contracts/Vault.sol\"></a>";
        assert!(
            to_text(html).contains("contracts/Vault.sol"),
            "{}",
            to_text(html)
        );
        let with = to_text_with_base(html, Some("https://example.com/program/scope"));
        assert!(
            with.contains("https://example.com/program/contracts/Vault.sol"),
            "{with}"
        );
    }

    #[test]
    fn link_text_that_differs_from_the_url_keeps_both() {
        let text = to_text("<a href=\"https://example.com/vault\" title=\"hidden\">docs</a>");
        assert!(text.contains("docs"), "{text}");
        assert!(text.contains("https://example.com/vault"), "{text}");
        assert!(!text.contains("hidden"), "{text}");
    }

    #[test]
    fn plain_text_and_empty_input_pass_through() {
        assert_eq!(to_text(""), "");
        // Trailing whitespace is collapsed, because HTML would not render it.
        assert_eq!(to_text("just text\n"), "just text");
        assert!(!to_text("a < b").contains('\u{0}'));
        assert!(to_text("a < b").contains("a < b"), "{}", to_text("a < b"));
    }

    #[test]
    fn an_unterminated_tag_does_not_quote_attributes() {
        let text = to_text("<p>kept</p><a href=\"https://example.invalid/secret\"");
        assert!(text.contains("kept"), "{text}");
        assert!(!text.contains("secret"), "{text}");
    }

    #[test]
    fn a_stray_apostrophe_inside_a_tag_does_not_swallow_the_document() {
        let html = "<p>before</p><p>contact <support's team> for details</p>\
<h2>Out of scope</h2><ul><li>Attack A</li></ul>";
        let text = to_text(html);
        assert!(text.contains("contact"), "{text}");
        assert!(text.contains("for details"), "{text}");
        assert!(text.contains("## Out of scope"), "{text}");
        assert!(text.contains("- Attack A"), "{text}");
    }

    #[test]
    fn an_unquoted_apostrophe_in_an_attribute_ends_at_the_gt() {
        let text = to_text("<a href=/don't>link</a><p>after</p>");
        assert!(text.contains("link"), "{text}");
        assert!(text.contains("after"), "{text}");
        assert!(!text.contains('<'), "{text}");
    }

    #[test]
    fn a_quoted_attribute_may_contain_gt_and_the_other_quote() {
        let text = to_text("<a title=\"a > b 'c'\" href='x > y'>link</a><p>after</p>");
        assert!(text.contains("link"), "{text}");
        assert!(text.contains("after"), "{text}");
        assert!(!text.contains("title"), "{text}");
        // The href is not the link text, so it is appended. "after" still
        // following it is what shows the quoted `>` did not end the tag.
        assert!(text.contains("x > y"), "{text}");
    }

    #[test]
    fn an_unterminated_quoted_attribute_drops_the_remainder() {
        // Documented behavior: the rest of the bytes are attribute state, so
        // they are not page text.
        let text = to_text("<p>kept</p><a title=\"unterminated>lost");
        assert!(text.contains("kept"), "{text}");
        assert!(
            !text.contains("lost"),
            "the remainder after an unterminated quote is attribute state: {text}"
        );
    }

    #[test]
    fn non_ascii_text_is_preserved() {
        let html = "<p>Réserve — trésorerie</p><li>emoji 😀</li>\
<li>e\u{301} combining</li><li>&#233;t&#xE9; and &amp;</li>";
        let text = to_text(html);
        assert!(text.contains("Réserve — trésorerie"), "{text}");
        assert!(text.contains("emoji 😀"), "{text}");
        assert!(text.contains("e\u{301} combining"), "{text}");
        assert!(text.contains("été and &"), "{text}");
    }

    #[test]
    fn a_long_unknown_entity_is_left_as_written() {
        let mut html = String::from("&");
        html.push_str(&"x".repeat(10_000));
        html.push(';');
        let text = to_text(&html);
        assert!(text.starts_with("&xxx"), "{text}");
        assert!(text.ends_with(';'), "{text}");
    }

    #[test]
    fn the_extension_check_is_case_insensitive() {
        assert!(is_html_path(Path::new("scope.html")));
        assert!(is_html_path(Path::new("scope.HTM")));
        assert!(!is_html_path(Path::new("scope.md")));
        assert!(!is_html_path(Path::new("scope")));
    }
}
