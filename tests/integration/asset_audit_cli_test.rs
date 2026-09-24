use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn audit_module_has_no_process_launch_capability() {
    let source = fs::read_to_string(workspace_root().join("cli/src/audit_command.rs")).unwrap();
    for forbidden in ["std::process", "Command", ".spawn(", ".status(", ".output("] {
        assert!(
            !source.contains(forbidden),
            "audit module must not contain process API token {forbidden}"
        );
    }
    assert!(source.contains("trait AuditExecutionContext"));
}

fn fixture(name: &str) -> PathBuf {
    workspace_root().join("tests").join("fixtures").join(name)
}

fn temp_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "runtimepack-asset-audit-{name}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn run_audit(working_dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .current_dir(working_dir)
        .arg("audit")
        .args(args)
        .output()
        .unwrap()
}

fn source_snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, current: &Path, snapshot: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut entries = fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, snapshot);
            } else {
                snapshot.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }

    let mut snapshot = BTreeMap::new();
    visit(root, root, &mut snapshot);
    snapshot
}

fn read_report(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn temp_artifacts(report_path: &Path) -> Vec<PathBuf> {
    let prefix = format!(".{}.", report_path.file_name().unwrap().to_string_lossy());
    fs::read_dir(report_path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(&prefix)
        })
        .collect()
}

#[test]
fn audit_defaults_to_standard_without_network_and_does_not_mutate_source() {
    let working_dir = temp_dir("default");
    let source = fixture("vxace_min");
    let before = source_snapshot(&source);

    let output = run_audit(&working_dir, &[source.to_str().unwrap()]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(source_snapshot(&source), before);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Network audit: disabled"));
    let report_path = working_dir.join("asset-audit-report.json");
    let report = read_report(&report_path);
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["engine"], "rpg-maker-vx-ace");
    assert_eq!(report["mode"], "STANDARD");
    assert_eq!(report["outcome"], "INDETERMINATE");
    assert_eq!(report["findings"][0]["code"], "CMP-AUD-001");
    assert_eq!(report["runtime_validation"], "NOT_RUN");
    assert!(!working_dir.join(".asset-audit-report.json.tmp").exists());

    fs::remove_dir_all(working_dir).unwrap();
}

#[test]
fn audit_deep_mode_and_network_flag_are_reflected_by_execution() {
    let working_dir = temp_dir("deep");
    let source = fixture("mv_min");
    let report_path = working_dir.join("deep-report.json");

    let output = run_audit(
        &working_dir,
        &[
            source.to_str().unwrap(),
            "--mode",
            "deep",
            "--allow-network-audit",
            "--output",
            report_path.to_str().unwrap(),
        ],
    );

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Network audit: enabled"));
    assert_eq!(read_report(&report_path)["mode"], "DEEP");

    fs::remove_dir_all(working_dir).unwrap();
}

#[test]
fn every_v1_engine_uses_an_unavailable_placeholder_until_adapters_land() {
    for (fixture_name, expected_engine) in [
        ("xp_min", "rpg-maker-xp"),
        ("vx_min", "rpg-maker-vx"),
        ("vxace_min", "rpg-maker-vx-ace"),
        ("mv_min", "rpg-maker-mv"),
        ("mz_min", "rpg-maker-mz"),
        ("renpy_supported_version_A", "ren-py"),
        ("tyrano_min", "tyrano"),
    ] {
        let working_dir = temp_dir(fixture_name);
        let source = fixture(fixture_name);
        let report_path = working_dir.join("report.json");
        let output = run_audit(
            &working_dir,
            &[
                source.to_str().unwrap(),
                "--output",
                report_path.to_str().unwrap(),
            ],
        );

        assert!(
            output.status.success(),
            "{fixture_name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report = read_report(&report_path);
        assert_eq!(report["engine"], expected_engine, "{fixture_name}");
        assert_eq!(report["outcome"], "INDETERMINATE", "{fixture_name}");
        assert_eq!(
            report["findings"][0]["code"], "CMP-AUD-001",
            "{fixture_name}"
        );
        fs::remove_dir_all(working_dir).unwrap();
    }
}

#[test]
fn audit_replaces_an_existing_report() {
    let working_dir = temp_dir("replace");
    let source = fixture("vxace_min");
    let report_path = working_dir.join("report.json");
    fs::write(&report_path, b"old report").unwrap();

    let output = run_audit(
        &working_dir,
        &[
            source.to_str().unwrap(),
            "--output",
            report_path.to_str().unwrap(),
        ],
    );

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(read_report(&report_path)["engine"], "rpg-maker-vx-ace");
    assert!(temp_artifacts(&report_path).is_empty());
    fs::remove_dir_all(working_dir).unwrap();
}

#[test]
fn failed_replacement_preserves_the_old_destination_and_cleans_temp_files() {
    let working_dir = temp_dir("failed-replace");
    let source = fixture("vxace_min");
    let report_path = working_dir.join("report.json");
    fs::create_dir(&report_path).unwrap();
    fs::write(report_path.join("old-report"), b"preserve me").unwrap();

    let output = run_audit(
        &working_dir,
        &[
            source.to_str().unwrap(),
            "--output",
            report_path.to_str().unwrap(),
        ],
    );

    assert!(!output.status.success());
    assert_eq!(
        fs::read(report_path.join("old-report")).unwrap(),
        b"preserve me"
    );
    assert!(temp_artifacts(&report_path).is_empty());
    fs::remove_dir_all(working_dir).unwrap();
}

#[test]
fn concurrent_audits_leave_one_complete_report_and_no_temp_files() {
    let working_dir = temp_dir("concurrent");
    let report_path = working_dir.join("report.json");
    let handles = ["vxace_min", "mv_min"].map(|fixture_name| {
        let working_dir = working_dir.clone();
        let report_path = report_path.clone();
        thread::spawn(move || {
            let source = fixture(fixture_name);
            run_audit(
                &working_dir,
                &[
                    source.to_str().unwrap(),
                    "--output",
                    report_path.to_str().unwrap(),
                ],
            )
        })
    });

    for output in handles.into_iter().map(|handle| handle.join().unwrap()) {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let report = read_report(&report_path);
    assert!(matches!(
        report["engine"].as_str(),
        Some("rpg-maker-vx-ace" | "rpg-maker-mv")
    ));
    assert_eq!(report["outcome"], "INDETERMINATE");
    assert!(temp_artifacts(&report_path).is_empty());
    fs::remove_dir_all(working_dir).unwrap();
}

#[test]
fn invalid_mode_returns_stable_diagnostic_without_report() {
    let working_dir = temp_dir("invalid-mode");
    let source = fixture("vxace_min");

    let output = run_audit(
        &working_dir,
        &[source.to_str().unwrap(), "--mode", "exhaustive"],
    );

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("[CMP-AUD-010]"));
    assert!(!working_dir.join("asset-audit-report.json").exists());

    fs::remove_dir_all(working_dir).unwrap();
}

#[test]
fn output_inside_source_is_rejected_without_creating_a_report() {
    let working_dir = temp_dir("source-output");
    let source = fixture("vxace_min");
    let report_path = source.join("asset-audit-report.json");

    let output = run_audit(
        &working_dir,
        &[
            source.to_str().unwrap(),
            "--output",
            report_path.to_str().unwrap(),
        ],
    );

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("[CMP-AUD-010]"));
    assert!(!report_path.exists());

    fs::remove_dir_all(working_dir).unwrap();
}
