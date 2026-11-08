// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! License and copyright maintenance tasks for Bake.
//!
//! Copyright lines are derived from the repository's Git history, honouring `.mailmap` and `.git-blame-ignore-revs`, together with contributions recorded outside it in `.contributors.toml`.
mod authorship;
mod contributors;
mod source;
mod update;
mod update_summary;

#[cfg(test)]
mod test_support;

pub use update::{check, update};
pub use update_summary::UpdateSummary;
/// Compatibility name for [`UpdateSummary`].
pub use update_summary::UpdateSummary as Update;

/// Refresh `license.md`, the README License section, and Rust source headers, from Git history and `.contributors.toml`.
#[bake::task(name = "license:update")]
pub fn refresh(context: &mut bake::Context) -> bake::Result<String> {
    let summary = update(context.root())?;
    Ok(format!(
        "Changed {} file(s); refreshed {} tracked Rust source file(s)",
        summary.files_changed, summary.source_files,
    ))
}

/// Fail, naming each file, when `license:update` would change `license.md`, the README License section, or a Rust source header.
///
/// The release hook runs `license:update`, so this check finds a missing or stale header in the pull request that introduces it rather than in the release, where a new header line would move the line numbers in compile-fail snapshots such as trybuild `.stderr` files.
#[bake::task(name = "license:check")]
pub fn verify(context: &mut bake::Context) -> bake::Result<String> {
    let files = check(context.root())?;
    if files.is_empty() {
        return Ok("License file and Rust source headers are up to date".to_owned());
    }
    let files: Vec<_> = files
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    Err(bake::Error::new(format!(
        "{} file(s) need license updates: {}; run `cargo bake license:update`, commit the result, and update any compile-fail snapshots, such as trybuild `.stderr` files, whose line numbers moved",
        files.len(),
        files.join(", "),
    )))
}

#[cfg(test)]
mod task_tests {
    use crate::test_support::GitRepository;
    use bake::Registry;
    use tempfile::tempdir;

    #[test]
    fn reports_updated_license_and_source_file_counts() {
        let repository = GitRepository::new();
        repository.write("src/lib.rs", "pub fn example() {}\n");
        repository.commit(
            "Alice",
            "alice@example.com",
            "2024-01-02T03:04:05+00:00",
            "Add source",
        );

        let mut context = Registry::new().context(repository.root());
        let result = crate::refresh(&mut context).unwrap();

        assert_eq!(
            result,
            "Changed 2 file(s); refreshed 1 tracked Rust source file(s)"
        );
    }

    #[test]
    fn check_task_names_each_file_and_the_fix_until_updated() {
        let repository = GitRepository::new();
        repository.write("src/lib.rs", "pub fn example() {}\n");
        repository.write("tests/ui/fails.rs", "fn main() {}\n");
        repository.commit(
            "Alice",
            "alice@example.com",
            "2024-01-02T03:04:05+00:00",
            "Add sources",
        );

        let mut context = Registry::new().context(repository.root());
        let error = crate::verify(&mut context).unwrap_err().to_string();
        assert_eq!(
            error,
            "3 file(s) need license updates: license.md, src/lib.rs, tests/ui/fails.rs; run `cargo bake license:update`, commit the result, and update any compile-fail snapshots, such as trybuild `.stderr` files, whose line numbers moved"
        );
        assert!(!repository.root().join("license.md").exists());
        assert_eq!(
            std::fs::read_to_string(repository.root().join("src/lib.rs")).unwrap(),
            "pub fn example() {}\n"
        );

        crate::refresh(&mut context).unwrap();
        assert_eq!(
            crate::verify(&mut context).unwrap(),
            "License file and Rust source headers are up to date"
        );
    }

    #[test]
    fn propagates_errors_from_the_check_task() {
        let directory = tempdir().unwrap();
        let mut context = Registry::new().context(directory.path());

        assert!(crate::verify(&mut context).is_err());
    }

    #[test]
    fn propagates_update_errors_from_the_license_task() {
        let directory = tempdir().unwrap();
        let mut context = Registry::new().context(directory.path());

        assert!(crate::refresh(&mut context).is_err());
    }
}
