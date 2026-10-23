//! Small, dependency-free token classifier for fenced code.
//!
//! It is deliberately line-based: strings and comments that span lines are
//! rare in the transcript and a stateful highlighter is a later upgrade. The
//! classifier never changes text, only token classes, so cell width and copy
//! output stay exact.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Plain,
    Keyword,
    String,
    Comment,
    Number,
    Function,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    pub kind: TokenKind,
}

const KEYWORDS: &[&str] = &[
    "as",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "def",
    "else",
    "enum",
    "export",
    "false",
    "fn",
    "for",
    "from",
    "function",
    "if",
    "impl",
    "import",
    "in",
    "interface",
    "let",
    "loop",
    "match",
    "mod",
    "mut",
    "new",
    "nil",
    "none",
    "null",
    "pub",
    "return",
    "self",
    "static",
    "struct",
    "super",
    "switch",
    "this",
    "throw",
    "trait",
    "true",
    "try",
    "type",
    "use",
    "var",
    "void",
    "where",
    "while",
    "yield",
];

/// Classify one line. Every input character appears in exactly one token.
pub fn highlight(line: &str) -> Vec<Token> {
    let chars: Vec<char> = line.chars().collect();
    let mut tokens = Vec::new();
    let mut plain = String::new();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch == '/' && chars.get(index + 1) == Some(&'/') {
            flush(&mut tokens, &mut plain);
            let text: String = chars[index..].iter().collect();
            tokens.push(Token {
                text,
                kind: TokenKind::Comment,
            });
            break;
        }
        if ch == '#' && (index == 0 || chars[index - 1].is_whitespace()) {
            flush(&mut tokens, &mut plain);
            let text: String = chars[index..].iter().collect();
            tokens.push(Token {
                text,
                kind: TokenKind::Comment,
            });
            break;
        }
        if matches!(ch, '"' | '\'' | '`') {
            flush(&mut tokens, &mut plain);
            let start = index;
            index += 1;
            while index < chars.len() {
                if chars[index] == '\\' {
                    index = (index + 2).min(chars.len());
                    continue;
                }
                let current = chars[index];
                index += 1;
                if current == ch {
                    break;
                }
            }
            tokens.push(Token {
                text: chars[start..index].iter().collect(),
                kind: TokenKind::String,
            });
            continue;
        }
        if ch.is_ascii_digit() {
            flush(&mut tokens, &mut plain);
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || matches!(chars[index], '.' | '_' | 'x'))
            {
                index += 1;
            }
            tokens.push(Token {
                text: chars[start..index].iter().collect(),
                kind: TokenKind::Number,
            });
            continue;
        }
        if ch.is_alphabetic() || ch == '_' {
            flush(&mut tokens, &mut plain);
            let start = index;
            index += 1;
            while index < chars.len() && (chars[index].is_alphanumeric() || chars[index] == '_') {
                index += 1;
            }
            let word: String = chars[start..index].iter().collect();
            let next = chars[index..]
                .iter()
                .find(|character| !character.is_whitespace());
            let kind = if KEYWORDS.contains(&word.as_str()) {
                TokenKind::Keyword
            } else if next == Some(&'(') {
                TokenKind::Function
            } else {
                TokenKind::Plain
            };
            tokens.push(Token { text: word, kind });
            continue;
        }
        plain.push(ch);
        index += 1;
    }
    flush(&mut tokens, &mut plain);
    tokens
}

fn flush(tokens: &mut Vec<Token>, plain: &mut String) {
    if !plain.is_empty() {
        tokens.push(Token {
            text: std::mem::take(plain),
            kind: TokenKind::Plain,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_keywords_numbers_strings_and_comments() {
        let tokens = highlight("let x = \"hi\"; // note");
        assert!(tokens
            .iter()
            .any(|token| token.kind == TokenKind::Keyword && token.text == "let"));
        assert!(tokens
            .iter()
            .any(|token| token.kind == TokenKind::String && token.text == "\"hi\""));
        assert!(tokens
            .iter()
            .any(|token| token.kind == TokenKind::Comment && token.text == "// note"));
        let numbers = highlight("port 8787");
        assert!(numbers.iter().any(|token| token.kind == TokenKind::Number));
    }

    #[test]
    fn preserves_the_exact_text() {
        for line in [
            "fn main() { println!(\"hi\"); }",
            "  # indented comment",
            "value = 12.5 # tail",
            "",
        ] {
            let rebuilt: String = highlight(line)
                .into_iter()
                .map(|token| token.text)
                .collect();
            assert_eq!(rebuilt, line);
        }
    }
}
