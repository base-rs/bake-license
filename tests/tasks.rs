// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use bake::Registry;
use std::fs;
use std::process::Command;

use bake_license as _;

#[test]
fn registered_task_updates_the_project_root() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn example() {}\n").unwrap();
    fs::write(
        root.join("readme.md"),
        "# Fixture\n\n## License\n\nOld license text.\n\n## Contributing\n\nOpen an issue.\n",
    )
    .unwrap();

    let output = Command::new("git")
        .current_dir(root)
        .args(["init", "--quiet"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new("git")
        .current_dir(root)
        .args(["add", "--all"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new("git")
        .current_dir(root)
        .args([
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
        ])
        .env("GIT_AUTHOR_DATE", "2024-06-01T00:00:00+00:00")
        .env("GIT_COMMITTER_DATE", "2024-06-01T00:00:00+00:00")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut context = Registry::discover().unwrap().context(root);
    let result = context.call("license:update", &[]).unwrap();

    assert_eq!(
        result.as_str(),
        Some("Changed 3 file(s); refreshed 1 tracked Rust source file(s)")
    );
    assert!(root.join("license.md").is_file());
    assert!(
        fs::read_to_string(root.join("src/lib.rs"))
            .unwrap()
            .starts_with("// Released under the MIT License.")
    );
    let readme = fs::read_to_string(root.join("readme.md")).unwrap();
    assert!(!readme.contains("Old license text."));
    assert!(readme.contains("## Contributing"));
}
