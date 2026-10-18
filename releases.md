# Releases

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
