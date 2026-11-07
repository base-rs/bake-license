// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

pub struct GitRepository {
    directory: TempDir,
}

impl GitRepository {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let output = Command::new("git")
            .current_dir(directory.path())
            .args(["init", "--quiet"])
            .output()
            .unwrap();
        assert_success(&output);

        Self { directory }
    }

    pub fn root(&self) -> &Path {
        self.directory.path()
    }

    pub fn write(&self, path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self.root().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        path
    }

    pub fn git(&self, arguments: &[&str]) -> Output {
        Command::new("git")
            .current_dir(self.root())
            .args(arguments)
            .output()
            .unwrap()
    }

    pub fn commit(&self, author: &str, email: &str, date: &str, message: &str) -> String {
        let output = self.git(&["add", "--all"]);
        assert_success(&output);

        let author = format!("user.name={author}");
        let email = format!("user.email={email}");
        let output = Command::new("git")
            .current_dir(self.root())
            .args([
                "-c",
                &author,
                "-c",
                &email,
                // Fixture commits must not depend on the user's signing configuration.
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                message,
            ])
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .output()
            .unwrap();
        assert_success(&output);

        let output = self.git(&["rev-parse", "HEAD"]);
        assert_success(&output);
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }
}

pub fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "command failed:")]
    fn reports_failed_command_output() {
        let repository = GitRepository::new();
        assert_success(&repository.git(&["not-a-git-command"]));
    }

    #[test]
    fn creates_a_repository_and_commits_files_with_an_explicit_author_date() {
        let repository = GitRepository::new();
        repository.write("src/lib.rs", "pub fn fixture() {}\n");

        let revision = repository.commit(
            "Test Author",
            "test@example.com",
            "2020-01-02T03:04:05+00:00",
            "Add fixture",
        );

        assert_eq!(revision.len(), 40);
        assert_eq!(
            fs::read_to_string(repository.root().join("src/lib.rs")).unwrap(),
            "pub fn fixture() {}\n"
        );
    }
}
