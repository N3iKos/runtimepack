//! Permanent parity gate: `inspect` and `audit` must report the same engine.
//!
//! Both subcommands consume the single detector list in
//! `runtimepack_cli::detection_registry::default_detector_registry()`. This test
//! runs the real binary twice per fixture and compares the reported engine, so a
//! second detector list with different order or content (in `main.rs` or
//! `audit_command.rs`) fails here instead of drifting silently.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn fixture(name: &str) -> PathBuf {
    workspace_root().join("tests").join("fixtures").join(name)
}

/// Unique scratch directory; removed when the guard drops, including on panic.
struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "runtimepack-detector-parity-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run(scratch: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .current_dir(scratch)
        .args(args)
        .output()
        .unwrap()
}

fn line_value(output: &Output, prefix: &str) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .unwrap_or_else(|| {
            panic!(
                "no line starting with {prefix:?} in stdout:\n{}\nstderr:\n{}",
                stdout,
                String::from_utf8_lossy(&output.stderr)
            )
        })
        .trim()
        .to_string()
}

fn assert_engine_parity(fixture_name: &str) {
    let scratch = ScratchDir::new(fixture_name);
    let game = fixture(fixture_name);
    assert!(
        game.is_dir(),
        "fixture {fixture_name} missing at {}",
        game.display()
    );
    let game_arg = game.to_string_lossy().to_string();
    let report = scratch.path().join("audit-report.json");
    let report_arg = report.to_string_lossy().to_string();

    let inspect = run(scratch.path(), &["inspect", &game_arg]);
    assert!(
        inspect.status.success(),
        "inspect failed for {fixture_name}: {}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    let inspect_engine = line_value(&inspect, "Engine:");

    let audit = run(
        scratch.path(),
        &["audit", &game_arg, "--no-html", "-o", &report_arg],
    );
    assert!(
        audit.status.success(),
        "audit failed for {fixture_name}: {}",
        String::from_utf8_lossy(&audit.stderr)
    );
    let audit_engine = line_value(&audit, "Detected engine:");

    assert!(
        !inspect_engine.is_empty(),
        "inspect reported an empty engine for {fixture_name}"
    );
    assert_eq!(
        inspect_engine, audit_engine,
        "inspect and audit disagree on the engine for {fixture_name}"
    );
}

#[test]
fn inspect_and_audit_agree_on_web_fixture() {
    assert_engine_parity("mv_min");
}

#[test]
fn inspect_and_audit_agree_on_renpy_fixture() {
    assert_engine_parity("renpy_v8_min");
}
