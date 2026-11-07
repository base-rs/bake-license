// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::authorship::Copyright;

pub(crate) fn update_source(contents: &str, copyrights: &[Copyright]) -> String {
    let lines: Vec<&str> = contents.split_inclusive('\n').collect();
    let newline = line_ending(contents);
    let prefix_length = prefix_length(&lines);

    let mut leading_comments = Vec::new();
    while let Some(line) = lines.get(prefix_length + leading_comments.len()) {
        let clean = clean_line(line);
        if clean.trim().is_empty() || clean.trim_start().starts_with("//") {
            leading_comments.push(*line);
        } else {
            break;
        }
    }

    let mut documentation: Vec<_> = leading_comments
        .into_iter()
        .filter(|line| !is_license_metadata(clean_line(line)))
        .collect();
    while documentation
        .first()
        .is_some_and(|line| clean_line(line).trim().is_empty())
    {
        documentation.remove(0);
    }
    // Keep one blank line between the kept comments and the code when the source had one, such as after module documentation.
    let mut separated = false;
    while documentation
        .last()
        .is_some_and(|line| clean_line(line).trim().is_empty())
    {
        documentation.pop();
        separated = true;
    }
    let separated = separated && !documentation.is_empty();

    let body_start = prefix_length
        + lines[prefix_length..]
            .iter()
            .take_while(|line| {
                let clean = clean_line(line);
                clean.trim().is_empty() || clean.trim_start().starts_with("//")
            })
            .count();

    let mut output = String::new();
    for line in &lines[..prefix_length] {
        output.push_str(line);
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
    output.push_str(newline);

    for line in documentation {
        output.push_str(line);
    }
    if separated {
        output.push_str(newline);
    }
    for line in &lines[body_start..] {
        output.push_str(line);
    }

    output
}

/// Count the shebang and inner-attribute lines that precede the leading comments.
fn prefix_length(lines: &[&str]) -> usize {
    let mut prefix_length = 0;

    if lines
        .first()
        .is_some_and(|line| line.starts_with("#!") && !line.starts_with("#!["))
    {
        prefix_length += 1;
    }

    while lines
        .get(prefix_length)
        .is_some_and(|line| clean_line(line).starts_with("#!["))
    {
        let mut open_brackets = 0isize;
        let mut in_string = false;
        let mut escaped = false;
        while let Some(line) = lines.get(prefix_length) {
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
            prefix_length += 1;
            if open_brackets <= 0 {
                break;
            }
        }
    }
    prefix_length
}

/// Whether the leading comments mark the file as generated, so its generator
/// owns the contents. Only plain `//` comments after any shebang and inner
/// attributes count, never documentation comments: either the conventional
/// `@generated` marker, or a comment ending "do not edit", as Bake's task
/// registry does. Block comments are not recognized.
pub(crate) fn is_generated(contents: &str) -> bool {
    let lines: Vec<&str> = contents.split_inclusive('\n').collect();
    lines[prefix_length(&lines)..]
        .iter()
        .map(|line| clean_line(line).trim())
        .take_while(|line| line.is_empty() || line.starts_with("//"))
        .filter(|line| !line.starts_with("///") && !line.starts_with("//!"))
        .any(|line| {
            let line = line.to_ascii_lowercase();
            line.contains("@generated") || line.trim_end_matches('.').ends_with("do not edit")
        })
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
        assert!(!is_generated("fn main() {}\n// do not edit\n"));
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

    #[test]
    fn inserts_headers_after_a_shebang_and_multiline_inner_attributes() {
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

        let updated = update_source(contents, &[copyright()]);

        assert_eq!(
            updated,
            concat!(
                "#!/usr/bin/env rustx\n",
                "#![cfg_attr(\n",
                "    feature = \"doc\",\n",
                "    doc = \"contains ] and escaped \\\" [\"\n",
                ")]\n",
                "\n",
                "// Released under the MIT License.\n",
                "// Copyright, 2020-2024, by Alice.\n",
                "\n",
                "//! Module documentation.\n",
                "/// Public API documentation.\n",
                "// Keep this comment.\n",
                "\n",
                "pub fn example() {}\n",
            )
        );
    }

    #[test]
    fn preserves_crate_inner_attributes_and_replaces_old_metadata() {
        let contents = concat!(
            "#![allow(dead_code)]\r\n",
            "\r\n",
            "// old comment\r\n",
            "// Copyright, 2018, by Previous Author.\r\n",
            "\r\n",
            "fn main() {}\r\n",
        );

        let updated = update_source(contents, &[copyright()]);

        assert!(updated.starts_with(
            "#![allow(dead_code)]\r\n\r\n// Released under the MIT License.\r\n// Copyright, 2020-2024, by Alice.\r\n\r\n// old comment\r\n"
        ));
        assert!(updated.ends_with("fn main() {}\r\n"));
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
