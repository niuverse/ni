//! Small inline Markdown parser for the terminal transcript.
//!
//! The event log stays raw text; this module only decides how a line is
//! presented. It intentionally covers the inline subset the product uses
//! (emphasis, code, links, strikethrough) and leaves block handling to the
//! caller. Unbalanced or unknown markers stay literal.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub link: bool,
    pub strike: bool,
    pub math: bool,
}

impl Style {
    fn merge(self, other: Style) -> Self {
        Self {
            bold: self.bold || other.bold,
            italic: self.italic || other.italic,
            code: self.code || other.code,
            link: self.link || other.link,
            strike: self.strike || other.strike,
            math: self.math || other.math,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

/// Return the ATX heading level and its text.
pub fn heading_with_level(text: &str) -> Option<(u8, String)> {
    let trimmed = text.trim_start();
    let hashes = trimmed
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &trimmed[hashes..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    Some((hashes as u8, rest.trim_start().to_string()))
}

/// Strip an ATX heading marker (`#` through `######`) and return its text.
#[cfg(test)]
fn heading(text: &str) -> Option<String> {
    heading_with_level(text).map(|(_, text)| text)
}

/// Parse one line of inline Markdown. Markers are removed when balanced and
/// echoed literally otherwise, so a half-streamed `**` never disappears.
pub fn inline(text: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut plain = String::new();
    let bytes = text.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        // Backslash escapes the next marker character.
        if bytes[index] == b'\\' && index + 1 < bytes.len() {
            let next = text[index + 1..].chars().next().unwrap_or('\\');
            if matches!(next, '*' | '_' | '`' | '~' | '[' | ']' | '\\') {
                plain.push(next);
                index += 1 + next.len_utf8();
                continue;
            }
        }

        if bytes[index] == b'$' {
            let (marker, offset) = if text[index..].starts_with("$$") {
                ("$$", 2)
            } else {
                ("$", 1)
            };
            if let Some(end) = text[index + offset..].find(marker) {
                let content = &text[index + offset..index + offset + end];
                if !content.is_empty() && !content.contains('\n') {
                    push(
                        &mut spans,
                        &mut plain,
                        Span {
                            text: latex_fallback(content),
                            style: Style {
                                math: true,
                                ..Style::default()
                            },
                        },
                    );
                    index += offset + end + marker.len();
                    continue;
                }
            }
        }

        if bytes[index] == b'`' {
            if let Some(end) = text[index + 1..].find('`') {
                let content = &text[index + 1..index + 1 + end];
                push(
                    &mut spans,
                    &mut plain,
                    Span {
                        text: content.to_string(),
                        style: Style {
                            code: true,
                            ..Style::default()
                        },
                    },
                );
                index += end + 2;
                continue;
            }
        }

        if bytes[index] == b'!' && text[index..].starts_with("![") {
            if let Some((label, url, consumed)) = link(text, index + 1) {
                push(
                    &mut spans,
                    &mut plain,
                    Span {
                        text: if url.is_empty() {
                            label
                        } else {
                            format!("{label} ({url})")
                        },
                        style: Style {
                            link: true,
                            ..Style::default()
                        },
                    },
                );
                index += consumed + 1;
                continue;
            }
        }

        if bytes[index] == b'[' {
            if let Some((label, url, consumed)) = link(text, index) {
                push(
                    &mut spans,
                    &mut plain,
                    Span {
                        text: if url.is_empty() {
                            label
                        } else {
                            format!("{label} ({url})")
                        },
                        style: Style {
                            link: true,
                            ..Style::default()
                        },
                    },
                );
                index += consumed;
                continue;
            }
        }

        if text[index..].starts_with("***")
            && text[index + 3..]
                .chars()
                .next()
                .is_some_and(|character| !character.is_whitespace())
        {
            if let Some(end) = text[index + 3..].find("***") {
                let inner = &text[index + 3..index + 3 + end];
                for mut span in inline(inner) {
                    span.style = span.style.merge(Style {
                        bold: true,
                        italic: true,
                        ..Style::default()
                    });
                    push(&mut spans, &mut plain, span);
                }
                index += end + 6;
                continue;
            }
        }

        if (text[index..].starts_with("**") || text[index..].starts_with("__"))
            && text[index + 2..]
                .chars()
                .next()
                .is_some_and(|character| !character.is_whitespace())
        {
            let marker = &text[index..index + 2];
            if let Some(end) = text[index + 2..].find(marker) {
                let inner = &text[index + 2..index + 2 + end];
                for mut span in inline(inner) {
                    span.style = span.style.merge(Style {
                        bold: true,
                        ..Style::default()
                    });
                    push(&mut spans, &mut plain, span);
                }
                index += end + 4;
                continue;
            }
        }

        if text[index..].starts_with("~~") {
            if let Some(end) = text[index + 2..].find("~~") {
                let inner = &text[index + 2..index + 2 + end];
                for mut span in inline(inner) {
                    span.style = span.style.merge(Style {
                        strike: true,
                        ..Style::default()
                    });
                    push(&mut spans, &mut plain, span);
                }
                index += end + 4;
                continue;
            }
        }

        if bytes[index] == b'*' || bytes[index] == b'_' {
            let marker = bytes[index] as char;
            let opens = text[index + 1..]
                .chars()
                .next()
                .is_some_and(|character| !character.is_whitespace());
            if opens {
                if let Some(end) = text[index + 1..].find(marker) {
                    if end > 0 {
                        let inner = &text[index + 1..index + 1 + end];
                        // Do not turn snake_case into italics.
                        let snake = marker == '_'
                            && inner.chars().any(|c| c.is_alphanumeric())
                            && index > 0
                            && text[..index]
                                .chars()
                                .last()
                                .is_some_and(|c| c.is_alphanumeric());
                        if !snake {
                            for mut span in inline(inner) {
                                span.style = span.style.merge(Style {
                                    italic: true,
                                    ..Style::default()
                                });
                                push(&mut spans, &mut plain, span);
                            }
                            index += end + 2;
                            continue;
                        }
                    }
                }
            }
        }

        let character = text[index..].chars().next().unwrap_or('*');
        plain.push(character);
        index += character.len_utf8();
    }

    push(
        &mut spans,
        &mut plain,
        Span {
            text: String::new(),
            style: Style::default(),
        },
    );
    if !plain.is_empty() {
        spans.push(Span {
            text: plain,
            style: Style::default(),
        });
    }
    spans
}

fn push(spans: &mut Vec<Span>, plain: &mut String, span: Span) {
    if !plain.is_empty() {
        spans.push(Span {
            text: std::mem::take(plain),
            style: Style::default(),
        });
    }
    if !span.text.is_empty() {
        spans.push(span);
    }
}

fn link(text: &str, start: usize) -> Option<(String, String, usize)> {
    let close = text[start..].find(']')? + start;
    if !text[close + 1..].starts_with('(') {
        return None;
    }
    let url_end = text[close + 2..].find(')')? + close + 2;
    let label = text[start + 1..close].to_string();
    let url = text[close + 2..url_end].to_string();
    Some((label, url, url_end + 1))
}

/// A readable Unicode fallback for common LaTeX. This is deliberately not a
/// TeX engine; the web client can use KaTeX for real typesetting.
pub fn latex_fallback(input: &str) -> String {
    let mut text = input.to_string();
    for (from, to) in [
        ("\\alpha", "α"),
        ("\\beta", "β"),
        ("\\gamma", "γ"),
        ("\\delta", "δ"),
        ("\\epsilon", "ε"),
        ("\\theta", "θ"),
        ("\\lambda", "λ"),
        ("\\mu", "μ"),
        ("\\pi", "π"),
        ("\\sigma", "σ"),
        ("\\phi", "φ"),
        ("\\omega", "ω"),
        ("\\sum", "∑"),
        ("\\prod", "∏"),
        ("\\int", "∫"),
        ("\\infty", "∞"),
        ("\\leq", "≤"),
        ("\\le", "≤"),
        ("\\geq", "≥"),
        ("\\ge", "≥"),
        ("\\neq", "≠"),
        ("\\times", "×"),
        ("\\cdot", "·"),
        ("\\pm", "±"),
        ("\\approx", "≈"),
        ("\\rightarrow", "→"),
        ("\\to", "→"),
    ] {
        text = text.replace(from, to);
    }
    // rac{a}{b} -> a/b, \sqrt{x} -> √(x)
    while let Some(start) = text.find("\\frac{") {
        let Some(first_end) = matching_brace(&text, start + 5) else {
            break;
        };
        let first = text[start + 6..first_end].to_string();
        let rest = &text[first_end + 1..];
        if !rest.starts_with('{') {
            break;
        }
        let Some(second_end) = matching_brace(rest, 0) else {
            break;
        };
        let second = rest[1..second_end].to_string();
        text.replace_range(
            start..first_end + second_end + 2,
            &format!("{first}/{second}"),
        );
    }
    while let Some(start) = text.find("\\sqrt{") {
        let Some(end) = matching_brace(&text, start + 5) else {
            break;
        };
        let body = text[start + 6..end].to_string();
        text.replace_range(start..end + 1, &format!("√({body})"));
    }
    text = text.replace("\\,", " ").replace("\\;", " ");
    text.replace(['{', '}'], "")
}

fn matching_brace(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0;
    for (offset, character) in text[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(spans: &[Span]) -> String {
        spans.iter().map(|span| span.text.as_str()).collect()
    }

    #[test]
    fn bold_italic_and_code_are_styled_and_stripped() {
        let spans = inline("a **bold** *it* `code` end");
        assert_eq!(text(&spans), "a bold it code end");
        assert!(spans
            .iter()
            .any(|span| span.text == "bold" && span.style.bold));
        assert!(spans
            .iter()
            .any(|span| span.text == "it" && span.style.italic));
        assert!(spans
            .iter()
            .any(|span| span.text == "code" && span.style.code));
    }

    #[test]
    fn triple_marker_is_bold_and_italic() {
        let spans = inline("***both***");
        assert_eq!(text(&spans), "both");
        assert!(spans[0].style.bold && spans[0].style.italic);
    }

    #[test]
    fn links_keep_label_and_url() {
        let spans = inline("[docs](https://example.test)");
        assert_eq!(text(&spans), "docs (https://example.test)");
        assert!(spans[0].style.link);
    }

    #[test]
    fn unbalanced_and_escaped_markers_stay_literal() {
        assert_eq!(text(&inline("2 * 3 and **open")), "2 * 3 and **open");
        assert_eq!(text(&inline(r"\*literal\*")), "*literal*");
        assert_eq!(text(&inline("snake_case_name")), "snake_case_name");
    }

    #[test]
    fn heading_levels_are_preserved() {
        assert_eq!(heading_with_level("# one"), Some((1, "one".into())));
        assert_eq!(heading_with_level("### three"), Some((3, "three".into())));
        assert_eq!(heading_with_level("####### seven"), None);
        assert_eq!(heading_with_level("#no-space"), None);
    }

    #[test]
    fn headings_strip_only_atx_markers() {
        assert_eq!(
            heading("## Release 0.1.12").as_deref(),
            Some("Release 0.1.12")
        );
        assert_eq!(heading("####### not a heading"), None);
        assert_eq!(heading("#hashtag"), None);
        assert_eq!(heading("#").as_deref(), Some(""));
    }

    #[test]
    fn math_uses_a_readable_unicode_fallback() {
        let spans = inline("energy $E = mc^2$ and $\\alpha \\le \\beta$");
        assert_eq!(text(&spans), "energy E = mc^2 and α ≤ β");
        assert!(spans
            .iter()
            .any(|span| span.text.contains("α") && span.style.math));
        assert_eq!(latex_fallback("\\frac{1}{2} + \\sqrt{x}"), "1/2 + √(x)");
    }

    #[test]
    fn currency_without_a_closing_dollar_stays_literal() {
        assert_eq!(text(&inline("costs $5 today")), "costs $5 today");
    }

    #[test]
    fn strikethrough_is_marked() {
        let spans = inline("~~old~~ new");
        assert_eq!(text(&spans), "old new");
        assert!(spans[0].style.strike);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Bullet,
    Ordered,
    Task(bool),
    Quote,
    Rule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockLine {
    pub text: String,
    pub kind: BlockKind,
}

fn ordered(text: &str) -> Option<(String, &str)> {
    let digits = text.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let marker = text.as_bytes().get(digits)?;
    if !matches!(marker, b'.' | b')') {
        return None;
    }
    let rest = text.get(digits + 1..)?.strip_prefix(' ')?;
    Some((format!("{}. ", &text[..digits]), rest))
}

/// Recognize a line-level Markdown block marker. Indentation is preserved.
/// `+` is deliberately not treated as a list marker: diff output uses it.
pub fn block(text: &str) -> Option<BlockLine> {
    let trimmed = text.trim_start();
    let indent = &text[..text.len() - trimmed.len()];
    if trimmed.chars().count() >= 3
        && trimmed
            .chars()
            .all(|character| matches!(character, '-' | '*' | '_'))
        && trimmed
            .chars()
            .all(|character| character == trimmed.chars().next().unwrap())
    {
        return Some(BlockLine {
            text: format!("{indent}{}", "─".repeat(40)),
            kind: BlockKind::Rule,
        });
    }
    if let Some((marker, rest)) = ordered(trimmed) {
        return Some(BlockLine {
            text: format!("{indent}{marker}{rest}"),
            kind: BlockKind::Ordered,
        });
    }
    let rest = if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
        return Some(BlockLine {
            text: format!("{indent}☐ {rest}"),
            kind: BlockKind::Task(false),
        });
    } else if let Some(rest) = trimmed
        .strip_prefix("- [x] ")
        .or_else(|| trimmed.strip_prefix("- [X] "))
    {
        return Some(BlockLine {
            text: format!("{indent}☑ {rest}"),
            kind: BlockKind::Task(true),
        });
    } else if let Some(rest) = trimmed.strip_prefix("- ") {
        Some((rest, BlockKind::Bullet))
    } else if let Some(rest) = trimmed.strip_prefix("* ") {
        Some((rest, BlockKind::Bullet))
    } else if let Some(rest) = trimmed.strip_prefix("> ") {
        Some((rest, BlockKind::Quote))
    } else if trimmed == ">" {
        Some(("", BlockKind::Quote))
    } else {
        None
    };
    rest.map(|(rest, kind)| {
        let marker = match kind {
            BlockKind::Bullet => "• ",
            BlockKind::Quote => "│ ",
            _ => "",
        };
        BlockLine {
            text: format!("{indent}{marker}{rest}"),
            kind,
        }
    })
}

#[cfg(test)]
mod block_tests {
    use super::*;

    #[test]
    fn bullets_tasks_and_quotes_are_recognized() {
        assert_eq!(block("- item").unwrap().text, "• item");
        assert_eq!(block("  * nested").unwrap().text, "  • nested");
        assert_eq!(block("- [ ] todo").unwrap().text, "☐ todo");
        assert_eq!(block("- [x] done").unwrap().text, "☑ done");
        assert_eq!(block("> quoted").unwrap().text, "│ quoted");
        assert_eq!(block(">").unwrap().text, "│ ");
        assert_eq!(block("1. numbered").unwrap().text, "1. numbered");
        assert_eq!(block("2) next").unwrap().text, "2. next");
        assert_eq!(block("---").unwrap().kind, BlockKind::Rule);
        assert_eq!(block("***").unwrap().kind, BlockKind::Rule);
    }

    #[test]
    fn emphasis_and_plus_are_not_mistaken_for_lists() {
        assert_eq!(block("*italic*"), None);
        assert_eq!(block("+ new line"), None);
    }
}
