use std::process::Command;

fn cargo_bin() -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--quiet", "--"]);
    cmd
}

#[test]
fn test_help_shows_all_subcommands() {
    let output = cargo_bin()
        .arg("--help")
        .output()
        .expect("failed to run cargo");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "help should exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("index"),
        "help should mention 'index' subcommand"
    );
    assert!(
        stdout.contains("daemon"),
        "help should mention 'daemon' subcommand"
    );
    assert!(
        stdout.contains("search"),
        "help should mention 'search' subcommand"
    );
    assert!(
        stdout.contains("status"),
        "help should mention 'status' subcommand"
    );
    assert!(
        stdout.contains("serve"),
        "help should mention 'serve' subcommand"
    );
}

#[test]
fn test_index_help() {
    let output = cargo_bin()
        .args(["index", "--help"])
        .output()
        .expect("failed to run cargo");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "index --help should exit 0");
    assert!(
        stdout.contains("force-reindex"),
        "index help should mention force-reindex flag"
    );
}

#[test]
fn test_invalid_subcommand() {
    let output = cargo_bin()
        .arg("notreal")
        .output()
        .expect("failed to run cargo");

    assert!(
        !output.status.success(),
        "invalid subcommand should exit non-zero"
    );
}
