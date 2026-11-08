// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::authorship::Copyright;

/// Write the license header at the top of the source, after any shebang and
/// before inner attributes and documentation, replacing the license metadata
/// in its leading comments.
///
/// rustfmt removes the blank lines around a comment that sits between inner
/// attributes, including `//!` documentation, so a header placed after a
/// leading `#![...]` attribute is not stable under formatting when more
/// follow. Placing it first is, and the source after it is kept as it was,
/// apart from the removed metadata and the blank lines that separated it.
pub(crate) fn update_source(contents: &str, copyrights: &[Copyright]) -> String {
    let lines: Vec<&str> = contents.split_inclusive('\n').collect();
    let newline = line_ending(contents);
    let shebang_length = shebang_length(&lines);
    let leading_length = leading_length(&lines, shebang_length);

    let mut remainder: Vec<&str> = Vec::new();
    let mut index = shebang_length;
    while index < lines.len() {
        let line = lines[index];
        if index < leading_length && is_comment(line) && is_license_metadata(clean_line(line)) {
            // Remove the metadata, and the blank lines that separated it from what follows when it was separated from what precedes it too, so a blank line between kept comments and code remains.
            index += 1;
            if remainder.last().is_none_or(|line| is_blank(line)) {
                while index < leading_length && is_blank(lines[index]) {
                    index += 1;
                }
            }
            continue;
        }
        remainder.push(line);
        index += 1;
    }
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

    let mut output = String::new();
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

/// Append a line, ending it with the source's line ending if it has none.
fn push_line(output: &mut String, line: &str, newline: &str) {
    output.push_str(line);
    if !line.ends_with('\n') {
        output.push_str(newline);
    }
}

/// Count the shebang line, which must stay first, if there is one.
fn shebang_length(lines: &[&str]) -> usize {
    usize::from(
        lines
            .first()
            .is_some_and(|line| line.starts_with("#!") && !line.starts_with("#![")),
    )
}

/// Find the end of the leading section after the shebang: the blank lines,
/// `//` comments and inner attributes before the first item.
fn leading_length(lines: &[&str], start: usize) -> usize {
    let mut index = start;
    while let Some(line) = lines.get(index) {
        if is_blank(line) || is_comment(line) {
            index += 1;
        } else if clean_line(line).starts_with("#![") {
            index = attribute_end(lines, index);
        } else {
            break;
        }
    }
    index
}

/// Find the line after the inner attribute starting at `start`, which may span several lines.
fn attribute_end(lines: &[&str], start: usize) -> usize {
    let mut index = start;
    let mut open_brackets = 0isize;
    let mut in_string = false;
    let mut escaped = false;
    while let Some(line) = lines.get(index) {
        for character in clean_line(line).chars() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    in_string = false;
                }
                continue;
            }

            match character {
                '"' => in_string = true,
                '[' => open_brackets += 1,
                ']' => open_brackets -= 1,
                _ => {}
            }
        }
        index += 1;
        if open_brackets <= 0 {
            break;
        }
    }
    index
}

/// Whether the leading comments mark the file as generated, so its generator
/// owns the contents. Only plain `//` comments among the leading comments and
/// inner attributes after any shebang count, never documentation comments:
/// either the conventional `@generated` marker, or a comment ending "do not
/// edit", as Bake's task registry does. Block comments are not recognized.
pub(crate) fn is_generated(contents: &str) -> bool {
    let lines: Vec<&str> = contents.split_inclusive('\n').collect();
    let start = shebang_length(&lines);
    lines[start..leading_length(&lines, start)]
        .iter()
        .filter(|line| is_comment(line))
        .map(|line| clean_line(line).trim())
        .filter(|line| !line.starts_with("///") && !line.starts_with("//!"))
        .any(|line| {
            let line = line.to_ascii_lowercase();
            line.contains("@generated") || line.trim_end_matches('.').ends_with("do not edit")
        })
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
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
}
