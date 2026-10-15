use std::path::Path;
use std::process::Command;

fn crownplugin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_crownplugin"))
}

#[test]
fn every_example_validates() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
    for entry in std::fs::read_dir(examples).expect("examples") {
        let dir = entry.expect("entry").path();
        let output = crownplugin().arg("validate").arg(&dir).output().expect("runs");
        assert!(
            output.status.success(),
            "{}:\n{}",
            dir.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn fresh_templates_validate() {
    let root = tempfile::tempdir().expect("tempdir");
    for (kind, id) in [("plugin", "dev.test.fresh-plugin"), ("theme", "dev.test.fresh-theme")] {
        let dir = root.path().join(id);
        let created = crownplugin().args(["new", kind, id, "--dir"]).arg(&dir).status().expect("runs");
        assert!(created.success());
        let output = crownplugin().arg("validate").arg(&dir).output().expect("runs");
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    }
}

#[test]
fn errors_point_at_the_field_and_fail_the_command() {
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        root.path().join("plugin.ron"),
        r#"Plugin(id: "dev.test.bad", name: "Bad", version: "1", contributes: (config: {"appearance": {"blur_passes": "x"}}))"#,
    )
    .expect("write");
    std::fs::write(root.path().join("main.luau"), "local x = = 1").expect("write");
    let output = crownplugin().arg("validate").arg(root.path()).output().expect("runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("error: version"), "{stderr}");
    assert!(stderr.contains("config.appearance.blur_passes"), "{stderr}");
    assert!(stderr.contains("main.luau:1"), "{stderr}");
}
