// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::authorship::Copyright;
use std::ops::Range;

/// Write the license header at the top of the source, after any byte order mark and shebang and before inner attributes and documentation, replacing the license metadata in its leading comments.
///
/// rustfmt removes the blank lines around a comment that sits between inner attributes, including `//!` documentation, so a header placed after a leading `#![...]` attribute is not stable under formatting when more follow. Placing it first is, and the source after it is kept as it was, apart from the removed metadata, the blank lines that separated it, and the blank lines at its end, which rustfmt removes.
pub(crate) fn update_source(contents: &str, copyrights: &[Copyright]) -> String {
    let (byte_order_mark, contents) = split_byte_order_mark(contents);
    let lines: Vec<&str> = contents.split_inclusive('\n').collect();
    let newline = line_ending(contents);
    let shebang_length = shebang_length(contents);
    let leading = leading_parts(&lines, shebang_length);

    let mut remainder: Vec<&str> = Vec::new();
    // As 0.2.9 and earlier, remove license metadata in documentation comments only among the leading comments directly after the shebang and the inner attributes that start the source: documentation comments after a block comment, or after an inner attribute that follows comments or blank lines, are the crate's documentation, and are kept.
    let mut removes_documentation = true;
    // Whether a blank line or comment has been seen, ending the inner attributes that start the source.
    let mut after_comment = false;
    let mut skips_blank_lines = false;
    for part in &leading {
        let line = lines[part.start];
        if !is_blank(line) && !is_comment(line) {
            // Keep inner attributes and block comments verbatim, including any lines inside them that look like comments.
            if after_comment || !is_inner_attribute(line) {
                removes_documentation = false;
            }
            remainder.extend(&lines[part.clone()]);
            skips_blank_lines = false;
            continue;
        }
        after_comment = true;
        if skips_blank_lines && is_blank(line) {
            continue;
        }
        skips_blank_lines = false;
        if is_license_metadata(clean_line(line))
            && (removes_documentation || !is_documentation_comment(line))
        {
            // Remove the metadata, and the blank lines that separated it from what follows when it was separated from what precedes it too, so a blank line between kept comments and code remains.
            skips_blank_lines = remainder.last().is_none_or(|line| is_blank(line));
            continue;
        }
        remainder.push(line);
    }
    let body = leading.last().map_or(shebang_length, |part| part.end);
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
    for line in &lines[..shebang_length] {
        push_line(&mut output, line, newline);
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

/// Count the shebang line, which must stay first, if there is one: a source starting with `#!` is an inner attribute instead when `[` follows, after optional whitespace.
fn shebang_length(contents: &str) -> usize {
    usize::from(
        contents
            .strip_prefix("#!")
            .is_some_and(|rest| !rest.trim_start().starts_with('[')),
    )
}

/// Split the leading section after the shebang, before the first item, into its parts: each blank line and `//` comment, and each inner attribute and block comment, which may span several lines.
fn leading_parts(lines: &[&str], start: usize) -> Vec<Range<usize>> {
    let mut parts = Vec::new();
    let mut index = start;
    while let Some(line) = lines.get(index) {
        let end = if is_blank(line) || is_comment(line) {
            Some(index + 1)
        } else if is_inner_attribute(line) || line.trim_start().starts_with("/*") {
            span_end(lines, index)
        } else {
            None
        };
        let Some(end) = end else {
            break;
        };
        parts.push(index..end);
        index = end;
    }
    parts
}

/// The lexical context of a character in [`span_end`].
enum Context {
    Code,
    String,
    /// A raw string, closed by `"` and this many `#`.
    RawString(usize),
    /// Block comments, which nest, at this depth.
    BlockComment(usize),
}

/// Find the line after the inner attribute or block comment starting at `start`, which may span several lines, ignoring brackets in strings, char and byte literals, and comments. Returns `None` when it is unterminated, or code follows it on its last line, so the leading section ends before it.
fn span_end(lines: &[&str], start: usize) -> Option<usize> {
    let mut context = Context::Code;
    let mut open_brackets = 0usize;
    // Whether code starts the span, so that it is an attribute, which ends at its closing bracket rather than at the end of a comment before it.
    let mut started = false;
    let mut closed = false;
    for (index, line) in lines.iter().enumerate().skip(start) {
        let mut characters = clean_line(line).chars().peekable();
        // The `#` counted since an `r` that may start a raw string, which it does only when the `"` follows in the same token, so the line's end clears it.
        let mut raw_hashes = None;
        while let Some(character) = characters.next() {
            match context {
                Context::String => match character {
                    '\\' => {
                        characters.next();
                    }
                    '"' => context = Context::Code,
                    _ => {}
                },
                Context::RawString(hashes) => {
                    if character == '"'
                        && (0..hashes).all(|_| characters.next_if_eq(&'#').is_some())
                    {
                        context = Context::Code;
                    }
                }
                Context::BlockComment(depth) => {
                    if character == '*' && characters.next_if_eq(&'/').is_some() {
                        context = if depth == 1 {
                            closed |= !started;
                            Context::Code
                        } else {
                            Context::BlockComment(depth - 1)
                        };
                    } else if character == '/' && characters.next_if_eq(&'*').is_some() {
                        context = Context::BlockComment(depth + 1);
                    }
                }
                Context::Code => {
                    // As in Rust's lexer, the prefix applies only to the character that follows it, so whitespace and comments end it.
                    let prefix_hashes = raw_hashes.take();
                    if character == '/' && characters.next_if_eq(&'/').is_some() {
                        // The rest of the line is a comment.
                        break;
                    }
                    if character == '/' && characters.next_if_eq(&'*').is_some() {
                        context = Context::BlockComment(1);
                        continue;
                    }
                    if character.is_whitespace() {
                        continue;
                    }
                    if closed {
                        return None;
                    }
                    started = true;
                    match character {
                        'r' => raw_hashes = Some(0),
                        '#' => raw_hashes = prefix_hashes.map(|hashes| hashes + 1),
                        '"' => {
                            context = prefix_hashes.map_or(Context::String, Context::RawString);
                        }
                        '\'' => {
                            // A char or byte literal, such as `'['` or `b'\''`, is an escape, or one character and its closing quote; otherwise the quote starts a lifetime or label, such as `'static`, whose name the scan reads as code.
                            let mut lookahead = characters.clone();
                            match lookahead.next() {
                                Some('\\') => {
                                    // Skip the escaped character, which may be a quote, and the rest of a `\x` or `\u{...}` escape, through the closing quote.
                                    characters.nth(1);
                                    while characters.next().is_some_and(|next| next != '\'') {}
                                }
                                Some(_) if lookahead.next() == Some('\'') => characters = lookahead,
                                _ => {}
                            }
                        }
                        '[' => open_brackets += 1,
                        ']' => {
                            open_brackets = open_brackets.saturating_sub(1);
                            closed = open_brackets == 0;
                        }
                        _ => {}
                    }
                }
            }
        }
        if closed && matches!(context, Context::Code) {
            return Some(index + 1);
        }
    }
    None
}

/// Whether the leading comments mark the file as generated, so its generator owns the contents. Only plain `//` comments among the leading comments, inner attributes and block comments after any shebang count, never documentation comments or lines inside attributes and block comments: either the conventional `@generated` marker, or a comment ending "do not edit", as Bake's task registry does.
pub(crate) fn is_generated(contents: &str) -> bool {
    let (_, contents) = split_byte_order_mark(contents);
    let lines: Vec<&str> = contents.split_inclusive('\n').collect();
    leading_parts(&lines, shebang_length(contents))
        .into_iter()
        .map(|part| lines[part.start])
        .filter(|line| is_comment(line) && !is_documentation_comment(line))
        .any(|line| {
            let line = clean_line(line).trim().to_ascii_lowercase();
            line.contains("@generated") || line.trim_end_matches('.').ends_with("do not edit")
        })
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

fn is_documentation_comment(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("///") || line.starts_with("//!")
}

/// Whether the line starts an inner attribute: `#!` followed by `[`, after optional whitespace.
fn is_inner_attribute(line: &str) -> bool {
    line.trim_start()
        .strip_prefix("#!")
        .is_some_and(|rest| rest.trim_start().starts_with('['))
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
            // An identifier ending in `r`, before a string after a line break or comment, which does not start a raw string.
            ("#![a(m!(r\n\"\\\"]\" x))]\n", ""),
            ("#![a(m!(br\n\"\\\"]\" x))]\n", ""),
            ("#![a(m!(r/**/\"\\\"]\" x))]\n", ""),
            ("#![a(m!(r// See [\n\"\\\"]\" x))]\n", ""),
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
            "#![a(c = ['🦀', 'é', ']'])]\n",
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
}
