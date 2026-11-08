// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use std::process::Command;

#[test]
fn lists_standard_project_tasks() {
    let output = Command::new(env!("CARGO_BIN_EXE_bake-license-project"))
        .arg("--list")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run the Bake task executable");

    assert!(
        output.status.success(),
        "task listing failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = String::from_utf8(output.stdout).expect("task listing is UTF-8");
    for task in [
        "agent:context:install",
        "cargo:after_version_bump",
        "cargo:release",
        "license:check",
        "license:update",
        "readme:update",
        "releases:update",
        "test",
        "test:coverage",
    ] {
        assert!(
            output.contains(task),
            "missing task {task:?} from:\n{output}"
        );
    }
}
