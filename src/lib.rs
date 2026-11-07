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

pub use update::update;
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
    fn propagates_update_errors_from_the_license_task() {
        let directory = tempdir().unwrap();
        let mut context = Registry::new().context(directory.path());

        assert!(crate::refresh(&mut context).is_err());
    }
}
