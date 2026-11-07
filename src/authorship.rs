// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::contributors::{Contribution, Contributors};
use bake::{Error, Result};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct Copyright {
    first_year: u16,
    last_year: u16,
    author: String,
}

impl Copyright {
    pub(crate) fn new(first_year: u16, last_year: u16, author: impl Into<String>) -> Self {
        Self {
            first_year,
            last_year,
            author: author.into(),
        }
    }

    pub(crate) fn statement(&self) -> String {
        let years = if self.first_year == self.last_year {
            self.first_year.to_string()
        } else {
            format!("{}-{}", self.first_year, self.last_year)
        };

        let punctuation = if self.author.ends_with('.') { "" } else { "." };
        format!("Copyright, {years}, by {}{punctuation}", self.author)
    }

    pub(crate) fn source_comment(&self) -> String {
        format!("// {}", self.statement())
    }
}

/// The repository's copyright: its whole Git history and every recorded contribution.
pub(crate) fn repository(root: &Path, contributors: &Contributors) -> Result<Vec<Copyright>> {
    collect(root, None, contributors.repository())
}

/// A file's copyright: its Git history, following renames, and the recorded contributions naming it.
pub(crate) fn file(
    root: &Path,
    path: &Path,
    contributors: &Contributors,
) -> Result<Vec<Copyright>> {
    collect(root, Some(path), contributors.file(path))
}

pub(crate) fn rust_files(root: &Path) -> Result<Vec<String>> {
    rust_files_with_git(root, "git")
}

fn rust_files_with_git(root: &Path, executable: &str) -> Result<Vec<String>> {
    let output = git_command(root, executable)
        .args(["ls-files", "--cached", "-z", "--", "*.rs"])
        .output()?;
    ensure_success(&output.status, &output.stderr, "git ls-files")?;

    parse_rust_paths(&output.stdout)
}

fn parse_rust_paths(output: &[u8]) -> Result<Vec<String>> {
    let mut paths = Vec::new();
    for path in output.split(|byte| *byte == 0) {
        if path.is_empty() {
            continue;
        }
        let path = std::str::from_utf8(path)
            .map_err(|error| Error::new(format!("tracked Rust path is not UTF-8: {error}")))?;
        if path
            .split('/')
            .any(|part| matches!(part, ".git" | "target" | "vendor"))
        {
            continue;
        }
        paths.push(path.to_owned());
    }

    Ok(paths)
}

fn collect<'a>(
    root: &Path,
    path: Option<&Path>,
    contributions: impl IntoIterator<Item = &'a Contribution>,
) -> Result<Vec<Copyright>> {
    let mut years = collect_with_git(root, path, "git")?;
    for contribution in contributions {
        years.add(contribution.author(), contribution.year());
    }
    Ok(years.into_copyrights())
}

fn collect_with_git(root: &Path, path: Option<&Path>, executable: &str) -> Result<Years> {
    let ignored = ignored_revisions(root)?;
    let mut command = git_command(root, executable);
    command.args(["log", "--format=%H%x00%aN%x00%aI"]);
    if path.is_some() {
        command.arg("--follow");
    }
    command
        .arg("--")
        .arg(path.unwrap_or_else(|| Path::new(".")));

    let output = command.output()?;
    ensure_success(&output.status, &output.stderr, "git log")?;

    Ok(parse_git_log(&output.stdout, &ignored))
}

/// Each author's first and last year of contribution.
#[derive(Default)]
struct Years(BTreeMap<String, (u16, u16)>);

impl Years {
    fn add(&mut self, author: &str, year: u16) {
        self.0
            .entry(author.to_owned())
            .and_modify(|(first_year, last_year)| {
                *first_year = (*first_year).min(year);
                *last_year = (*last_year).max(year);
            })
            .or_insert((year, year));
    }

    fn into_copyrights(self) -> Vec<Copyright> {
        let mut copyrights: Vec<_> = self
            .0
            .into_iter()
            .map(|(author, (first_year, last_year))| Copyright::new(first_year, last_year, author))
            .collect();
        copyrights.sort();
        copyrights
    }
}

fn parse_git_log(output: &[u8], ignored: &HashSet<String>) -> Years {
    let output = String::from_utf8_lossy(output);
    let mut years = Years::default();

    for record in output.lines().filter(|line| !line.is_empty()) {
        let mut fields = record.split('\0');
        let revision = fields.next().unwrap_or_default();
        let Some(author) = fields.next() else {
            continue;
        };
        let Some(date) = fields.next() else {
            continue;
        };

        if ignored
            .iter()
            .any(|ignored_revision| revision.starts_with(ignored_revision))
            || author.ends_with("[bot]")
            || author.is_empty()
        {
            continue;
        }

        let Some(year) = date.get(..4).and_then(|year| year.parse::<u16>().ok()) else {
            continue;
        };

        years.add(author, year);
    }

    years
}

fn ignored_revisions(root: &Path) -> Result<HashSet<String>> {
    let path = root.join(".git-blame-ignore-revs");
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(HashSet::new());
        }
        Err(error) => return Err(Error::from(error)),
    };

    Ok(contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect())
}

fn git_command(root: &Path, executable: &str) -> Command {
    let mut command = Command::new(executable);
    command.current_dir(root);
    command
}

fn ensure_success(status: &std::process::ExitStatus, stderr: &[u8], operation: &str) -> Result<()> {
    if status.success() {
        return Ok(());
    }

    Err(Error::new(format!(
        "{operation} failed: {}",
        String::from_utf8_lossy(stderr).trim()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{GitRepository, assert_success};
    use std::fs;

    fn commit(repository: &GitRepository, author: &str, email: &str, date: &str) -> String {
        repository.commit(author, email, date, "Update source")
    }

    #[test]
    fn formats_single_year_ranges_and_source_comments() {
        let single = Copyright::new(2020, 2020, "Alice");
        let range = Copyright::new(2019, 2024, "Bob.");

        assert_eq!(single.statement(), "Copyright, 2020, by Alice.");
        assert_eq!(single.source_comment(), "// Copyright, 2020, by Alice.");
        assert_eq!(range.statement(), "Copyright, 2019-2024, by Bob.");
    }

    #[test]
    fn collects_mailmapped_authors_and_ignores_bots_and_listed_revisions() {
        let repository = GitRepository::new();
        repository.write(
            ".mailmap",
            "Alice <alice@example.com> <alice-old@example.com>\n",
        );
        repository.write("src/lib.rs", "pub fn first() {}\n");
        commit(
            &repository,
            "Alice Old",
            "alice-old@example.com",
            "2018-01-02T03:04:05+00:00",
        );

        repository.write("src/lib.rs", "pub fn ignored() {}\n");
        let ignored = commit(
            &repository,
            "Bob",
            "bob@example.com",
            "2020-02-03T04:05:06+00:00",
        );
        repository.write(
            ".git-blame-ignore-revs",
            format!("# ignored\n\n{}\n", &ignored[..8]),
        );

        repository.write("src/lib.rs", "pub fn last() {}\n");
        commit(
            &repository,
            "Alice Old",
            "alice-old@example.com",
            "2023-03-04T05:06:07+00:00",
        );
        repository.write("src/generated.rs", "pub fn generated() {}\n");
        commit(
            &repository,
            "build[bot]",
            "bot@example.com",
            "2024-04-05T06:07:08+00:00",
        );

        let copyrights = super::repository(repository.root(), &Contributors::default()).unwrap();
        assert_eq!(copyrights.len(), 1);
        assert_eq!(copyrights[0].statement(), "Copyright, 2018-2023, by Alice.");
    }

    #[test]
    fn collects_only_a_files_history_and_follows_renames() {
        let repository = GitRepository::new();
        repository.write("src/old.rs", "pub fn original() {}\n");
        commit(
            &repository,
            "Alice",
            "alice@example.com",
            "2019-01-02T03:04:05+00:00",
        );

        assert_success(&repository.git(&["mv", "src/old.rs", "src/new.rs"]));
        commit(
            &repository,
            "Bob",
            "bob@example.com",
            "2022-02-03T04:05:06+00:00",
        );
        repository.write("src/other.rs", "pub fn unrelated() {}\n");
        commit(
            &repository,
            "Carol",
            "carol@example.com",
            "2024-03-04T05:06:07+00:00",
        );

        let copyrights = file(
            repository.root(),
            Path::new("src/new.rs"),
            &Contributors::default(),
        )
        .unwrap();
        assert_eq!(
            copyrights
                .iter()
                .map(Copyright::statement)
                .collect::<Vec<_>>(),
            ["Copyright, 2019, by Alice.", "Copyright, 2022, by Bob.",]
        );
    }

    #[test]
    fn lists_tracked_rust_files_without_vendor_or_target_paths() {
        let repository = GitRepository::new();
        repository.write("src/lib.rs", "pub fn library() {}\n");
        repository.write("tests/integration.rs", "#[test] fn test() {}\n");
        repository.write("vendor/dependency/src/lib.rs", "pub fn vendor() {}\n");
        repository.write("target/debug/build/generated.rs", "pub fn target() {}\n");
        repository.commit(
            "Alice",
            "alice@example.com",
            "2020-01-02T03:04:05+00:00",
            "Add tracked Rust files",
        );

        assert_eq!(
            rust_files(repository.root()).unwrap(),
            ["src/lib.rs", "tests/integration.rs"]
        );
    }

    #[test]
    fn rejects_tracked_rust_paths_that_are_not_utf8() {
        assert!(
            parse_rust_paths(b"src/non-utf8-\xff.rs\0")
                .unwrap_err()
                .to_string()
                .contains("tracked Rust path is not UTF-8")
        );

        assert_eq!(parse_rust_paths(b"\0src/lib.rs\0").unwrap(), ["src/lib.rs"]);
    }

    #[test]
    fn reports_git_and_ignore_file_failures() {
        let directory = tempfile::tempdir().unwrap();
        assert!(repository(directory.path(), &Contributors::default()).is_err());
        assert!(rust_files(directory.path()).is_err());
        assert!(rust_files_with_git(directory.path(), "missing-git-for-test").is_err());
        assert!(collect_with_git(directory.path(), None, "missing-git-for-test").is_err());

        let repository = GitRepository::new();
        repository.write("src/lib.rs", "pub fn fixture() {}\n");
        repository.commit(
            "Alice",
            "alice@example.com",
            "2020-01-02T03:04:05+00:00",
            "Add fixture",
        );
        fs::create_dir(repository.root().join(".git-blame-ignore-revs")).unwrap();

        assert!(super::repository(repository.root(), &Contributors::default()).is_err());
    }

    #[test]
    fn ignores_malformed_history_records_and_invalid_years() {
        let output = concat!(
            "short record\n",
            "missing-date\0Alice\n",
            "empty-author\0\02020-01-01T00:00:00+00:00\n",
            "bad-year\0Bob\0year-01-01T00:00:00+00:00\n",
            "ignored-revision\0Carol\02020-01-01T00:00:00+00:00\n",
            "bot-revision\0builder[bot]\02021-01-01T00:00:00+00:00\n",
            "accepted-revision\0Dana\02022-01-01T00:00:00+00:00\n",
        );
        let ignored = HashSet::from(["ignored".to_owned()]);

        let copyrights = parse_git_log(output.as_bytes(), &ignored).into_copyrights();

        assert_eq!(copyrights.len(), 1);
        assert_eq!(copyrights[0].statement(), "Copyright, 2022, by Dana.");
    }

    #[test]
    fn merges_recorded_contributions_with_a_files_history() {
        let repository = GitRepository::new();
        repository.write("src/heap.rs", "pub fn heap() {}\n");
        commit(
            &repository,
            "Samuel",
            "samuel@example.com",
            "2026-01-02T03:04:05+00:00",
        );
        repository.write("src/other.rs", "pub fn other() {}\n");
        commit(
            &repository,
            "Carol",
            "carol@example.com",
            "2026-02-03T04:05:06+00:00",
        );
        repository.write(
            ".contributors.toml",
            concat!(
                "[[contribution]]\npath = \"src/heap.rs\"\ntime = 2021-02-12T12:19:44+01:00\n",
                "author = { name = \"Wander\", email = \"wander@example.com\" }\n\n",
                "[[contribution]]\npath = \"src/heap.rs\"\ntime = 2021-08-01\n",
                "author = { name = \"Samuel\", email = \"samuel@example.com\" }\n\n",
                "[[contribution]]\ntime = 2018-05-06\nauthor = { name = \"Dana\" }\n",
            ),
        );
        let contributors = Contributors::load(repository.root()).unwrap();

        let statements = |copyrights: Vec<Copyright>| {
            copyrights
                .iter()
                .map(Copyright::statement)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            statements(file(repository.root(), Path::new("src/heap.rs"), &contributors).unwrap()),
            [
                "Copyright, 2021, by Wander.",
                "Copyright, 2021-2026, by Samuel."
            ]
        );
        assert_eq!(
            statements(file(repository.root(), Path::new("src/other.rs"), &contributors).unwrap()),
            ["Copyright, 2026, by Carol."]
        );
        assert_eq!(
            statements(super::repository(repository.root(), &contributors).unwrap()),
            [
                "Copyright, 2018, by Dana.",
                "Copyright, 2021, by Wander.",
                "Copyright, 2021-2026, by Samuel.",
                "Copyright, 2026, by Carol.",
            ]
        );
    }
}
