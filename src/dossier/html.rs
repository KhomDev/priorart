//! A conservative HTML-to-text conversion for a saved program page.
//!
//! This is not a browser and not a general HTML parser. It exists so a program
//! page saved as `.html` yields roughly the Markdown a careful copy would:
//!
//! - `<h1>`-`<h6>` become `#`-`######` heading lines;
//! - `<li>` becomes a `- ` bullet;
//! - `<p>`, `<br>`, `<div>` and the other block elements become line breaks;
//! - every other tag is dropped and its text kept;
//! - `<script>` and `<style>` contents are dropped, never quoted;
//! - common named entities and numeric character references are decoded;
//! - whitespace is collapsed the way a browser collapses it, except in `<pre>`.
//!
//! What it does not do: CSS layout, JavaScript rendering, tables as tables,
//! text carried in attributes (`alt`, `title`, `aria-label`), or recovery of
//! content a page loads from elsewhere. A complex page should be saved as
//! Markdown by hand; `docs/dossier.md` says so, and the `priorart init`
//! scaffold repeats it.

use std::path::Path;

/// True when the path names a file this module converts.
pub fn is_html_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
}

/// Convert one HTML document to text. Never fails: malformed markup loses the
/// tag it starts, not the document.
pub fn to_text(html: &str) -> String {
    let bytes = html.as_bytes();
    let mut out = String::with_capacity(html.len());
    let mut pending_space = false;
    let mut heading: Option<usize> = None;
    let mut heading_started = false;
    let mut pre_depth = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] != b'<' {
            let start = i;
            while i < bytes.len() && bytes[i] != b'<' {
                i += 1;
            }
            push_text(
                &mut out,
                &html[start..i],
                &mut pending_space,
                heading,
                &mut heading_started,
                pre_depth > 0,
            );
            continue;
        }

        // Comments, doctypes and processing instructions are not text.
        if matches_ci(bytes, i, b"<!--") {
            i = find_from(bytes, i + 4, b"-->").map_or(bytes.len(), |p| p + 3);
            continue;
        }
        if matches_ci(bytes, i, b"<![CDATA[") {
            // The payload is text, the markers are not.
            let start = i + 9;
            let end = find_from(bytes, start, b"]]>").unwrap_or(bytes.len());
            push_text(
                &mut out,
                &html[start..end],
                &mut pending_space,
                heading,
                &mut heading_started,
                pre_depth > 0,
            );
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
            // A lone '<' is text, not a tag.
            push_char(
                &mut out,
                '<',
                &mut pending_space,
                heading,
                &mut heading_started,
                pre_depth > 0,
            );
            i += 1;
            continue;
        }
        let Some(end) = tag_end(bytes, j) else {
            // An unterminated tag: drop it rather than quote attributes.
            break;
        };
        i = end;

        if !closing && (name == "script" || name == "style") {
            // Raw text: everything up to the matching close tag is not page
            // text. An unterminated script drops the rest of the document,
            // which is what a browser would not display either.
            i = skip_raw_text(bytes, i, &name);
            block_break(&mut out, &mut pending_space);
            continue;
        }

        match name.as_str() {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                if closing {
                    if heading.is_some() {
                        trim_trailing_space(&mut out);
                        block_break(&mut out, &mut pending_space);
                        heading = None;
                        heading_started = false;
                    }
                } else if heading.is_none() {
                    block_break(&mut out, &mut pending_space);
                    let level = name.as_bytes()[1] - b'0';
                    for _ in 0..level {
                        out.push('#');
                    }
                    out.push(' ');
                    heading = Some(level as usize);
                    heading_started = false;
                }
            }
            "li" => {
                if heading.is_none() {
                    if closing {
                        block_break(&mut out, &mut pending_space);
                    } else {
                        block_break(&mut out, &mut pending_space);
                        out.push_str("- ");
                    }
                }
            }
            "pre" => {
                if closing {
                    pre_depth = pre_depth.saturating_sub(1);
                    block_break(&mut out, &mut pending_space);
                } else {
                    block_break(&mut out, &mut pending_space);
                    pre_depth += 1;
                }
            }
            "br" | "hr" => {
                if heading.is_none() {
                    block_break(&mut out, &mut pending_space);
                }
            }
            // Block containers. A break on both sides keeps one paragraph or
            // cell from being glued to the next one.
            "p" | "div" | "ul" | "ol" | "dl" | "dt" | "dd" | "section" | "article" | "header"
            | "footer" | "nav" | "main" | "aside" | "blockquote" | "figure" | "figcaption"
            | "table" | "thead" | "tbody" | "tfoot" | "tr" | "td" | "th" | "option" | "address"
            | "fieldset" | "form" | "title"
                if heading.is_none() =>
            {
                block_break(&mut out, &mut pending_space);
            }
            // Every other tag is markup: drop it, keep its text.
            _ => {}
        }
    }

    if heading.is_some() {
        trim_trailing_space(&mut out);
        block_break(&mut out, &mut pending_space);
    }
    out.replace("\r\n", "\n").replace('\r', "\n")
}

/// Is this byte allowed in a tag name? ASCII is enough for HTML in practice.
fn is_tag_name_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-' || c == b':' || c == b'_'
}

/// The index just past the `>` that ends a tag, respecting quoted attribute
/// values. None when the tag never ends.
fn tag_end(bytes: &[u8], mut i: usize) -> Option<usize> {
    let mut quote = 0u8;
    while i < bytes.len() {
        let c = bytes[i];
        if quote != 0 {
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'>' {
            return Some(i + 1);
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

/// Append a text run, decoding character references.
#[allow(clippy::too_many_arguments)]
fn push_text(
    out: &mut String,
    raw: &str,
    pending_space: &mut bool,
    heading: Option<usize>,
    heading_started: &mut bool,
    pre: bool,
) {
    let mut rest = raw;
    while let Some(pos) = rest.find('&') {
        for c in rest[..pos].chars() {
            push_char(out, c, pending_space, heading, heading_started, pre);
        }
        let tail = &rest[pos..];
        if let Some((c, consumed)) = entity_at(tail) {
            push_char(out, c, pending_space, heading, heading_started, pre);
            rest = &tail[consumed..];
        } else {
            push_char(out, '&', pending_space, heading, heading_started, pre);
            rest = &tail[1..];
        }
    }
    for c in rest.chars() {
        push_char(out, c, pending_space, heading, heading_started, pre);
    }
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
        assert!(!text.contains("secret"), "{text}");
        assert!(!text.contains("tooltip"), "{text}");
        assert!(!text.contains("hidden"), "{text}");
    }

    #[test]
    fn tables_become_one_line_per_cell() {
        let text = to_text("<table><tr><td>src/A.sol</td><td>src/B.sol</td></tr></table>");
        assert!(text.contains("src/A.sol\n"), "{text}");
        assert!(text.contains("src/B.sol\n"), "{text}");
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
