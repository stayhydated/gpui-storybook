use std::{path::PathBuf, process::Command};

#[test]
fn duplicate_story_keys_fail_to_build() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = manifest_dir.join("tests/fixtures/duplicate-story-key");

    let output = Command::new(env!("CARGO"))
        .arg("build")
        .arg("--quiet")
        .current_dir(fixture)
        .output()
        .expect("fixture build should run");

    assert!(
        !output.status.success(),
        "duplicate story key fixture unexpectedly built successfully"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "__gpui_storybook_story_key__gpui-storybook-duplicate-fixture__DuplicateStory"
        ) || stderr.contains("multiple definition")
            || stderr.contains("defined multiple times"),
        "duplicate story key fixture failed without the expected duplicate symbol diagnostic:\n{stderr}"
    );
}
