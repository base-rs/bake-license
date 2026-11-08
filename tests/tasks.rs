// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use bake::Registry;
use std::fs;
use std::path::Path;
use std::process::Command;

use bake_license as _;

const FIXTURE: &str = "fn main() {\n    compile_error!(\"line 2\");\n}\n";

fn git(root: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(arguments)
        .env("GIT_AUTHOR_DATE", "2024-06-01T00:00:00+00:00")
        .env("GIT_COMMITTER_DATE", "2024-06-01T00:00:00+00:00")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A committed project with a library source, a compile-fail fixture, and a README License section.
fn project() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn example() {}\n").unwrap();
    fs::create_dir_all(root.join("tests/ui")).unwrap();
    fs::write(root.join("tests/ui/fails.rs"), FIXTURE).unwrap();
    fs::write(
        root.join("readme.md"),
        "# Fixture\n\n## License\n\nOld license text.\n\n## Contributing\n\nOpen an issue.\n",
    )
    .unwrap();

    git(root, &["init", "--quiet"]);
    git(root, &["add", "--all"]);
    git(
        root,
        &[
            "-c",
            "user.name=Fixture Author",
            "-c",
            "user.email=fixture@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "Add fixture files",
        ],
    );
    directory
}

#[test]
fn registered_task_updates_the_project_root() {
    let directory = project();
    let root = directory.path();

    let mut context = Registry::discover().unwrap().context(root);
    let result = context.call("license:update", &[]).unwrap();

    assert_eq!(
        result.as_str(),
        Some("Changed 4 file(s); refreshed 2 tracked Rust source file(s)")
    );
    assert!(root.join("license.md").is_file());
    for path in ["src/lib.rs", "tests/ui/fails.rs"] {
        assert!(
            fs::read_to_string(root.join(path)).unwrap().starts_with(
                "// Released under the MIT License.\n// Copyright, 2024, by Fixture Author.\n\n"
            ),
            "{path}"
        );
    }
    let readme = fs::read_to_string(root.join("readme.md")).unwrap();
    assert!(!readme.contains("Old license text."));
    assert!(readme.contains("## Contributing"));
}

#[test]
fn registered_check_task_reports_files_until_updated() {
    let directory = project();
    let root = directory.path();

    let mut context = Registry::discover().unwrap().context(root);
    let error = context.call("license:check", &[]).unwrap_err().to_string();
    assert!(
        error.starts_with(
            "license:check: 4 file(s) need license updates: license.md, readme.md, src/lib.rs, tests/ui/fails.rs; run `cargo bake license:update`"
        ),
        "{error}"
    );
    assert!(!root.join("license.md").exists());
    assert_eq!(
        fs::read_to_string(root.join("tests/ui/fails.rs")).unwrap(),
        FIXTURE
    );

    context.call("license:update", &[]).unwrap();
    let result = context.call("license:check", &[]).unwrap();
    assert_eq!(
        result.as_str(),
        Some("License file and Rust source headers are up to date")
    );
}
