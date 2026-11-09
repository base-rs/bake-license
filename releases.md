# Releases

## Unreleased

- Require `bake` 0.20 or later, instead of any 0.x version, so that Cargo resolves this crate to the same `bake` as a task package that requires `bake` 0.20.2. In a workspace whose `rust-version` is below the 1.88 that `bake` 0.20.2 declares, Cargo resolved the old requirement to `bake` 0.19.2, which declares none, and the task package failed to compile against two `bake` versions.

## v0.2.10

- Declare Rust 1.88 as the minimum supported version in the package's `rust-version`, so Cargo reports an older toolchain clearly and resolves dependencies that support it; `bake` 0.20's `bake-macros` uses let chains, which need Rust 1.88, one release above the Base default. The test workflow checks the package with that version.
- Write Rust license headers that `cargo fmt` leaves unchanged, so that a release's `license:update` no longer fails `cargo fmt --check`. The header now goes first, after any shebang and before inner attributes and `//!` documentation, since `rustfmt` removes the blank lines around a comment between inner attributes; an existing header after a leading `#![...]` attribute moves to the top on the next update. As `rustfmt` does, every updated source now ends at its last line, without trailing blank lines, and with a final line ending if it had none, so a file with nothing after its header no longer ends with a blank line, and a shebang on a source's last line, without a final line ending, is no longer followed directly by the header, without a blank line between them, until the next update. A `@generated` or "do not edit" comment after inner attributes now marks a source as generated when its license header comes before them, as it did when the header followed them.
- Read the leading inner attributes and comments with rustc's own lexer, from the `ra-ap-rustc_lexer` crate, instead of scanning their text, so that brackets, quotes and comment markers in strings, raw strings, char and byte literals, and comments, such as in `#![allow(dead_code)] // see [`, `#![a(c = '[')]` or `#![doc = r#"[ \"#]`, no longer make the update treat the rest of the source as part of the attribute, writing the new header at the end of the source, and another on every update, while keeping the old header and ignoring generated markers. The leading comments now end at an attribute that code follows on the same line, and at an attribute or block comment that rustc cannot lex, with an unterminated literal or comment or unbalanced delimiters, which is kept after the header as it was, with the rest of the source. As rustc reads them, a `////` comment is a plain comment, which can mark a source as generated, and only what Rust counts as whitespace is blank: a line holding only the left-to-right or right-to-left mark, U+200E or U+200F, is now blank, and may be removed with the old metadata around it, while a line holding a no-break space, an ideographic space, U+3000, or another Unicode space that is not whitespace in Rust is not blank: it ends the leading comments, and is kept.
- Keep a leading byte order mark first, with the header after it, instead of writing the header before it and keeping the old header after it. Recognize `#! [...]`, with whitespace, comments or line breaks before the `[`, as an inner attribute rather than a shebang, as rustc does, and recognize indented inner attributes. Keep the license-like documentation comments that make rustc read a `#!` before such a `[` as a shebang, which it then rejects, so that the next update still reads it as one. Remove an old header after an indented inner attribute or a leading block comment, such as `/*! ... */` documentation, instead of keeping it below the new one, and look for generated markers after them.
- Write the header after the frontmatter of a cargo script, the `---` fenced block that rustc requires to come first, after any shebang, instead of before the frontmatter, without a shebang, or between the shebang and the frontmatter, which rustc rejects, and replace an old header after the frontmatter. Frontmatter is kept first only when no whitespace comes before its opening fence on its line and a later line starts with at least as many dashes as that fence, closing it; other frontmatter is kept after the header, as before.

## v0.2.9

- Add `license:check`, which fails, naming each file and the fix, when `license:update` would change `license.md`, the README License section, or a Rust source header, without writing anything, so pull request CI can catch a missing or stale header before a release adds it. It refuses shallow clones, whose history gives incomplete copyright lines. The library exposes it as `check`.
- Write `license.md` as `base_markdown::normalize_markdown` writes it, one line per paragraph with backslash line breaks between copyright lines, so that normalizing it after an update, as the Base project release hook does, leaves it unchanged. Require base-markdown 0.1.3, which provides it.

## v0.2.8

- Read contributions made outside the repository's Git history from `.contributors.toml`, so that copyright lines for work such as code ported from another project survive `license:update`. Each `[[contribution]]` gives an `author` with `name` and optional `email`, a `time`, and an optional `path`; it counts like a commit to that file, which must be tracked, or, without a path, to the repository's `license.md`. Authors are mapped through `.mailmap`, and bots are skipped. Malformed entries and untracked paths fail the update with their position.
- Disable commit signing in the test fixtures' commits, so the tests no longer depend on the user's Git signing configuration.

## v0.2.7

- Keep the blank line between leading documentation, such as module `//!` comments, and the code that follows when updating license headers, instead of removing it on every update.

## v0.2.6

- Leave generated Rust sources, whose leading comments say `@generated` or "do not edit", without license headers. Previously the update stamped a header onto Bake's generated task registry, which the next `cargo bake --regenerate` removed again.

## v0.2.5

- Depend on `base-markdown` under the `base-rs` organization, replacing `core-markdown`.

## v0.2.4

- Migrate to the source-backed `core-markdown` document AST, preserving supported Markdown behavior and source bytes.

## v0.2.3

- Use `core-markdown` in place of `socketry-markdown`.

## v0.2.2

- Expose the descriptive `UpdateSummary` result type while preserving `Update` as a compatibility alias.

## v0.2.1

- Resolve development task dependencies to the current checkout.

- Adopt `socketry-project` 0.3.7 for shared project tasks and Markdown normalization.

- Require the aggregate test and coverage result for pull request merges.

- Update the usage example and document shared agent context setup.

## v0.2.0

- Remove the redundant `bake_license::license` module. Use the root API and `license:update` task instead.

## v0.1.6

- Declare compatibility with the Bake 0.x API so task libraries can share one task registry when upgrading to crate-derived task namespaces.

## v0.1.5

- Use the shared `socketry-project` Releasing skill for the standard release process and remove references to the duplicate Bake Cargo publishing context.

## v0.1.4

- Preserve CRLF line endings when adding license headers to Rust source files.

## v0.1.3

- Preserve heading-like text inside HTML blocks when removing generated license sections.

## v0.1.2

- Create or update GitHub Releases after successful crates.io publication.
- Keep release versioning tasks compatible with the current local crate.

## v0.1.1

- Switch the runtime dependency from `socketry-bake` to `bake` 0.17.0.

- Add Bake Agent Context tasks to the project's development executable.

- Link the shared Rust context guidance from the README.

## v0.1.0

- Add the `license:update` task to refresh MIT license and Rust copyright headers.
