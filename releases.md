# Releases

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
