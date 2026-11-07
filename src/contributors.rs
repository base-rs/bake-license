// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Contributions recorded outside the repository's Git history, from `.contributors.toml`.

use bake::{Error, Result};
use std::collections::BTreeSet;
use std::path::{Component, Path};
use std::process::Command;
use toml_edit::{DocumentMut, Item, Table, Value};

const FILE_NAME: &str = ".contributors.toml";

/// One contribution: an author's work in a year, on one path or, without a path, on the repository as a whole.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Contribution {
    path: Option<String>,
    author: String,
    year: u16,
}

impl Contribution {
    pub(crate) fn author(&self) -> &str {
        &self.author
    }

    pub(crate) fn year(&self) -> u16 {
        self.year
    }

    fn applies_to(&self, path: &Path) -> bool {
        self.path
            .as_deref()
            .is_some_and(|own| normalized(Path::new(own)) == normalized(path))
    }
}

/// The contributions a repository records outside its Git history. Empty when it has no `.contributors.toml`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Contributors(Vec<Contribution>);

impl Contributors {
    /// Load `.contributors.toml` from the repository root, mapping authors through the repository's `.mailmap` and skipping bots, as for Git authors.
    pub(crate) fn load(root: &Path) -> Result<Self> {
        Self::load_with_git(root, "git")
    }

    fn load_with_git(root: &Path, executable: &str) -> Result<Self> {
        let path = root.join(FILE_NAME);
        let contents = match std::fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(Error::new(format!("{FILE_NAME}: {error}"))),
        };
        let entries =
            parse(&contents).map_err(|error| Error::new(format!("{FILE_NAME}: {error}")))?;
        let names = mailmap_names(root, executable, &entries)?;
        Ok(Self(
            entries
                .into_iter()
                .zip(names)
                .filter(|(_, author)| !author.ends_with("[bot]"))
                .map(|(entry, author)| Contribution {
                    path: entry.path,
                    author,
                    year: entry.year,
                })
                .collect(),
        ))
    }

    /// Every contribution, which counts toward the repository's own copyright.
    pub(crate) fn repository(&self) -> impl Iterator<Item = &Contribution> {
        self.0.iter()
    }

    /// The contributions that name `path`, relative to the repository root.
    pub(crate) fn file<'a>(&'a self, path: &'a Path) -> impl Iterator<Item = &'a Contribution> {
        self.0
            .iter()
            .filter(move |contribution| contribution.applies_to(path))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Entry {
    path: Option<String>,
    name: String,
    email: Option<String>,
    year: u16,
}

fn parse(contents: &str) -> std::result::Result<Vec<Entry>, String> {
    let document = contents
        .parse::<DocumentMut>()
        .map_err(|error| error.to_string())?;
    let table = document.as_table();
    if let Some(key) = table
        .iter()
        .map(|(key, _)| key)
        .find(|key| *key != "contribution")
    {
        return Err(format!(
            "unknown key `{key}`; expected `[[contribution]]` entries"
        ));
    }
    let Some(item) = table.get("contribution") else {
        return Ok(Vec::new());
    };
    let Some(contributions) = item.as_array_of_tables() else {
        return Err(
            "`contribution` must be an array of tables, written `[[contribution]]`".to_owned(),
        );
    };
    contributions
        .iter()
        .enumerate()
        .map(|(index, table)| {
            entry(table).map_err(|error| format!("contribution {}: {error}", index + 1))
        })
        .collect()
}

fn entry(table: &Table) -> std::result::Result<Entry, String> {
    if let Some(key) = table
        .iter()
        .map(|(key, _)| key)
        .find(|key| !matches!(*key, "path" | "time" | "author"))
    {
        return Err(format!("unknown key `{key}`"));
    }
    let path = optional_string(table.get("path"), "path")?;
    let year = match table.get("time").and_then(Item::as_value) {
        Some(Value::Datetime(time)) => time
            .value()
            .date
            .map(|date| date.year)
            .ok_or_else(|| "`time` must include a date".to_owned())?,
        Some(_) => return Err("`time` must be a TOML date or date-time".to_owned()),
        None => return Err("missing `time`".to_owned()),
    };
    let author = match table.get("author") {
        Some(Item::Table(author)) => author
            .iter()
            .map(|(key, item)| (key, item.as_value()))
            .collect::<Vec<_>>(),
        Some(Item::Value(Value::InlineTable(author))) => author
            .iter()
            .map(|(key, value)| (key, Some(value)))
            .collect(),
        Some(_) => return Err("`author` must be a table".to_owned()),
        None => return Err("missing `author`".to_owned()),
    };
    if let Some((key, _)) = author
        .iter()
        .find(|(key, _)| !matches!(*key, "name" | "email"))
    {
        return Err(format!("unknown author key `{key}`"));
    }
    let field = |name: &str| {
        author
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| *value)
    };
    let name = match field("name") {
        Some(Some(Value::String(name))) if !name.value().trim().is_empty() => name.value().clone(),
        Some(_) => return Err("`author.name` must be a nonempty string".to_owned()),
        None => return Err("missing `author.name`".to_owned()),
    };
    let email = match field("email") {
        Some(Some(Value::String(email))) => Some(email.value().clone()),
        Some(_) => return Err("`author.email` must be a string".to_owned()),
        None => None,
    };
    Ok(Entry {
        path,
        name,
        email,
        year,
    })
}

fn optional_string(item: Option<&Item>, key: &str) -> std::result::Result<Option<String>, String> {
    match item.map(Item::as_value) {
        None => Ok(None),
        Some(Some(Value::String(value))) => Ok(Some(value.value().clone())),
        Some(_) => Err(format!("`{key}` must be a string")),
    }
}

// Map each entry's author through the repository's mailmap with `git check-mailmap`, which applies Git's own rules, as `%aN` does for commits. Entries without an email keep their name.
fn mailmap_names(root: &Path, executable: &str, entries: &[Entry]) -> Result<Vec<String>> {
    let contacts: BTreeSet<String> = entries
        .iter()
        .filter_map(|entry| {
            entry
                .email
                .as_ref()
                .map(|email| contact(&entry.name, email))
        })
        .collect();
    let mut mapped = std::collections::BTreeMap::new();
    if !contacts.is_empty() {
        let output = Command::new(executable)
            .current_dir(root)
            .arg("check-mailmap")
            .args(&contacts)
            .output()?;
        if !output.status.success() {
            return Err(Error::new(format!(
                "git check-mailmap failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let output = String::from_utf8_lossy(&output.stdout);
        for (contact, line) in contacts.iter().zip(output.lines()) {
            let name = line.rsplit_once(" <").map_or(line, |(name, _)| name);
            mapped.insert(contact.clone(), name.to_owned());
        }
    }
    Ok(entries
        .iter()
        .map(|entry| {
            entry
                .email
                .as_ref()
                .and_then(|email| mapped.get(&contact(&entry.name, email)))
                .cloned()
                .unwrap_or_else(|| entry.name.clone())
        })
        .collect())
}

fn contact(name: &str, email: &str) -> String {
    format!("{name} <{email}>")
}

fn normalized(path: &Path) -> Vec<Component<'_>> {
    path.components()
        .filter(|component| !matches!(component, Component::CurDir))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::GitRepository;

    #[test]
    fn a_repository_without_a_contributors_file_has_no_contributions() {
        let repository = GitRepository::new();
        assert_eq!(
            Contributors::load(repository.root()).unwrap(),
            Contributors::default()
        );
    }

    #[test]
    fn loads_contributions_with_paths_dates_and_mailmapped_authors_and_skips_bots() {
        let repository = GitRepository::new();
        repository.write(
            ".mailmap",
            "Alice <alice@example.com> <alice-old@example.com>\n",
        );
        repository.write(
            FILE_NAME,
            r#"
[[contribution]]
path = "src/heap.rs"
time = 2021-02-12T12:19:44+01:00
author = { name = "Alice Old", email = "alice-old@example.com" }

[[contribution]]
time = 2019-06-01
[contribution.author]
name = "Bob"

[[contribution]]
path = "./src/heap.rs"
time = 2020-01-01T00:00:00
author = { name = "builder[bot]", email = "bot@example.com" }
"#,
        );

        let contributors = Contributors::load(repository.root()).unwrap();
        let all: Vec<_> = contributors
            .repository()
            .map(|c| (c.author(), c.year()))
            .collect();
        assert_eq!(all, [("Alice", 2021), ("Bob", 2019)]);
        let heap: Vec<_> = contributors
            .file(Path::new("src/heap.rs"))
            .map(|c| (c.author(), c.year()))
            .collect();
        assert_eq!(heap, [("Alice", 2021)]);
        assert_eq!(contributors.file(Path::new("src/other.rs")).count(), 0);
    }

    #[test]
    fn contributions_without_emails_keep_their_names_without_consulting_the_mailmap() {
        let repository = GitRepository::new();
        repository.write(
            FILE_NAME,
            "[[contribution]]\ntime = 2020-01-01\nauthor = { name = \"Erin\" }\n",
        );
        // No contact needs mapping, so Git is not run; a missing executable proves it.
        let contributors =
            Contributors::load_with_git(repository.root(), "missing-git-for-test").unwrap();
        let all: Vec<_> = contributors
            .repository()
            .map(|c| (c.author(), c.year()))
            .collect();
        assert_eq!(all, [("Erin", 2020)]);
    }

    #[test]
    fn matches_paths_regardless_of_a_leading_current_directory() {
        let contribution = Contribution {
            path: Some("./src/lib.rs".to_owned()),
            author: "Alice".to_owned(),
            year: 2020,
        };
        assert!(contribution.applies_to(Path::new("src/lib.rs")));
        assert!(!contribution.applies_to(Path::new("src")));
        let repository_wide = Contribution {
            path: None,
            ..contribution
        };
        assert!(!repository_wide.applies_to(Path::new("src/lib.rs")));
    }

    #[test]
    fn rejects_malformed_contributions_with_their_position() {
        let cases = [
            ("not toml = [", "expected"),
            ("other = 1\n", "unknown key `other`"),
            ("contribution = 1\n", "array of tables"),
            (
                "[[contribution]]\ntime = 2020-01-01\nauthor = { name = \"A\" }\nextra = 1\n",
                "contribution 1: unknown key `extra`",
            ),
            (
                "[[contribution]]\nauthor = { name = \"A\" }\n",
                "missing `time`",
            ),
            (
                "[[contribution]]\ntime = \"2020\"\nauthor = { name = \"A\" }\n",
                "TOML date or date-time",
            ),
            (
                "[[contribution]]\ntime = 12:00:00\nauthor = { name = \"A\" }\n",
                "must include a date",
            ),
            ("[[contribution]]\ntime = 2020-01-01\n", "missing `author`"),
            (
                "[[contribution]]\ntime = 2020-01-01\nauthor = \"A\"\n",
                "`author` must be a table",
            ),
            (
                "[[contribution]]\ntime = 2020-01-01\nauthor = { email = \"a@example.com\" }\n",
                "missing `author.name`",
            ),
            (
                "[[contribution]]\ntime = 2020-01-01\nauthor = { name = \" \" }\n",
                "nonempty string",
            ),
            (
                "[[contribution]]\ntime = 2020-01-01\nauthor = { name = \"A\", email = 1 }\n",
                "`author.email` must be a string",
            ),
            (
                "[[contribution]]\ntime = 2020-01-01\nauthor = { name = \"A\", login = \"a\" }\n",
                "unknown author key `login`",
            ),
            (
                "[[contribution]]\npath = 1\ntime = 2020-01-01\nauthor = { name = \"A\" }\n",
                "`path` must be a string",
            ),
            (
                "[[contribution]]\ntime = 2020-01-01\nauthor = { name = \"A\" }\n[[contribution]]\nauthor = { name = \"B\" }\n",
                "contribution 2: missing `time`",
            ),
        ];
        for (contents, expected) in cases {
            let error = parse(contents).unwrap_err();
            assert!(error.contains(expected), "{contents:?}: {error}");
        }
        assert_eq!(parse("").unwrap(), Vec::new());
    }

    #[test]
    fn reports_unreadable_files_and_mailmap_failures() {
        let repository = GitRepository::new();
        std::fs::create_dir(repository.root().join(FILE_NAME)).unwrap();
        assert!(
            Contributors::load(repository.root())
                .unwrap_err()
                .to_string()
                .contains(FILE_NAME)
        );

        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join(FILE_NAME),
            "[[contribution]]\ntime = 2020-01-01\nauthor = { name = \"A\", email = \"a@example.com\" }\n",
        )
        .unwrap();
        assert!(Contributors::load_with_git(directory.path(), "missing-git-for-test").is_err());
        assert!(
            Contributors::load(directory.path())
                .unwrap_err()
                .to_string()
                .contains("git check-mailmap failed")
        );
        let invalid = GitRepository::new();
        invalid.write(FILE_NAME, "not toml = [");
        assert!(
            Contributors::load(invalid.root())
                .unwrap_err()
                .to_string()
                .starts_with(FILE_NAME)
        );
    }
}
