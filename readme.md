# `bake-license`

`bake-license` provides a Bake task for maintaining the MIT license file and copyright headers in Rust projects.

## Motivation

Updating license text and copyright headers by hand is repetitive. This task uses the repository's Git history to keep them consistent with its authorship.

## Usage

Add `bake-license` to the private `bake/` task package:

```sh
cargo bake --regenerate
cargo add --manifest-path bake/Cargo.toml bake-license
cargo bake --regenerate
```

Run the updater:

```sh
cargo bake license:update
```

The task updates `license.md` and tracked Rust source headers, honoring `.mailmap` and `.git-blame-ignore-revs`. It skips bot accounts ending in `[bot]`. Existing Rust documentation comments are retained. The header goes at the top of each file, after any byte order mark and shebang line and before inner attributes such as `#![doc = ...]` and `//!` documentation, where `rustfmt` leaves it unchanged. Commit new files before running the task if they should receive copyright headers. Every tracked Rust file receives a header, including test fixtures such as trybuild compile-fail cases under `tests/ui/`; regenerate their compiler-output snapshots, for trybuild with `TRYBUILD=overwrite cargo test`, after a header changes their line numbers.

Check that nothing needs updating, without writing any file:

```sh
cargo bake license:check
```

The check fails when `license:update` would change `license.md`, the README, or any Rust source header, naming each file and the command that fixes it. Run it in pull request CI, so a missing or stale header is fixed in the pull request that introduces it rather than by a later release. Copyright lines come from the Git history, so the check refuses a shallow clone; in GitHub Actions, check out the commits under review with `fetch-depth: 0`. The shared [Testing guide](https://github.com/base-rs/bake-test/blob/main/context/testing.md) gives the standard workflow job.

A pull request is checked against its own commits. A squash merge records one author at the merge date, so a pull request with commits by several authors, one merged in a later year, or one that changes a file also changed on the base branch since it branched can leave headers on the base branch that the next update changes. The check on pushes to the base branch reports them; fix them with `license:update` before releasing.

`license.md` is written as `base_markdown::normalize_markdown` writes it, so normalizing it afterwards, as the Base project release hook does, does not make the check fail.

### Contributions outside the Git history

Existing copyright lines are replaced with lines derived from authorship, so work that predates the repository, such as code ported from another project, needs recording. List it in `.contributors.toml` at the repository root, one `[[contribution]]` per contribution:

```toml
[[contribution]]
path = "src/timers.rs"
time = 2021-02-12T12:19:44+01:00
author = { name = "Wander Hillen", email = "wjw.hillen@gmail.com" }
```

Each contribution counts like a commit by its author in that year. One naming a `path` adds to that file's header; the path is relative to the repository root and must name a tracked file, so update it when the file is renamed. One without a `path` adds only to `license.md`, which counts every contribution. `time` is a TOML date or date-time. Names and emails are trimmed and may not contain control characters. Authors with an `email` are mapped through `.mailmap`, and bot authors are skipped, as for commits. Malformed entries and untracked paths fail the update, naming the contribution. The format follows the Ruby `bake-modernize` task's `.contributors.yaml`.

## Releasing

Prepare a release with `cargo bake cargo:release:patch` (or `minor`, `major`, or `bump --version X.Y.Z`) from a clean checkout of `main`; it bumps the version, commits with the `Release: vVERSION` trailer, validates the candidate, and opens the pull request. After review and merge, GitHub Actions publishes the release when the configured `crates-io` environment approves it. Follow the shared [Releasing skill](https://github.com/base-rs/base-project/blob/main/context/releasing.md) for the standard process.

## Releases

<!-- bake-readme:releases:start -->

See [releases.md](releases.md) for the full release history.

### v0.2.11

- Require `bake` 0.20 or later, instead of any 0.x version, so that Cargo resolves this crate to the same `bake` as a task package that requires `bake` 0.20.2. In a workspace whose `rust-version` is below the 1.88 that `bake` 0.20.2 declares, Cargo resolved the old requirement to `bake` 0.19.2, which declares none, and the task package failed to compile against two `bake` versions.

### v0.2.10

- Declare Rust 1.88 as the minimum supported version in the package's `rust-version`, so Cargo reports an older toolchain clearly and resolves dependencies that support it; `bake` 0.20's `bake-macros` uses let chains, which need Rust 1.88, one release above the Base default. The test workflow checks the package with that version.
- Write Rust license headers that `cargo fmt` leaves unchanged, so that a release's `license:update` no longer fails `cargo fmt --check`. The header now goes first, after any shebang and before inner attributes and `//!` documentation, since `rustfmt` removes the blank lines around a comment between inner attributes; an existing header after a leading `#![...]` attribute moves to the top on the next update. As `rustfmt` does, every updated source now ends at its last line, without trailing blank lines, and with a final line ending if it had none, so a file with nothing after its header no longer ends with a blank line, and a shebang on a source's last line, without a final line ending, is no longer followed directly by the header, without a blank line between them, until the next update. A `@generated` or "do not edit" comment after inner attributes now marks a source as generated when its license header comes before them, as it did when the header followed them.
- Read the leading inner attributes and comments with rustc's own lexer, from the `ra-ap-rustc_lexer` crate, instead of scanning their text, so that brackets, quotes and comment markers in strings, raw strings, char and byte literals, and comments, such as in `#![allow(dead_code)] // see [`, `#![a(c = '[')]` or `#![doc = r#"[ \"#]`, no longer make the update treat the rest of the source as part of the attribute, writing the new header at the end of the source, and another on every update, while keeping the old header and ignoring generated markers. The leading comments now end at an attribute that code follows on the same line, and at an attribute or block comment that rustc cannot lex, with an unterminated literal or comment or unbalanced delimiters, which is kept after the header as it was, with the rest of the source. As rustc reads them, a `////` comment is a plain comment, which can mark a source as generated, and only what Rust counts as whitespace is blank: a line holding only the left-to-right or right-to-left mark, U+200E or U+200F, is now blank, and may be removed with the old metadata around it, while a line holding a no-break space, an ideographic space, U+3000, or another Unicode space that is not whitespace in Rust is not blank: it ends the leading comments, and is kept.
- Keep a leading byte order mark first, with the header after it, instead of writing the header before it and keeping the old header after it. Recognize `#! [...]`, with whitespace, comments or line breaks before the `[`, as an inner attribute rather than a shebang, as rustc does, and recognize indented inner attributes. Keep the license-like documentation comments that make rustc read a `#!` before such a `[` as a shebang, which it then rejects, so that the next update still reads it as one. Remove an old header after an indented inner attribute or a leading block comment, such as `/*! ... */` documentation, instead of keeping it below the new one, and look for generated markers after them.
- Write the header after the frontmatter of a cargo script, the `---` fenced block that rustc requires to come first, after any shebang, instead of before the frontmatter, without a shebang, or between the shebang and the frontmatter, which rustc rejects, and replace an old header after the frontmatter. Frontmatter is kept first only when no whitespace comes before its opening fence on its line and a later line starts with at least as many dashes as that fence, closing it; other frontmatter is kept after the header, as before.

### v0.2.9

- Add `license:check`, which fails, naming each file and the fix, when `license:update` would change `license.md`, the README License section, or a Rust source header, without writing anything, so pull request CI can catch a missing or stale header before a release adds it. It refuses shallow clones, whose history gives incomplete copyright lines. The library exposes it as `check`.
- Write `license.md` as `base_markdown::normalize_markdown` writes it, one line per paragraph with backslash line breaks between copyright lines, so that normalizing it after an update, as the Base project release hook does, leaves it unchanged. Require base-markdown 0.1.3, which provides it.

<!-- bake-readme:releases:end -->

## See Also

- [Bake](https://github.com/base-rs/bake) — composable development tasks for Rust projects.

- [base-project](https://github.com/base-rs/base-project) — shared project tasks and conventions for Base Rust crates.

## Contributing

Please open an issue or pull request on [GitHub](https://github.com/base-rs/bake-license).

### Agent Context

Run `cargo bake agent:context:install` to install shared context and skills. Read `.agents/context/index.md` to find relevant guides, follow `agents.md` if present, and apply skills under `.agents/skills/`. The installer preserves repository-owned `agents.md`; it does not create or regenerate that file.

[Agent Context guide]: https://github.com/base-rs/bake-agent-context/blob/main/context/agent-context.md
