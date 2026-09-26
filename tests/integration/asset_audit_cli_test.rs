use runtimepack_core::ingest::{find_system_rtp_path, GameSourceIndex};
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
    assert_eq!(report["outcome"], "FAIL");
    assert_eq!(report["runtime_validation"], "NOT_RUN");
    assert!(report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .all(|finding| finding["code"] != "CMP-AUD-001"));
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
fn every_v1_engine_uses_real_adapter_and_produces_real_audit_report() {
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
        assert_eq!(report["schema_version"], 1, "{fixture_name}");
        assert_eq!(report["runtime_validation"], "NOT_RUN", "{fixture_name}");
        // Real adapters must not emit placeholder CMP-AUD-001
        let findings = report["findings"].as_array().unwrap();
        for finding in findings {
            assert_ne!(
                finding["code"].as_str(),
                Some("CMP-AUD-001"),
                "{fixture_name} findings should not contain unavailable adapter CMP-AUD-001"
            );
        }
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
    assert_eq!(report["outcome"], "FAIL");
    assert!(report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .all(|finding| finding["code"] != "CMP-AUD-001"));
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

#[test]
fn audit_resolves_missing_assets_with_rtp_path() {
    let working_dir = temp_dir("audit-rtp");
    let game_source = workspace_root().join("tests/fixtures/rgss_asset_audit/vxace_missing_asset");
    let rtp_dir = temp_dir("rtp-source");

    // Provision missing PictureAce.png in external RTP folder
    let rtp_pic = rtp_dir.join("Graphics/Pictures");
    fs::create_dir_all(&rtp_pic).unwrap();
    let src_pic = workspace_root().join("tests/fixtures/rgss_asset_audit/vxace_valid/Graphics/Pictures/PictureAce.png");
    fs::copy(&src_pic, rtp_pic.join("PictureAce.png")).unwrap();

    // 1. Audit without --rtp-path fails due to missing PictureAce
    let report_no_rtp = working_dir.join("report-no-rtp.json");
    let output_no_rtp = run_audit(
        &working_dir,
        &[
            game_source.to_str().unwrap(),
            "--output",
            report_no_rtp.to_str().unwrap(),
        ],
    );
    assert!(output_no_rtp.status.success());
    let report_data = read_report(&report_no_rtp);
    assert_eq!(report_data["outcome"], "FAIL");
    assert!(report_data["findings"].as_array().unwrap().iter().any(|f| {
        f["message"].as_str().map(|m| m.contains("PictureAce")).unwrap_or(false)
    }));

    // 2. Audit with --rtp-path resolves PictureAce from RTP folder
    let report_rtp = working_dir.join("report-rtp.json");
    let output_rtp = run_audit(
        &working_dir,
        &[
            game_source.to_str().unwrap(),
            "--rtp-path",
            rtp_dir.to_str().unwrap(),
            "--output",
            report_rtp.to_str().unwrap(),
        ],
    );
    assert!(output_rtp.status.success(), "{}", String::from_utf8_lossy(&output_rtp.stderr));
    let stdout = String::from_utf8_lossy(&output_rtp.stdout);
    assert!(stdout.contains("RTP path:"));

    let report_rtp_data = read_report(&report_rtp);
    assert_ne!(report_rtp_data["outcome"], "FAIL");
    assert!(!report_rtp_data["findings"].as_array().unwrap().iter().any(|f| {
        f["message"].as_str().map(|m| m.contains("PictureAce")).unwrap_or(false)
    }));

    fs::remove_dir_all(working_dir).unwrap();
    fs::remove_dir_all(rtp_dir).unwrap();
}

#[test]
fn test_find_system_rtp_path_via_env_var() {
    let temp_rtp = temp_dir("env-rtp-test");
    let marker = temp_rtp.join("marker.txt");
    fs::write(&marker, b"test rtp").unwrap();

    std::env::set_var("RPGVXACE_RTP_PATH", &temp_rtp);
    let resolved = find_system_rtp_path("RPGVXAce");
    std::env::remove_var("RPGVXACE_RTP_PATH");

    assert!(resolved.is_some(), "Expected find_system_rtp_path to detect env var");
    let resolved_path = resolved.unwrap();
    assert!(resolved_path.join("marker.txt").exists());

    fs::remove_dir_all(temp_rtp).unwrap();
}

#[test]
fn test_find_system_rtp_path_nonexistent() {
    let resolved = find_system_rtp_path("NONEXISTENT_RTP_NAME_XYZ_99999");
    assert!(resolved.is_none());
}

#[test]
fn test_find_system_rtp_path_system_windows() {
    if cfg!(windows) {
        let system_common = Path::new(r"C:\Program Files (x86)\Common Files\Enterbrain\RGSS3\RPGVXAce");
        if system_common.exists() {
            let resolved = find_system_rtp_path("RPGVXAce");
            assert!(resolved.is_some(), "Expected find_system_rtp_path to find installed Enterbrain RTP");
            let p = resolved.unwrap();
            assert!(p.exists());
            assert!(p.join("Graphics").exists() || p.join("Audio").exists() || p.join("Fonts").exists());
        }
    }
}

#[test]
fn test_build_with_auto_rtp_without_rtp_declaration() {
    let source = fixture("vxace_min");
    let index = GameSourceIndex::build_with_auto_rtp(&source, None).unwrap();
    assert!(index.has_file("Game.ini"));
    assert!(index.has_file("Data/Scripts.rvdata2"));
}

#[test]
fn test_build_with_auto_rtp_with_game_ini_declaration() {
    let mock_game = temp_dir("mock-game-auto-rtp");
    let mock_rtp = temp_dir("mock-rtp-auto-rtp");

    let game_ini_content = "[Game]\r\nLibrary=System\\RGSS301.dll\r\nScripts=Data\\Scripts.rvdata2\r\nTitle=Test Game\r\nRTP=RPGVXAce\r\n";
    fs::write(mock_game.join("Game.ini"), game_ini_content).unwrap();
    fs::create_dir_all(mock_game.join("Data")).unwrap();
    fs::write(mock_game.join("Data/Scripts.rvdata2"), b"mock scripts").unwrap();

    let rtp_mist = mock_rtp.join("Graphics/Titles2");
    fs::create_dir_all(&rtp_mist).unwrap();
    fs::write(rtp_mist.join("Mist.png"), b"fake mist").unwrap();

    std::env::set_var("RPGVXACE_RTP_PATH", &mock_rtp);
    let index = GameSourceIndex::build_with_auto_rtp(&mock_game, None).unwrap();
    std::env::remove_var("RPGVXACE_RTP_PATH");

    assert!(index.has_file("Game.ini"));
    assert!(index.has_file("Data/Scripts.rvdata2"));
    assert!(index.has_file("Graphics/Titles2/Mist.png"), "Expected auto-RTP to merge Mist.png");

    let file_info = index.get_file("Graphics/Titles2/Mist.png").unwrap();
    assert!(file_info.source_root.is_some());

    // Game directory must remain untouched (immutable)
    assert!(!mock_game.join("Graphics").exists());

    fs::remove_dir_all(mock_game).unwrap();
    fs::remove_dir_all(mock_rtp).unwrap();
}

#[test]
fn test_build_with_auto_rtp_explicit_override() {
    let mock_game = temp_dir("mock-game-explicit-override");
    let auto_rtp = temp_dir("mock-rtp-auto");
    let explicit_rtp = temp_dir("mock-rtp-explicit");

    let game_ini_content = "[Game]\r\nLibrary=System\\RGSS301.dll\r\nScripts=Data\\Scripts.rvdata2\r\nTitle=Test Game\r\nRTP=RPGVXAce\r\n";
    fs::write(mock_game.join("Game.ini"), game_ini_content).unwrap();
    fs::create_dir_all(mock_game.join("Data")).unwrap();
    fs::write(mock_game.join("Data/Scripts.rvdata2"), b"mock scripts").unwrap();

    // Auto RTP has Mist.png
    let auto_titles = auto_rtp.join("Graphics/Titles2");
    fs::create_dir_all(&auto_titles).unwrap();
    fs::write(auto_titles.join("Mist.png"), b"auto mist").unwrap();

    // Explicit RTP has Overridden.png
    let explicit_titles = explicit_rtp.join("Graphics/Titles2");
    fs::create_dir_all(&explicit_titles).unwrap();
    fs::write(explicit_titles.join("Overridden.png"), b"explicit asset").unwrap();

    std::env::set_var("RPGVXACE_RTP_PATH", &auto_rtp);
    let index = GameSourceIndex::build_with_auto_rtp(&mock_game, Some(&explicit_rtp)).unwrap();
    std::env::remove_var("RPGVXACE_RTP_PATH");

    assert!(index.has_file("Graphics/Titles2/Overridden.png"));
    assert!(!index.has_file("Graphics/Titles2/Mist.png"));

    fs::remove_dir_all(mock_game).unwrap();
    fs::remove_dir_all(auto_rtp).unwrap();
    fs::remove_dir_all(explicit_rtp).unwrap();
}
