// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::authorship::{self, Copyright};
use crate::source;
use bake::{Error, Result};
use base_markdown::{
    Document, ParseOptions,
    stream::{EventKind, Tag},
};
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

const MIT_LICENSE: &str = r#"Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE."#;

use crate::UpdateSummary;

pub fn update(root: &Path) -> Result<UpdateSummary> {
    update_with(root, &mut FileOperations)
}

trait UpdateOperations {
    fn repository(&mut self, root: &Path) -> Result<Vec<Copyright>> {
        authorship::repository(root)
    }

    fn rust_files(&mut self, root: &Path) -> Result<Vec<String>> {
        authorship::rust_files(root)
    }

    fn file(&mut self, root: &Path, path: &Path) -> Result<Vec<Copyright>> {
        authorship::file(root, path)
    }

    fn read_file(&mut self, path: &Path) -> Result<String> {
        Ok(fs::read_to_string(path)?)
    }

    fn replace_if_changed(&mut self, path: &Path, contents: &str) -> Result<bool> {
        replace_if_changed(path, contents)
    }
}

struct FileOperations;

impl UpdateOperations for FileOperations {}

fn update_with(root: &Path, operations: &mut impl UpdateOperations) -> Result<UpdateSummary> {
    let repository_copyrights = operations.repository(root)?;
    let mut summary = UpdateSummary::default();

    let license_path = root.join("license.md");
    let license = license_document(&repository_copyrights);
    if operations.replace_if_changed(&license_path, &license)? {
        summary.files_changed += 1;
    }

    let readme_path = root.join("readme.md");
    if readme_path.is_file() {
        let readme = operations.read_file(&readme_path)?;
        let updated = remove_license_section(&readme);
        if operations.replace_if_changed(&readme_path, &updated)? {
            summary.files_changed += 1;
        }
    }

    for relative_path in operations.rust_files(root)? {
        let path = root.join(&relative_path);
        if !path.is_file() {
            continue;
        }

        let copyrights = operations.file(root, Path::new(&relative_path))?;
        if copyrights.is_empty() {
            continue;
        }

        let contents = operations
            .read_file(&path)
            .map_err(|error| Error::new(format!("{}: {error}", path.display())))?;
        let updated = source::update_source(&contents, &copyrights);
        if operations.replace_if_changed(&path, &updated)? {
            summary.files_changed += 1;
        }
        summary.source_files += 1;
    }

    Ok(summary)
}

fn license_document(copyrights: &[Copyright]) -> String {
    let mut output = String::from("# MIT License\n\n");
    for copyright in copyrights {
        output.push_str(&copyright.statement());
        output.push_str("  \n");
    }
    if !copyrights.is_empty() {
        output.push('\n');
    }
    output.push_str(MIT_LICENSE);
    output.push('\n');
    output
}

fn remove_license_section(source: &str) -> String {
    let document = Document::parse(source, &ParseOptions::default())
        .expect("the built-in Markdown parser produces a valid document");
    let mut children = document.children(document.root());
    let mut output = String::with_capacity(source.len());
    let mut copied_until = 0;
    while let Some(id) = children.next() {
        let node = document.node(id).unwrap();
        let EventKind::Start(Tag::Heading(level)) = node.kind else {
            continue;
        };
        let text = document.text_content(id);
        let title = text
            .strip_suffix("\r\n")
            .or_else(|| text.strip_suffix(['\r', '\n']))
            .unwrap_or(&text);
        if !title.eq_ignore_ascii_case("license") || node.span.start < copied_until {
            continue;
        }
        let end = children
            .clone()
            .find_map(|next| {
                let node = document.node(next).unwrap();
                match node.kind {
                    EventKind::Start(Tag::Heading(next_level)) if next_level <= level => {
                        Some(node.span.start)
                    }
                    _ => None,
                }
            })
            .unwrap_or(source.len());
        output.push_str(&source[copied_until..node.span.start]);
        copied_until = end;
    }
    output.push_str(&source[copied_until..]);
    output
}

fn replace_if_changed(path: &Path, contents: &str) -> Result<bool> {
    replace_if_changed_using(path, contents, write_temporary_file)
}

fn replace_if_changed_using(
    path: &Path,
    contents: &str,
    replace: impl FnOnce(NamedTempFile, &Path, &str) -> Result<()>,
) -> Result<bool> {
    let target_path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
    match fs::read_to_string(&target_path) {
        Ok(existing) if existing == contents => return Ok(false),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::from(error)),
    }

    let parent = target_path
        .parent()
        .ok_or_else(|| Error::new("updated file has no parent directory"))?;
    let temporary = NamedTempFile::new_in(parent)?;
    replace(temporary, &target_path, contents)?;
    Ok(true)
}

trait TemporaryReplacementFile: Write + Sized {
    fn set_permissions(&self, permissions: fs::Permissions) -> std::io::Result<()>;
    fn sync_all(&self) -> std::io::Result<()>;
    fn persist(self, path: &Path) -> std::io::Result<()>;
}

impl TemporaryReplacementFile for NamedTempFile {
    fn set_permissions(&self, permissions: fs::Permissions) -> std::io::Result<()> {
        self.as_file().set_permissions(permissions)
    }

    fn sync_all(&self) -> std::io::Result<()> {
        self.as_file().sync_all()
    }

    fn persist(self, path: &Path) -> std::io::Result<()> {
        NamedTempFile::persist(self, path)
            .map(|_| ())
            .map_err(|error| error.error)
    }
}

fn write_temporary_file<T: TemporaryReplacementFile>(
    mut temporary: T,
    path: &Path,
    contents: &str,
) -> Result<()> {
    temporary.write_all(contents.as_bytes())?;
    if let Ok(metadata) = fs::metadata(path) {
        temporary.set_permissions(metadata.permissions())?;
    }
    temporary.sync_all()?;
    temporary.persist(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authorship::Copyright;
    use crate::test_support::GitRepository;
    use std::fs;
    use tempfile::tempdir;

    struct FailingOperations {
        failure: &'static str,
    }

    impl FailingOperations {
        fn error(&self, operation: &str) -> Result<()> {
            if self.failure == operation {
                Err(Error::new(format!("injected {operation} failure")))
            } else {
                Ok(())
            }
        }
    }

    impl UpdateOperations for FailingOperations {
        fn repository(&mut self, _root: &Path) -> Result<Vec<Copyright>> {
            self.error("repository")?;
            Ok(Vec::new())
        }

        fn rust_files(&mut self, _root: &Path) -> Result<Vec<String>> {
            self.error("rust-files")?;
            if matches!(self.failure, "file" | "replace-source") {
                Ok(vec!["src/lib.rs".to_owned()])
            } else {
                Ok(Vec::new())
            }
        }

        fn file(&mut self, _root: &Path, _path: &Path) -> Result<Vec<Copyright>> {
            self.error("file")?;
            Ok(vec![Copyright::new(2024, 2024, "Alice")])
        }

        fn replace_if_changed(&mut self, path: &Path, _contents: &str) -> Result<bool> {
            let filename = path.file_name().and_then(|filename| filename.to_str());
            let failure = match filename {
                Some("license.md") => "replace-license",
                Some("readme.md") => "replace-readme",
                Some("lib.rs") => "replace-source",
                _ => "",
            };
            self.error(failure)?;
            Ok(false)
        }
    }

    #[test]
    fn removes_the_license_section_until_the_next_equal_or_higher_heading() {
        let document = "# Project\n\n## License\n\nMIT terms.\n\n### Details\n\nMore terms.\n\n## Usage\n\nUse the project.\n";
        assert_eq!(
            remove_license_section(document),
            "# Project\n\n## Usage\n\nUse the project.\n"
        );
    }

    #[test]
    fn leaves_license_text_inside_html_blocks_untouched() {
        let document = "# Project\n\n<div>\n## License\n</div>\n\n## Usage\n\nUse the project.\n";
        assert_eq!(remove_license_section(document), document);
    }

    #[test]
    fn leaves_documents_without_a_license_heading_untouched() {
        let document = "# Project\n\n## Usage\n\nUse the project.\n";
        assert_eq!(remove_license_section(document), document);
    }

    #[test]
    fn removes_case_insensitive_license_sections_through_the_end_of_a_document() {
        let document = "# Project\n\n## LICENSE\n\nMIT terms.\n";
        assert_eq!(remove_license_section(document), "# Project\n\n");
    }

    #[test]
    fn ignores_license_headings_nested_inside_a_removed_section() {
        let document =
            "# Project\n\n## License\n\n### License\n\nNested terms.\n\n## Usage\n\nUse it.\n";
        assert_eq!(
            remove_license_section(document),
            "# Project\n\n## Usage\n\nUse it.\n"
        );
    }

    #[test]
    fn removes_formatted_license_headings_and_preserves_nested_headings() {
        for ending in ["\n", "\r", "\r\n"] {
            let source = "# Project\n\n> ## License\n> Quoted terms.\n\n## **Lic&#101;nse**\n\nTerms.\n\n## Usage\n".replace('\n', ending);
            let expected =
                "# Project\n\n> ## License\n> Quoted terms.\n\n## Usage\n".replace('\n', ending);
            assert_eq!(remove_license_section(&source), expected);
        }
    }

    #[test]
    fn renders_license_copyrights_with_an_optional_author_section() {
        let empty = license_document(&[]);
        assert!(empty.starts_with("# MIT License\n\nPermission is hereby granted"));

        let with_authors = license_document(&[
            Copyright::new(2020, 2020, "Alice"),
            Copyright::new(2021, 2024, "Bob."),
        ]);
        assert!(with_authors.starts_with(
            "# MIT License\n\nCopyright, 2020, by Alice.  \nCopyright, 2021-2024, by Bob.  \n\nPermission is hereby granted"
        ));
        assert!(with_authors.ends_with("SOFTWARE.\n"));
    }

    #[test]
    fn updates_license_readme_and_tracked_sources_idempotently() {
        let repository = GitRepository::new();
        repository.write(
            "readme.md",
            "# Project\n\n## License\n\nMIT terms.\n\n## Usage\n\nUse the project.\n",
        );
        repository.write(
            "src/lib.rs",
            "//! Module documentation.\npub fn example() {}\n",
        );
        repository.commit(
            "Alice",
            "alice@example.com",
            "2024-01-02T03:04:05+00:00",
            "Add project files",
        );

        let first = update(repository.root()).unwrap();
        assert_eq!(
            first,
            UpdateSummary {
                source_files: 1,
                files_changed: 3,
            }
        );
        assert!(
            fs::read_to_string(repository.root().join("license.md"))
                .unwrap()
                .starts_with("# MIT License\n\nCopyright, 2024, by Alice.")
        );
        assert_eq!(
            fs::read_to_string(repository.root().join("readme.md")).unwrap(),
            "# Project\n\n## Usage\n\nUse the project.\n"
        );
        assert!(
            fs::read_to_string(repository.root().join("src/lib.rs"))
                .unwrap()
                .contains("// Copyright, 2024, by Alice.\n")
        );

        assert_eq!(
            update(repository.root()).unwrap(),
            UpdateSummary {
                source_files: 1,
                files_changed: 0,
            }
        );
    }

    #[test]
    fn skips_missing_readmes_deleted_sources_and_files_without_authors() {
        let repository = GitRepository::new();
        repository.write("src/deleted.rs", "pub fn deleted() {}\n");
        repository.commit(
            "Alice",
            "alice@example.com",
            "2024-01-02T03:04:05+00:00",
            "Add deleted source",
        );
        repository.write("src/generated.rs", "pub fn generated() {}\n");
        repository.commit(
            "automation[bot]",
            "bot@example.com",
            "2025-02-03T04:05:06+00:00",
            "Add generated source",
        );
        fs::remove_file(repository.root().join("src/deleted.rs")).unwrap();
        assert!(
            authorship::rust_files(repository.root())
                .unwrap()
                .contains(&"src/deleted.rs".to_owned())
        );

        assert_eq!(
            update(repository.root()).unwrap(),
            UpdateSummary {
                source_files: 0,
                files_changed: 1,
            }
        );
        assert!(!repository.root().join("readme.md").exists());
        assert_eq!(
            fs::read_to_string(repository.root().join("src/generated.rs")).unwrap(),
            "pub fn generated() {}\n"
        );
    }

    #[test]
    fn reports_invalid_readme_and_source_text() {
        let readme = GitRepository::new();
        readme.write("readme.md", [0xff]);
        readme.write("src/lib.rs", "pub fn example() {}\n");
        readme.commit(
            "Alice",
            "alice@example.com",
            "2024-01-02T03:04:05+00:00",
            "Add invalid readme",
        );
        assert!(update(readme.root()).is_err());

        let source = GitRepository::new();
        source.write("src/lib.rs", [0xff]);
        source.commit(
            "Alice",
            "alice@example.com",
            "2024-01-02T03:04:05+00:00",
            "Add invalid source",
        );
        assert!(update(source.root()).is_err());
    }

    #[test]
    fn propagates_errors_from_each_project_update_stage() {
        for failure in [
            "repository",
            "replace-license",
            "replace-readme",
            "rust-files",
            "file",
            "replace-source",
        ] {
            let directory = tempdir().unwrap();
            if failure == "replace-readme" {
                fs::write(
                    directory.path().join("readme.md"),
                    "# Project\n\n## License\n\nMIT terms.\n",
                )
                .unwrap();
            }
            if matches!(failure, "file" | "replace-source") {
                let source = directory.path().join("src/lib.rs");
                fs::create_dir_all(source.parent().unwrap()).unwrap();
                fs::write(source, "pub fn example() {}\n").unwrap();
            }

            let mut operations = FailingOperations { failure };
            let error = update_with(directory.path(), &mut operations).unwrap_err();
            assert!(error.to_string().contains(failure), "{failure}: {error}");
        }

        let directory = tempdir().unwrap();
        let mut operations = FailingOperations { failure: "unused" };
        assert!(update_with(directory.path(), &mut operations).is_ok());
        assert!(
            operations
                .replace_if_changed(&directory.path().join("other.md"), "content")
                .is_ok()
        );
    }

    #[test]
    fn replace_if_changed_creates_updates_and_preserves_file_permissions() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("license.md");
        assert!(replace_if_changed(&path, "first\n").unwrap());
        assert!(!replace_if_changed(&path, "first\n").unwrap());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        }

        assert!(replace_if_changed(&path, "second\n").unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), "second\n");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o640
            );
        }
    }

    #[test]
    fn reports_invalid_target_text_missing_parent_and_unreadable_directories() {
        let directory = tempdir().unwrap();
        let invalid = directory.path().join("invalid.md");
        fs::write(&invalid, [0xff]).unwrap();
        assert!(replace_if_changed(&invalid, "content").is_err());

        assert!(replace_if_changed(Path::new(""), "content").is_err());
        assert!(replace_if_changed(&directory.path().join("missing/file.md"), "content").is_err());

        let folder = directory.path().join("folder");
        fs::create_dir(&folder).unwrap();
        assert!(replace_if_changed(&folder, "content").is_err());
    }

    #[test]
    fn propagates_temporary_replacement_failures() {
        #[derive(Clone, Copy)]
        enum Failure {
            Write,
            Permissions,
            Sync,
            Persist,
            Pass,
        }

        struct FailingFile(Failure);

        impl Write for FailingFile {
            fn write(&mut self, contents: &[u8]) -> std::io::Result<usize> {
                if matches!(self.0, Failure::Write) {
                    Err(std::io::Error::other("injected write failure"))
                } else {
                    Ok(contents.len())
                }
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        impl TemporaryReplacementFile for FailingFile {
            fn set_permissions(&self, _permissions: fs::Permissions) -> std::io::Result<()> {
                if matches!(self.0, Failure::Permissions) {
                    Err(std::io::Error::other("injected permissions failure"))
                } else {
                    Ok(())
                }
            }

            fn sync_all(&self) -> std::io::Result<()> {
                if matches!(self.0, Failure::Sync) {
                    Err(std::io::Error::other("injected sync failure"))
                } else {
                    Ok(())
                }
            }

            fn persist(self, _path: &Path) -> std::io::Result<()> {
                if matches!(self.0, Failure::Persist) {
                    Err(std::io::Error::other("injected persist failure"))
                } else {
                    Ok(())
                }
            }
        }

        let directory = tempdir().unwrap();
        let target = directory.path().join("target");
        fs::write(&target, "old").unwrap();

        for (failure, expected) in [
            (Failure::Write, "write"),
            (Failure::Permissions, "permissions"),
            (Failure::Sync, "sync"),
            (Failure::Persist, "persist"),
        ] {
            assert!(
                write_temporary_file(FailingFile(failure), &target, "new")
                    .unwrap_err()
                    .to_string()
                    .contains(expected)
            );
        }

        let mut passing_file = FailingFile(Failure::Pass);
        passing_file.flush().unwrap();
        assert!(write_temporary_file(passing_file, &target, "new").is_ok());

        assert!(
            replace_if_changed_using(&directory.path().join("other"), "new", |_, _, _| {
                Err(Error::new("injected replacement failure"))
            })
            .unwrap_err()
            .to_string()
            .contains("injected replacement failure")
        );
    }
}
