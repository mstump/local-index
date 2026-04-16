use std::fs;
use std::process::Command;

/// Get the binary path. The binary must already be built (cargo test builds it).
fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_local-index"))
}

fn run_index(
    vault_path: &str,
    extra_args: &[&str],
    env_vars: &[(&str, &str)],
    remove_vars: &[&str],
) -> std::process::Output {
    let bin = binary_path();
    let mut cmd = Command::new(&bin);
    cmd.args(["--log-level", "warn", "index", vault_path]);
    cmd.args(extra_args);

    // Set current_dir to the vault path (or /tmp) to prevent dotenvy from
    // finding the project's .env file, which would override env_remove.
    let cwd = if std::path::Path::new(vault_path).exists() {
        std::path::PathBuf::from(vault_path)
    } else {
        std::env::temp_dir()
    };
    cmd.current_dir(&cwd);

    // Clear potentially inherited VOYAGE_API_KEY by default
    cmd.env_remove("VOYAGE_API_KEY");

    for var in remove_vars {
        cmd.env_remove(var);
    }
    for (k, v) in env_vars {
        cmd.env(k, v);
    }

    cmd.output().expect("failed to run binary")
}

#[test]
fn test_index_no_credentials() {
    let dir = tempfile::tempdir().unwrap();

    // Create a markdown file so the command actually tries to do work
    fs::write(dir.path().join("note.md"), "# Test\nSome content\n").unwrap();

    let output = run_index(dir.path().to_str().unwrap(), &[], &[], &["VOYAGE_API_KEY"]);

    assert!(
        !output.status.success(),
        "should fail without VOYAGE_API_KEY"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("VOYAGE_API_KEY"),
        "stderr should mention VOYAGE_API_KEY, got: {}",
        stderr
    );
    assert!(
        stderr.contains("https://dash.voyageai.com/"),
        "stderr should contain actionable guidance, got: {}",
        stderr
    );
}

#[test]
fn test_index_empty_vault() {
    let dir = tempfile::tempdir().unwrap();

    let output = run_index(
        dir.path().to_str().unwrap(),
        &[],
        &[("VOYAGE_API_KEY", "fake-key-for-test")],
        &[],
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "empty vault should exit 0, stderr: {}",
        stderr
    );

    // Non-TTY (piped) should produce JSON with 0 files
    assert!(
        stdout.contains("files_indexed") || stdout.contains("0 files"),
        "should report 0 files, got stdout: {}",
        stdout
    );
}

#[test]
fn test_index_force_reindex_flag() {
    let dir = tempfile::tempdir().unwrap();

    let output = run_index(
        dir.path().to_str().unwrap(),
        &["--force-reindex"],
        &[("VOYAGE_API_KEY", "fake-key-for-test")],
        &[],
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "should accept --force-reindex flag, stderr: {}",
        stderr
    );
}

#[test]
fn test_index_nonexistent_path() {
    let output = run_index(
        "/nonexistent_dir_xyz_local_index_test",
        &[],
        &[("VOYAGE_API_KEY", "fake-key-for-test")],
        &[],
    );

    assert!(
        !output.status.success(),
        "nonexistent path should exit non-zero"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Invalid vault path"),
        "stderr should mention invalid path, got: {}",
        stderr
    );
}

#[test]
fn test_index_json_output_non_tty() {
    let dir = tempfile::tempdir().unwrap();

    // When running via Command (piped), stdout is non-TTY, so output should be JSON
    let output = run_index(
        dir.path().to_str().unwrap(),
        &[],
        &[("VOYAGE_API_KEY", "fake-key-for-test")],
        &[],
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "should exit 0, stderr: {}", stderr);

    // Parse stdout as JSON
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "stdout should be valid JSON, got '{}', error: {}",
            stdout, e
        )
    });

    assert!(
        json.get("files_indexed").is_some(),
        "JSON should contain files_indexed key"
    );
    assert!(
        json.get("chunks_embedded").is_some(),
        "JSON should contain chunks_embedded key"
    );
    assert!(
        json.get("chunks_skipped").is_some(),
        "JSON should contain chunks_skipped key"
    );
    assert!(
        json.get("errors").is_some(),
        "JSON should contain errors key"
    );
    assert!(
        json.get("orphan_files_removed").is_some(),
        "JSON should contain orphan_files_removed key"
    );
}

#[test]
#[ignore] // Only runs with real VOYAGE_API_KEY set
fn test_index_with_real_api() {
    let dir = tempfile::tempdir().unwrap();

    fs::write(
        dir.path().join("note1.md"),
        "---\ntags:\n  - test\n---\n# Heading One\nBody of heading one.\n## Sub Heading\nBody of sub heading.\n",
    )
    .unwrap();

    let subdir = dir.path().join("subdir");
    fs::create_dir_all(&subdir).unwrap();
    fs::write(
        subdir.join("note2.md"),
        "# Single Heading\nSome content here.\n",
    )
    .unwrap();

    let api_key =
        std::env::var("VOYAGE_API_KEY").expect("VOYAGE_API_KEY must be set for this test");

    let bin = binary_path();
    let data_dir = dir.path().join(".local-index");
    let output = Command::new(&bin)
        .args([
            "--log-level",
            "warn",
            "index",
            dir.path().to_str().unwrap(),
            "--data-dir",
            data_dir.to_str().unwrap(),
        ])
        .env("VOYAGE_API_KEY", &api_key)
        .output()
        .expect("failed to run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "real API index should exit 0, stderr: {}, stdout: {}",
        stderr,
        stdout
    );

    let json: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|_| panic!("should be valid JSON: {}", stdout));

    let embedded = json["chunks_embedded"].as_u64().unwrap_or(0);
    assert!(
        embedded > 0,
        "should have embedded at least 1 chunk, got: {}",
        embedded
    );

    assert!(
        data_dir.exists(),
        "data directory should be created at {:?}",
        data_dir
    );
}
