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

The task updates `license.md` and tracked Rust source headers, honoring `.mailmap` and `.git-blame-ignore-revs`. It skips bot accounts ending in `[bot]`. Existing Rust documentation comments are retained. Commit new files before running the task if they should receive copyright headers. Every tracked Rust file receives a header, including test fixtures such as trybuild compile-fail cases under `tests/ui/`; regenerate their compiler-output snapshots, for trybuild with `TRYBUILD=overwrite cargo test`, after a header changes their line numbers.

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

### v0.2.9

- Add `license:check`, which fails, naming each file and the fix, when `license:update` would change `license.md`, the README License section, or a Rust source header, without writing anything, so pull request CI can catch a missing or stale header before a release adds it. It refuses shallow clones, whose history gives incomplete copyright lines. The library exposes it as `check`.
- Write `license.md` as `base_markdown::normalize_markdown` writes it, one line per paragraph with backslash line breaks between copyright lines, so that normalizing it after an update, as the Base project release hook does, leaves it unchanged. Require base-markdown 0.1.3, which provides it.

### v0.2.8

- Read contributions made outside the repository's Git history from `.contributors.toml`, so that copyright lines for work such as code ported from another project survive `license:update`. Each `[[contribution]]` gives an `author` with `name` and optional `email`, a `time`, and an optional `path`; it counts like a commit to that file, which must be tracked, or, without a path, to the repository's `license.md`. Authors are mapped through `.mailmap`, and bots are skipped. Malformed entries and untracked paths fail the update with their position.
- Disable commit signing in the test fixtures' commits, so the tests no longer depend on the user's Git signing configuration.

### v0.2.7

- Keep the blank line between leading documentation, such as module `//!` comments, and the code that follows when updating license headers, instead of removing it on every update.

<!-- bake-readme:releases:end -->

## See Also

- [Bake](https://github.com/base-rs/bake) — composable development tasks for Rust projects.

- [base-project](https://github.com/base-rs/base-project) — shared project tasks and conventions for Base Rust crates.

## Contributing

Please open an issue or pull request on [GitHub](https://github.com/base-rs/bake-license).

### Agent Context

Run `cargo bake agent:context:install` to install shared context and skills. Read `.agents/context/index.md` to find relevant guides, follow `agents.md` if present, and apply skills under `.agents/skills/`. The installer preserves repository-owned `agents.md`; it does not create or regenerate that file.

[Agent Context guide]: https://github.com/base-rs/bake-agent-context/blob/main/context/agent-context.md
