// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::authorship::Copyright;
use ra_ap_rustc_lexer::{Cursor, FrontmatterAllowed, LiteralKind, TokenKind};
use std::iter::Peekable;
use std::ops::Range;

/// Write the license header at the top of the source, after any byte order mark, shebang and frontmatter and before inner attributes and documentation, replacing the license metadata in its leading comments.
///
/// rustfmt removes the blank lines around a comment that sits between inner attributes, including `//!` documentation, so a header placed after a leading `#![...]` attribute is not stable under formatting when more follow. Placing it first is, and the source after it is kept as it was, apart from the removed metadata, the blank lines that separated it, and the blank lines at its end, which rustfmt removes.
pub(crate) fn update_source(contents: &str, copyrights: &[Copyright]) -> String {
    let (byte_order_mark, contents) = split_byte_order_mark(contents);
    let lines: Vec<&str> = contents.split_inclusive('\n').collect();
    let newline = line_ending(contents);
    let prefix_length = prefix_length(contents, &lines);
    let leading = leading_parts(contents, &lines, prefix_length);

    let mut remainder: Vec<&str> = Vec::new();
    // As 0.2.9 and earlier, remove license metadata in documentation comments only among the leading comments directly after the shebang and the inner attributes that start the source: documentation comments after a block comment, or after an inner attribute that follows comments or blank lines, are the crate's documentation, and are kept. Keep them too when they make a `#!` that only comments separate from a `[` a shebang, so that removing them does not make it an inner attribute on the next update.
    let mut removes_documentation = !is_shebang_by_documentation(contents);
    // Whether a blank line or comment has been seen, ending the inner attributes that start the source.
    let mut after_comment = false;
    let mut skips_blank_lines = false;
    for part in &leading {
        let line = lines[part.lines.start];
        let documentation = match part.kind {
            PartKind::Blank => false,
            PartKind::LineComment { documentation } => documentation,
            PartKind::InnerAttribute | PartKind::BlockComment => {
                // Keep inner attributes and block comments verbatim, including any lines inside them that look like comments.
                if after_comment || part.kind == PartKind::BlockComment {
                    removes_documentation = false;
                }
                remainder.extend(&lines[part.lines.clone()]);
                skips_blank_lines = false;
                continue;
            }
        };
        after_comment = true;
        if skips_blank_lines && part.kind == PartKind::Blank {
            continue;
        }
        skips_blank_lines = false;
        if is_license_metadata(clean_line(line)) && (removes_documentation || !documentation) {
            // Remove the metadata, and the blank lines that separated it from what follows when it was separated from what precedes it too, so a blank line between kept comments and code remains.
            skips_blank_lines = remainder.last().is_none_or(|line| is_blank(line));
            continue;
        }
        remainder.push(line);
    }
    let body = leading.last().map_or(prefix_length, |part| part.lines.end);
    remainder.extend(&lines[body..]);

    let start = remainder
        .iter()
        .position(|line| !is_blank(line))
        .unwrap_or(remainder.len());
    // rustfmt ends a file at its last line, without trailing blank lines.
    let end = remainder
        .iter()
        .rposition(|line| !is_blank(line))
        .map_or(start, |index| index + 1);
    let remainder = &remainder[start..end];

    let mut output = String::from(byte_order_mark);
    for line in &lines[..prefix_length] {
        push_line(&mut output, line, newline);
    }
    if prefix_length > 0 {
        output.push_str(newline);
    }

    output.push_str("// Released under the MIT License.");
    output.push_str(newline);
    for copyright in copyrights {
        output.push_str(&copyright.source_comment());
        output.push_str(newline);
    }

    if !remainder.is_empty() {
        output.push_str(newline);
        for line in remainder {
            push_line(&mut output, line, newline);
        }
    }

    output
}

/// Split off a leading byte order mark, which rustc skips before the shebang, so that it stays first.
fn split_byte_order_mark(contents: &str) -> (&str, &str) {
    match contents.strip_prefix('\u{feff}') {
        Some(rest) => ("\u{feff}", rest),
        None => ("", contents),
    }
}

/// Append a line, ending it with the source's line ending if it has none.
fn push_line(output: &mut String, line: &str, newline: &str) {
    output.push_str(line);
    if !line.ends_with('\n') {
        output.push_str(newline);
    }
}

/// Count the shebang line, which must stay first, if there is one: as rustc decides, a source starting with `#!` is an inner attribute instead when `[` is the next token, after whitespace and comments other than documentation.
fn shebang_length(contents: &str) -> usize {
    usize::from(ra_ap_rustc_lexer::strip_shebang(contents).is_some())
}

/// Whether the source starts with a shebang only because of a documentation comment before a `[` that only comments separate from its `#!`: without that comment, as rustc decides, the `#!` would start an inner attribute.
fn is_shebang_by_documentation(contents: &str) -> bool {
    shebang_length(contents) == 1
        && ra_ap_rustc_lexer::tokenize(&contents[2..], FrontmatterAllowed::No)
            .find(|token| token.kind != TokenKind::Whitespace && !is_comment_token(token.kind))
            .is_some_and(|token| token.kind == TokenKind::OpenBracket)
}

/// Count the lines that must stay before the header: the shebang, if there is one, and the frontmatter of a cargo script after it, the `---` fenced block that rustc requires to come first, with only whitespace before it.
///
/// Frontmatter whose closing fence the lexer does not find is not kept first, since the lexer's recovery for it can extend it over the header on the next update. rustc rejects it.
fn prefix_length(contents: &str, lines: &[&str]) -> usize {
    let shebang_length = shebang_length(contents);
    let line_starts = line_starts(lines);
    let mut end = line_starts
        .get(shebang_length)
        .copied()
        .unwrap_or(contents.len());
    for token in ra_ap_rustc_lexer::tokenize(&contents[end..], FrontmatterAllowed::Yes) {
        let start = end;
        end += token.len as usize;
        match token.kind {
            TokenKind::Whitespace => {}
            TokenKind::Frontmatter {
                has_invalid_preceding_whitespace: false,
                ..
            } => {
                // The lexer ends frontmatter at the end of the line holding its closing fence, a line starting with as many dashes as its opening fence, when it finds one.
                let frontmatter = &contents[start..end];
                let fence =
                    &frontmatter[..frontmatter.len() - frontmatter.trim_start_matches('-').len()];
                if !frontmatter.contains(&format!("\n{fence}")) {
                    break;
                }
                return line_starts.partition_point(|&start| start < end);
            }
            _ => break,
        }
    }
    shebang_length
}

/// The byte offset of the start of each line.
fn line_starts(lines: &[&str]) -> Vec<usize> {
    lines
        .iter()
        .scan(0, |offset, line| {
            let start = *offset;
            *offset += line.len();
            Some(start)
        })
        .collect()
}

/// What a part of the leading section is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PartKind {
    /// A line holding only whitespace.
    Blank,
    /// A line holding only a `//` comment, which is documentation when rustc reads it so, as `///` or `//!`, but not `////`.
    LineComment { documentation: bool },
    /// An inner attribute, with any comments after it on its last line.
    InnerAttribute,
    /// A block comment, with any comments after it on its last line.
    BlockComment,
}

/// A part of the leading section: the lines it spans and what it is.
#[derive(Debug, PartialEq, Eq)]
struct Part {
    lines: Range<usize>,
    kind: PartKind,
}

/// A token other than whitespace, as rustc's lexer reads it, and the bytes of the source it spans.
struct Token {
    kind: TokenKind,
    span: Range<usize>,
}

/// Split the leading section after the shebang and frontmatter, before the first item, into its parts: each blank line and `//` comment, and each inner attribute and block comment, which may span several lines.
///
/// rustc's lexer reads the source, so brackets, quotes and comment markers in strings, char literals and comments are never mistaken for the structure around them. The section ends at the first line that starts with anything else, at an attribute or block comment that code follows on its last line, and at one that the lexer cannot end: an unterminated literal or comment, or unbalanced delimiters, which rustc rejects. That part and everything after it are kept as they are.
fn leading_parts(contents: &str, lines: &[&str], start: usize) -> Vec<Part> {
    let line_starts = line_starts(lines);
    let offset = line_starts.get(start).copied().unwrap_or(contents.len());
    let mut tokens = tokenize(contents, offset)
        .filter(|token| token.kind != TokenKind::Whitespace)
        .peekable();

    let mut parts = Vec::new();
    for (index, line) in lines.iter().enumerate().skip(start) {
        if parts
            .last()
            .is_some_and(|part: &Part| part.lines.end > index)
        {
            continue;
        }
        let line_end = line_starts[index] + line.len();
        let Some(token) = tokens.next_if(|token| token.span.start < line_end) else {
            parts.push(Part {
                lines: index..index + 1,
                kind: PartKind::Blank,
            });
            continue;
        };
        let (kind, end) = match token.kind {
            TokenKind::LineComment { doc_style } => {
                parts.push(Part {
                    lines: index..index + 1,
                    kind: PartKind::LineComment {
                        documentation: doc_style.is_some(),
                    },
                });
                continue;
            }
            TokenKind::BlockComment {
                terminated: true, ..
            } => (PartKind::BlockComment, token.span.end),
            TokenKind::Pound => match inner_attribute_end(&mut tokens) {
                Some(end) => (PartKind::InnerAttribute, end),
                None => break,
            },
            _ => break,
        };
        let Some(end) = end_of_line(contents, &mut tokens, end) else {
            break;
        };
        // The line holding the last byte of the part, and any comments after it, is its last.
        let last = line_starts.partition_point(|&start| start < end) - 1;
        parts.push(Part {
            lines: index..last + 1,
            kind,
        });
    }
    parts
}

/// Read the tokens of the source from `offset`, as rustc's parser reads them from its lexer.
fn tokenize(contents: &str, mut offset: usize) -> impl Iterator<Item = Token> {
    // Any frontmatter is before `offset`, and only allowed first, so it is not recognized from there.
    let mut cursor = Cursor::new(&contents[offset..], FrontmatterAllowed::No);
    std::iter::from_fn(move || {
        let token = cursor.advance_token();
        let start = offset;
        match token.kind {
            TokenKind::Eof => return None,
            // The prefix reserved for guarded strings, `#"` or `##`, is an error from Rust 2024, and earlier a `#`, after which rustc lexes again. Do the same, so that `#"]"#` is read as `#`, `"]"` and `#` rather than as an unterminated string after `#"`.
            TokenKind::GuardedStrPrefix => {
                offset += 1;
                cursor = Cursor::new(&contents[offset..], FrontmatterAllowed::No);
                return Some(Token {
                    kind: TokenKind::Pound,
                    span: start..offset,
                });
            }
            _ => offset += token.len as usize,
        }
        Some(Token {
            kind: token.kind,
            span: start..offset,
        })
    })
}

/// Read the rest of an inner attribute after its `#`: `!`, then `[` and the tokens through its matching `]`, returning the end of that bracket. Returns `None` when it is not an inner attribute, or its delimiters are unbalanced or a literal or comment in it is unterminated.
fn inner_attribute_end(tokens: &mut Peekable<impl Iterator<Item = Token>>) -> Option<usize> {
    let mut code =
        tokens.filter(|token| !is_comment_token(token.kind) || is_unterminated(token.kind));
    if code.next()?.kind != TokenKind::Bang || code.next()?.kind != TokenKind::OpenBracket {
        return None;
    }
    let mut open = vec![TokenKind::CloseBracket];
    for token in code {
        match token.kind {
            TokenKind::OpenParen => open.push(TokenKind::CloseParen),
            TokenKind::OpenBrace => open.push(TokenKind::CloseBrace),
            TokenKind::OpenBracket => open.push(TokenKind::CloseBracket),
            TokenKind::CloseParen | TokenKind::CloseBrace | TokenKind::CloseBracket => {
                if open.pop() != Some(token.kind) {
                    return None;
                }
                if open.is_empty() {
                    return Some(token.span.end);
                }
            }
            kind if is_unterminated(kind) => return None,
            _ => {}
        }
    }
    None
}

/// Read the comments after the part ending at `end` on its last line, returning the end of the last of them, or `end`. Returns `None` when code, or an unterminated comment, follows on that line.
fn end_of_line(
    contents: &str,
    tokens: &mut Peekable<impl Iterator<Item = Token>>,
    mut end: usize,
) -> Option<usize> {
    while let Some(token) = tokens.next_if(|token| !contents[end..token.span.start].contains('\n'))
    {
        if !is_comment_token(token.kind) || is_unterminated(token.kind) {
            return None;
        }
        end = token.span.end;
    }
    Some(end)
}

fn is_comment_token(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
    )
}

/// Whether the token is a literal or block comment without its end, which runs to the end of the source, or a raw string with an invalid start, such as `r#~`, which rustc rejects.
fn is_unterminated(kind: TokenKind) -> bool {
    match kind {
        TokenKind::BlockComment { terminated, .. } => !terminated,
        TokenKind::Literal { kind, .. } => match kind {
            LiteralKind::Char { terminated }
            | LiteralKind::Byte { terminated }
            | LiteralKind::Str { terminated }
            | LiteralKind::ByteStr { terminated }
            | LiteralKind::CStr { terminated } => !terminated,
            LiteralKind::RawStr { n_hashes }
            | LiteralKind::RawByteStr { n_hashes }
            | LiteralKind::RawCStr { n_hashes } => n_hashes.is_none(),
            LiteralKind::Int { .. } | LiteralKind::Float { .. } => false,
        },
        _ => false,
    }
}

/// Whether the leading comments mark the file as generated, so its generator owns the contents. Only plain `//` comments among the leading comments, inner attributes and block comments after any shebang and frontmatter count, never documentation comments or comments inside or after attributes and block comments: either the conventional `@generated` marker, or a comment ending "do not edit", as Bake's task registry does.
pub(crate) fn is_generated(contents: &str) -> bool {
    let (_, contents) = split_byte_order_mark(contents);
    let lines: Vec<&str> = contents.split_inclusive('\n').collect();
    leading_parts(contents, &lines, prefix_length(contents, &lines))
        .into_iter()
        .filter(|part| {
            part.kind
                == PartKind::LineComment {
                    documentation: false,
                }
        })
        .any(|part| {
            let line = clean_line(lines[part.lines.start])
                .trim()
                .to_ascii_lowercase();
            line.contains("@generated") || line.trim_end_matches('.').ends_with("do not edit")
        })
}

/// Whether the line holds only whitespace, as rustc's lexer reads it, so that other blank-looking lines, such as a no-break space, which rustc rejects, are kept.
fn is_blank(line: &str) -> bool {
    line.chars().all(ra_ap_rustc_lexer::is_whitespace)
}

fn is_license_metadata(line: &str) -> bool {
    let Some(comment) = line.trim_start().strip_prefix("//") else {
        return false;
    };
    let comment = comment.trim_start();
    let comment = comment
        .strip_prefix('!')
        .or_else(|| comment.strip_prefix('/'))
        .unwrap_or(comment)
        .trim_start();
    comment.starts_with("Released under ")
        || comment.starts_with("Copyright,")
        || comment.starts_with("SPDX-License-Identifier:")
}

fn clean_line(line: &str) -> &str {
    line.trim_end_matches('\n').trim_end_matches('\r')
}

fn line_ending(contents: &str) -> &str {
    match contents.find('\n') {
        Some(index) if contents[..index].ends_with('\r') => "\r\n",
        _ => "\n",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copyright() -> Copyright {
        Copyright::new(2020, 2024, "Alice")
    }

    #[test]
    fn recognizes_generated_markers_only_in_leading_comments() {
        assert!(is_generated(
            "// Generated by `cargo bake --regenerate`; do not edit.\nuse base_project as _;\n"
        ));
        assert!(is_generated(
            "// Released under the MIT License.\n\n// @generated by a tool.\nfn main() {}\n"
        ));
        assert!(is_generated(
            "#!/usr/bin/env rustx\n#![allow(clippy::all)]\n// @generated by a tool.\nfn main() {}\n"
        ));
        assert!(is_generated(
            "// Released under the MIT License.\n\n#![allow(clippy::all)]\n\n// @generated by a tool.\nfn main() {}\n"
        ));
        assert!(!is_generated("fn main() {}\n// do not edit\n"));
        assert!(!is_generated(
            "#![allow(clippy::all)]\nfn main() {}\n// @generated by a tool.\n"
        ));
        assert!(!is_generated("// Ordinary documentation.\nfn main() {}\n"));
        assert!(!is_generated(
            "/// Do not edit without updating the schema.\npub struct A;\n"
        ));
        assert!(!is_generated(
            "//! Parses sources marked `@generated`.\npub fn a() {}\n"
        ));
        assert!(!is_generated(
            "// Do not edit this list by hand; run `x` instead.\nfn main() {}\n"
        ));
        // As rustc reads it, four slashes start a plain comment rather than documentation.
        assert!(is_generated("//// @generated by a tool.\nfn main() {}\n"));
    }

    /// Update twice, asserting the result and that a second update leaves it unchanged.
    fn assert_updates_to(contents: &str, expected: &str) {
        let updated = update_source(contents, &[copyright()]);
        assert_eq!(updated, expected, "updating {contents:?}");
        assert_eq!(
            update_source(&updated, &[copyright()]),
            updated,
            "updating {contents:?} again"
        );
    }

    const HEADER: &str = "// Released under the MIT License.\n// Copyright, 2020-2024, by Alice.\n";

    #[test]
    fn inserts_headers_after_a_shebang_and_before_multiline_inner_attributes() {
        let contents = concat!(
            "#!/usr/bin/env rustx\n",
            "#![cfg_attr(\n",
            "    feature = \"doc\",\n",
            "    doc = \"contains ] and escaped \\\" [\"\n",
            ")]\n",
            "//! Module documentation.\n",
            "// Released under an old license.\n",
            "// Copyright, 2018, by Previous Author.\n",
            "// SPDX-License-Identifier: Apache-2.0\n",
            "/// Public API documentation.\n",
            "// Keep this comment.\n",
            "\n",
            "pub fn example() {}\n",
        );

        assert_updates_to(
            contents,
            concat!(
                "#!/usr/bin/env rustx\n",
                "\n",
                "// Released under the MIT License.\n",
                "// Copyright, 2020-2024, by Alice.\n",
                "\n",
                "#![cfg_attr(\n",
                "    feature = \"doc\",\n",
                "    doc = \"contains ] and escaped \\\" [\"\n",
                ")]\n",
                "//! Module documentation.\n",
                "/// Public API documentation.\n",
                "// Keep this comment.\n",
                "\n",
                "pub fn example() {}\n",
            ),
        );
    }

    #[test]
    fn moves_headers_before_crate_inner_attributes_and_replaces_old_metadata() {
        let contents = concat!(
            "#![allow(dead_code)]\r\n",
            "\r\n",
            "// old comment\r\n",
            "// Copyright, 2018, by Previous Author.\r\n",
            "\r\n",
            "fn main() {}\r\n",
        );

        let updated = update_source(contents, &[copyright()]);

        assert_eq!(
            updated,
            "// Released under the MIT License.\r\n// Copyright, 2020-2024, by Alice.\r\n\r\n#![allow(dead_code)]\r\n\r\n// old comment\r\n\r\nfn main() {}\r\n"
        );
        assert_eq!(update_source(&updated, &[copyright()]), updated);
    }

    #[test]
    fn ends_a_source_with_nothing_after_its_header_at_the_header() {
        // base-markdown's `src/contract.rs` holds only an inner attribute and the header; updating it appended a blank line, which rustfmt removes.
        assert_updates_to(
            "#![doc = include_str!(\"../context/contract.md\")]\n\n// Released under the MIT License.\n// Copyright, 2024, by Alice.\n",
            &format!("{HEADER}\n#![doc = include_str!(\"../context/contract.md\")]\n"),
        );
        // The same file as 0.2.9 and earlier wrote it, ending in a blank line.
        assert_updates_to(
            "#![doc = include_str!(\"../context/contract.md\")]\n\n// Released under the MIT License.\n// Copyright, 2024, by Alice.\n\n",
            &format!("{HEADER}\n#![doc = include_str!(\"../context/contract.md\")]\n"),
        );
        assert_updates_to("", HEADER);
        assert_updates_to("\n\n", HEADER);
        assert_updates_to(HEADER, HEADER);
        assert_updates_to(&format!("{HEADER}\n"), HEADER);
        assert_updates_to(
            "//! Module documentation.\n\n",
            &format!("{HEADER}\n//! Module documentation.\n"),
        );
        assert_updates_to(
            "#!/usr/bin/env rustx\n",
            &format!("#!/usr/bin/env rustx\n\n{HEADER}"),
        );
    }

    #[test]
    fn keeps_headers_before_inner_attributes_and_documentation() {
        for rest in [
            "#![doc = include_str!(\"../readme.md\")]\n",
            "#![allow(dead_code)]\n\npub mod corpus;\n",
            "#![allow(dead_code)]\n//! Module documentation.\n\nuse std::io;\n",
            "#![allow(dead_code)]\n#![deny(missing_docs)]\n\n//! Module documentation.\n\nuse std::io;\n",
            "//! Module documentation.\n#![allow(dead_code)]\n\nuse std::io;\n",
            "#![allow(dead_code)]\n\n// A comment.\n\nfn main() {}\n",
        ] {
            let source = format!("{HEADER}\n{rest}");
            assert_updates_to(&source, &source);
        }
    }

    #[test]
    fn moves_headers_from_between_inner_attributes_to_the_top() {
        for (contents, rest) in [
            // As 0.2.9 and earlier wrote headers after an inner attribute.
            (
                "#![allow(dead_code)]\n\n// Released under the MIT License.\n// Copyright, 2024, by Alice.\n\npub mod corpus;\n",
                "#![allow(dead_code)]\n\npub mod corpus;\n",
            ),
            (
                "#![allow(dead_code)]\n\n// Released under the MIT License.\n// Copyright, 2024, by Alice.\n\n//! Module documentation.\n\nuse std::io;\n",
                "#![allow(dead_code)]\n\n//! Module documentation.\n\nuse std::io;\n",
            ),
            // As rustfmt then formats it.
            (
                "#![allow(dead_code)]\n// Released under the MIT License.\n// Copyright, 2024, by Alice.\n\n//! Module documentation.\n\nuse std::io;\n",
                "#![allow(dead_code)]\n\n//! Module documentation.\n\nuse std::io;\n",
            ),
            (
                "#![allow(dead_code)]\n// Released under the MIT License.\n// Copyright, 2024, by Alice.\n#![deny(missing_docs)]\n\nuse std::io;\n",
                "#![allow(dead_code)]\n#![deny(missing_docs)]\n\nuse std::io;\n",
            ),
            // Without a final line ending.
            ("#![allow(dead_code)]", "#![allow(dead_code)]\n"),
        ] {
            assert_updates_to(contents, &format!("{HEADER}\n{rest}"));
        }
    }

    #[test]
    fn writes_sources_that_rustfmt_leaves_unchanged() {
        for contents in [
            "#![doc = include_str!(\"../readme.md\")]\n\n// Released under the MIT License.\n// Copyright, 2024, by Alice.\n",
            "#![allow(dead_code)]\n\n// Released under the MIT License.\n// Copyright, 2024, by Alice.\n\n//! Module documentation.\n\nuse std::io;\n",
            "#![allow(dead_code)]\n#![deny(missing_docs)]\n//! Module documentation.\n\nuse std::io;\n",
            "#![allow(dead_code)]\n\npub mod corpus;\n",
            "//! Module documentation.\n\nuse std::io;\n",
            "#![allow(dead_code)]",
            "",
        ] {
            let updated = update_source(contents, &[copyright()]);
            // rustfmt's `--check` succeeds for standard input even when it prints a difference, so compare the formatted source instead.
            let mut rustfmt = std::process::Command::new("rustfmt")
                .args(["--edition", "2024", "--emit", "stdout"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            std::io::Write::write_all(&mut rustfmt.stdin.take().unwrap(), updated.as_bytes())
                .unwrap();
            let output = rustfmt.wait_with_output().unwrap();
            assert!(output.status.success());
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                updated,
                "rustfmt changes the update of {contents:?}"
            );
        }
    }

    #[test]
    fn handles_a_source_file_without_a_prefix_or_existing_comments() {
        assert_eq!(
            update_source("pub fn example() {}\n", &[]),
            "// Released under the MIT License.\n\npub fn example() {}\n"
        );
        assert_eq!(line_ending("single line"), "\n");
    }

    #[test]
    fn recognizes_license_metadata_in_plain_and_documentation_comments() {
        for line in [
            "// Released under the MIT License.",
            "// Copyright, 2024, by Alice.",
            "// SPDX-License-Identifier: MIT",
            "//! Released under the MIT License.",
            "/// Copyright, 2024, by Alice.",
        ] {
            assert!(is_license_metadata(line), "{line:?}");
        }

        for line in [
            "pub fn example() {}",
            "// Ordinary comment.",
            "//! Module docs.",
        ] {
            assert!(!is_license_metadata(line), "{line:?}");
        }

        assert_eq!(clean_line("// comment\r\n"), "// comment");
    }

    #[test]
    fn keeps_the_blank_line_after_module_documentation() {
        let contents = "// Released under the MIT License.\n// Copyright, 2024, by Alice.\n\n//! Module documentation.\n\nuse std::io;\n";
        let updated = update_source(contents, &[copyright()]);
        assert_eq!(
            updated,
            "// Released under the MIT License.\n// Copyright, 2020-2024, by Alice.\n\n//! Module documentation.\n\nuse std::io;\n"
        );
        assert_eq!(
            update_source(&updated, &[copyright()]),
            updated,
            "updates are idempotent"
        );
        assert_eq!(
            update_source("//! Docs.\nuse std::io;\n", &[copyright()]),
            "// Released under the MIT License.\n// Copyright, 2020-2024, by Alice.\n\n//! Docs.\nuse std::io;\n",
            "no blank line is added where there was none"
        );
    }

    #[test]
    fn keeps_the_insides_of_inner_attributes_verbatim() {
        for rest in [
            // A string that spans lines, as crate documentation.
            "#![doc = \"\n// Copyright, 2001, by Someone.\n\"]\n",
            // A comment inside an attribute, separated by blank lines.
            "#![cfg_attr(\n    feature = \"doc\",\n\n    // Copyright, 2001, by Someone.\n\n    allow(dead_code)\n)]\n\nfn main() {}\n",
        ] {
            assert_updates_to(rest, &format!("{HEADER}\n{rest}"));
        }
        assert!(!is_generated(
            "#![doc = \"\n// @generated by a tool.\n\"]\nfn main() {}\n"
        ));
    }

    #[test]
    fn ignores_brackets_in_strings_and_comments_when_finding_the_end_of_an_inner_attribute() {
        for (attribute, removed) in [
            ("#![doc = \"[ \\\" [\"]\n", ""),
            ("#![doc = r#\"[ \"quoted\" \\\"#]\n", ""),
            ("#![doc = br\"[\\\"]\n", ""),
            ("#![allow(dead_code)] // See [the guide.\n", ""),
            ("#![allow(dead_code)] /* See [ /* nested [ */ */\n", ""),
            ("#![allow(dead_code)] /* See\n[the guide. */\n", ""),
            // Not an attribute's end, so the leading section ends before it, and old metadata after it stays.
            (
                "#![allow(dead_code)] fn first() {}\n",
                "// Copyright, 2018, by Previous Author.\n\n",
            ),
            (
                "#![allow(dead_code)\n",
                "// Copyright, 2018, by Previous Author.\n\n",
            ),
        ] {
            let body = "fn main() {}\n\n// Copyright, 2001, by Someone.\n";
            assert_updates_to(
                &format!("{attribute}// Copyright, 2018, by Previous Author.\n\n{body}"),
                &format!(
                    "{HEADER}\n{attribute}{removed}{}{body}",
                    if removed.is_empty() { "\n" } else { "" }
                ),
            );
            assert!(
                !is_generated(&format!(
                    "{attribute}fn main() {{}}\n// @generated by a tool.\n"
                )),
                "{attribute:?}"
            );
        }
    }

    #[test]
    fn ignores_brackets_and_quotes_in_char_literals_when_finding_the_end_of_an_inner_attribute() {
        for attribute in [
            "#![a(c = ']')]\n",
            "#![a(c = '[')]\n",
            "#![a(c = '\"')]\n",
            // Escapes, whose closing quote does not follow the first character after the opening quote.
            "#![a(c = ['\\'',']'])]\n",
            "#![a(c = ['\\\\',']'])]\n",
            "#![a(c = ['\\u{5d}',']'])]\n",
            "#![a(c = b'[')]\n",
            // Lifetimes, which have no closing quote.
            "#![a(&'a [u8], ']')]\n",
            "#![a(&'static [u8], '\"')]\n",
        ] {
            let body = "fn main() {}\n\n// Copyright, 2001, by Someone.\n";
            assert_updates_to(
                &format!("{attribute}// Copyright, 2018, by Previous Author.\n\n{body}"),
                &format!("{HEADER}\n{attribute}\n{body}"),
            );
            assert!(
                is_generated(&format!(
                    "{attribute}// @generated by a tool.\nfn main() {{}}\n"
                )),
                "{attribute:?}"
            );
        }
    }

    #[test]
    fn reads_inner_attributes_as_rustc_lexes_them() {
        for attribute in [
            // A literal's suffix or a raw identifier before a string with an escaped quote, rather than a raw string prefix.
            "#![a(1r\"\\\"]\")]\n",
            "#![a(\"\"r\"\\\"]\")]\n",
            "#![a(r#bar\"\\\"]\")]\n",
            "#![a(r#r\"\\\"[\")]\n",
            // Raw strings with hashes, holding quotes and brackets.
            "#![doc = r##\"[ \"# ]\"##]\n",
            "#![doc = br#\"[\"#]\n",
            "#![a(b\"]\", c\"[\", cr#\"]\"#)]\n",
            // Before Rust 2024, a `#` before a string, or another `#`, rather than a guarded string prefix.
            "#![a(#\"]\"#)]\n",
            "#![a(##)]\n",
            // Multibyte text.
            "#![doc = \"日本語 ]\"] // ü [\n",
            "#![a('日', ']')]\n",
            // Comments and line breaks between tokens.
            "#!\n[allow(dead_code)]\n",
            "#! // A comment.\n[allow(dead_code)]\n",
            "# /* ] */ ! [allow(dead_code)]\n",
            "#![allow(\n    dead_code, // ]\n    /* [ */\n)]\n",
            "#![allow(dead_code)] /* ] */ // [\n",
            // Braces, as in a macro call.
            "#![doc = concat!{\"]\", {\"[\"}}]\n",
        ] {
            let body = "fn main() {}\n\n// Copyright, 2001, by Someone.\n";
            assert_updates_to(
                &format!("{attribute}// Copyright, 2018, by Previous Author.\n\n{body}"),
                &format!("{HEADER}\n{attribute}\n{body}"),
            );
            assert!(
                is_generated(&format!(
                    "{attribute}// @generated by a tool.\nfn main() {{}}\n"
                )),
                "{attribute:?}"
            );
        }
    }

    #[test]
    fn keeps_what_rustc_cannot_lex_after_the_header() {
        // Each ends the leading section, so the source from it on is kept as it was, including the old metadata after it.
        for attribute in [
            // Unterminated literals and comments, which run to the end of the source.
            "#![doc = \"unterminated]\n",
            "#![doc = r#\"unterminated\"]\n",
            "#![a('[)]\n",
            "#![a /* unterminated ]\n",
            "#![allow(dead_code)] /* unterminated\n",
            // Unbalanced delimiters.
            "#![a(]\n",
            "#![a)]\n",
            "#![a(\n",
            // Delimiters that balance in number but not in kind.
            "#![a{)]\n",
            // An unterminated char literal, with what would close the attribute on the next line.
            "#![a('[)\n]\n",
            // A raw string with an invalid start.
            "#![a(r#~)]\n",
            // A no-break space, which is not whitespace in Rust.
            "\u{a0}\n",
            // Not inner attributes.
            "#[derive(Debug)]\n",
            "// A comment.\n#!a\n",
        ] {
            let body = "fn main() {}\n\n// Copyright, 2001, by Someone.\n";
            let source = format!("{attribute}// Copyright, 2018, by Previous Author.\n\n{body}");
            assert_updates_to(&source, &format!("{HEADER}\n{source}"));
            assert!(
                !is_generated(&format!(
                    "{attribute}// @generated by a tool.\nfn main() {{}}\n"
                )),
                "{attribute:?}"
            );
        }
        // A source ending in an attribute's `#` or `#!`.
        for end in ["#", "#!"] {
            assert_updates_to(
                &format!("// Copyright, 2018, by Previous Author.\n{end}"),
                &format!("{HEADER}\n{end}\n"),
            );
        }
    }

    #[test]
    fn keeps_license_metadata_in_the_code() {
        let body = "fn main() {\n    // Copyright, 2001, by Someone.\n}\n";
        assert_updates_to(body, &format!("{HEADER}\n{body}"));
        assert_updates_to(
            &format!("#![allow(dead_code)]\n\n{body}"),
            &format!("{HEADER}\n#![allow(dead_code)]\n\n{body}"),
        );
    }

    #[test]
    fn removes_indented_license_metadata() {
        assert_updates_to(
            "    // Copyright, 2018, by Previous Author.\nfn main() {}\n",
            &format!("{HEADER}\nfn main() {{}}\n"),
        );
        assert_updates_to(
            "  #![allow(dead_code)]\n  // Copyright, 2018, by Previous Author.\nfn main() {}\n",
            &format!("{HEADER}\n  #![allow(dead_code)]\nfn main() {{}}\n"),
        );
    }

    #[test]
    fn ends_a_source_without_a_final_line_ending_with_its_own_line_ending() {
        assert_updates_to(
            "#![allow(dead_code)]\r\n\r\nfn main() {}",
            &format!(
                "{}\r\n#![allow(dead_code)]\r\n\r\nfn main() {{}}\r\n",
                HEADER.replace('\n', "\r\n")
            ),
        );
    }

    #[test]
    fn keeps_a_byte_order_mark_first() {
        for (contents, rest) in [
            (
                "\u{feff}// Released under the MIT License.\n// Copyright, 2024, by Alice.\n\nfn main() {}\n",
                "\nfn main() {}\n",
            ),
            ("\u{feff}#![allow(dead_code)]\n", "\n#![allow(dead_code)]\n"),
            ("\u{feff}", ""),
        ] {
            assert_updates_to(contents, &format!("\u{feff}{HEADER}{rest}"));
        }
        assert_updates_to(
            "\u{feff}#!/usr/bin/env rustx\nfn main() {}\n",
            &format!("\u{feff}#!/usr/bin/env rustx\n\n{HEADER}\nfn main() {{}}\n"),
        );
        assert!(is_generated(
            "\u{feff}// @generated by a tool.\nfn main() {}\n"
        ));
    }

    #[test]
    fn removes_old_headers_after_block_comments() {
        for documentation in [
            "/*! Crate documentation. */\n",
            "/*!\n * Crate documentation, with [brackets and /* nested */ comments.\n */\n",
        ] {
            assert_updates_to(
                &format!(
                    "{documentation}\n// Released under the MIT License.\n// Copyright, 2018, by Previous Author.\n\nfn main() {{}}\n"
                ),
                &format!("{HEADER}\n{documentation}\nfn main() {{}}\n"),
            );
        }
        // An unterminated block comment ends the leading section.
        assert_updates_to(
            "/* Unterminated.\n// Copyright, 2001, by Someone.\n",
            &format!("{HEADER}\n/* Unterminated.\n// Copyright, 2001, by Someone.\n"),
        );
    }

    #[test]
    fn recognizes_inner_attributes_with_a_space_after_the_hash_and_bang() {
        assert_updates_to(
            "#! [allow(dead_code)]\n\n// Copyright, 2018, by Previous Author.\n\nfn main() {}\n",
            &format!("{HEADER}\n#! [allow(dead_code)]\n\nfn main() {{}}\n"),
        );
        assert!(is_generated(
            "#! [allow(clippy::all)]\n// @generated by a tool.\nfn main() {}\n"
        ));
        assert_eq!(shebang_length("#!\t[allow(dead_code)]\n"), 0);
        assert_eq!(shebang_length("#!/usr/bin/env rustx\n"), 1);
        // As rustc decides, comments may separate the `#!` of an attribute from its `[`.
        assert_eq!(shebang_length("#! // A comment.\n[allow(dead_code)]\n"), 0);
    }

    #[test]
    fn keeps_license_like_documentation_after_later_inner_attributes() {
        // As 0.2.9 and earlier, documentation comments that look like license metadata are removed only directly after the attributes that start the source.
        assert_updates_to(
            "#![allow(dead_code)]\n//! Copyright, 2018, by Previous Author.\n\nfn main() {}\n",
            &format!("{HEADER}\n#![allow(dead_code)]\n\nfn main() {{}}\n"),
        );
        for rest in [
            "#![allow(dead_code)]\n//! Copyright, 2001, by Someone.\n\nfn main() {}\n",
            "/*! Crate documentation. */\n#![allow(dead_code)]\n/// Copyright, 2001, by Someone.\nfn main() {}\n",
        ] {
            let source = format!("{HEADER}\n{rest}");
            assert_updates_to(&source, &source);
        }
        let rest = "#![allow(dead_code)]\n/* A comment. */\n//! Copyright, 2001, by Someone.\n";
        assert_updates_to(rest, &format!("{HEADER}\n{rest}"));
    }

    #[test]
    fn keeps_frontmatter_first() {
        let script =
            "#!/usr/bin/env -S cargo +nightly -Zscript\n---\n[dependencies]\nclap = \"4\"\n---\n";
        // A shebang, frontmatter and inner attributes, with the header after the frontmatter, which rustc requires to come first.
        assert_updates_to(
            &format!("{script}#![allow(dead_code)]\n\nfn main() {{}}\n"),
            &format!("{script}\n{HEADER}\n#![allow(dead_code)]\n\nfn main() {{}}\n"),
        );
        // Frontmatter without a shebang, with an infostring, longer fences, a fence inside it, and line breaks before it.
        let frontmatter = "\n----cargo\n[package]\ndescription = \"\"\"\n---\n\"\"\"\n----\n";
        assert_updates_to(
            &format!("{frontmatter}fn main() {{}}\n"),
            &format!("{frontmatter}\n{HEADER}\nfn main() {{}}\n"),
        );
        // An old header after frontmatter, as 0.2.9 and earlier wrote it, is replaced.
        assert_updates_to(
            "---\n[package]\n---\n// Released under the MIT License.\n// Copyright, 2018, by Previous Author.\n\n//! Documentation.\n\nfn main() {}\n",
            &format!("---\n[package]\n---\n\n{HEADER}\n//! Documentation.\n\nfn main() {{}}\n"),
        );
        assert_updates_to(
            "\u{feff}---\r\n[package]\r\n---",
            &format!(
                "\u{feff}---\r\n[package]\r\n---\r\n\r\n{}",
                HEADER.replace('\n', "\r\n")
            ),
        );
        assert!(is_generated(
            "#!/usr/bin/env cargo\n---\n---\n// @generated by a tool.\nfn main() {}\n"
        ));
        assert!(!is_generated(
            "---\n// @generated by a tool.\n---\nfn main() {}\n"
        ));
    }

    #[test]
    fn keeps_frontmatter_rustc_rejects_after_the_header() {
        for frontmatter in [
            // Without a closing fence as long as its opening one, which the lexer extends to the next line starting with `use`, `//!`, `#![` or dashes, or to the end of the source.
            "----\n[package]\n---\n",
            "---\n[package]\n",
            // After something other than whitespace on its line, or after a comment.
            " ---\n[package]\n---\n",
            "// A comment.\n---\n[package]\n---\n",
        ] {
            let source =
                format!("{frontmatter}// Copyright, 2018, by Previous Author.\nuse std::io;\n");
            assert_updates_to(&source, &format!("{HEADER}\n{source}"));
        }
    }

    #[test]
    fn keeps_documentation_that_makes_a_shebang() {
        // rustc reads `#!` as a shebang here only because of the documentation comment, which it then rejects; without it, the `#!` would start an inner attribute, so the comment is kept.
        assert_updates_to(
            "#!\n/// Copyright, 2018, by Previous Author.\n[a]\n",
            &format!("#!\n\n{HEADER}\n/// Copyright, 2018, by Previous Author.\n[a]\n"),
        );
        assert_updates_to(
            "#!\n// Copyright, 2018, by Previous Author.\n//! Copyright, 2018, by Previous Author.\n/* A comment. */\n[a]\n",
            &format!(
                "#!\n\n{HEADER}\n//! Copyright, 2018, by Previous Author.\n/* A comment. */\n[a]\n"
            ),
        );
        assert!(is_shebang_by_documentation("#! /// A comment.\n[a]\n"));
        assert!(!is_shebang_by_documentation(
            "#!/usr/bin/env rustx\n/// A comment.\n"
        ));
        assert!(!is_shebang_by_documentation("#![allow(dead_code)]\n"));
        assert!(!is_shebang_by_documentation("#!\n/// A comment.\n"));
        assert!(!is_shebang_by_documentation("fn main() {}\n"));
    }
}
