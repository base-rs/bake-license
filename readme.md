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

The task updates `license.md` and tracked Rust source headers, honoring `.mailmap` and `.git-blame-ignore-revs`. It skips bot accounts ending in `[bot]`. Existing Rust documentation comments are retained. Commit new files before running the task if they should receive copyright headers.

## Releasing

Prepare a release with `cargo bake cargo:version:patch` (or `minor`, `major`, or `bump --version X.Y.Z`), commit with a `Release: vVERSION` trailer, then run `cargo bake cargo:release` and open a pull request. After review and merge, GitHub Actions publishes the release when the configured `crates-io` environment approves it. Follow the shared [Releasing skill](https://github.com/core-rs/core-project/blob/main/context/releasing.md) for the standard process.

## Releases

<!-- bake-readme:releases:start -->

See [releases.md](releases.md) for the full release history.

### v0.2.3

- Use `core-markdown` in place of `socketry-markdown`.

### v0.2.2

- Expose the descriptive `UpdateSummary` result type while preserving `Update` as a compatibility alias.

### v0.2.1

- Resolve development task dependencies to the current checkout.

- Adopt `socketry-project` 0.3.7 for shared project tasks and Markdown normalization.

- Require the aggregate test and coverage result for pull request merges.

- Update the usage example and document shared agent context setup.

<!-- bake-readme:releases:end -->

## See Also

- [Bake](https://github.com/core-rs/bake) — composable development tasks for Rust projects.

- [core-project](https://github.com/core-rs/core-project) — shared project tasks and conventions for Core Rust crates.

## Contributing

Please open an issue or pull request on [GitHub](https://github.com/core-rs/bake-license).

### Agent Context

Run `cargo bake agent:context:install` to install shared context and skills. Read `.agents/context/index.md` to find relevant guides, follow `agents.md` if present, and apply skills under `.agents/skills/`. The installer preserves repository-owned `agents.md`; it does not create or regenerate that file.

[Agent Context guide]: https://github.com/core-rs/bake-agent-context/blob/main/context/agent-context.md
