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

Prepare a release with `cargo bake cargo:version:patch` (or `minor`, `major`, or `bump --version X.Y.Z`), commit with a `Release: vVERSION` trailer, then run `cargo bake cargo:release` and open a pull request. After review and merge, GitHub Actions publishes the release when the configured `crates-io` environment approves it. Follow the shared [Releasing skill](https://github.com/base-rs/base-project/blob/main/context/releasing.md) for the standard process.

## Releases

<!-- bake-readme:releases:start -->

See [releases.md](releases.md) for the full release history.

### v0.2.7

- Keep the blank line between leading documentation, such as module `//!` comments, and the code that follows when updating license headers, instead of removing it on every update.

### v0.2.6

- Leave generated Rust sources, whose leading comments say `@generated` or "do not edit", without license headers. Previously the update stamped a header onto Bake's generated task registry, which the next `cargo bake --regenerate` removed again.

### v0.2.5

- Depend on `base-markdown` under the `base-rs` organization, replacing `core-markdown`.

<!-- bake-readme:releases:end -->

## See Also

- [Bake](https://github.com/base-rs/bake) — composable development tasks for Rust projects.

- [base-project](https://github.com/base-rs/base-project) — shared project tasks and conventions for Base Rust crates.

## Contributing

Please open an issue or pull request on [GitHub](https://github.com/base-rs/bake-license).

### Agent Context

Run `cargo bake agent:context:install` to install shared context and skills. Read `.agents/context/index.md` to find relevant guides, follow `agents.md` if present, and apply skills under `.agents/skills/`. The installer preserves repository-owned `agents.md`; it does not create or regenerate that file.

[Agent Context guide]: https://github.com/base-rs/bake-agent-context/blob/main/context/agent-context.md
