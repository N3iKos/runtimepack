use runtimepack_core::asset_audit::{
    AssetAuditOptions, AuditCache, AuditCacheKey, AuditMode, CanonicalAuditArtifacts,
    CURRENT_AUDIT_RULESET_VERSION,
};
use runtimepack_core::domain::EngineId;
use runtimepack_core::ingest::{find_system_rtp_path, GameSourceIndex};
use serde_json::Value;
use sha2::{Digest, Sha256};
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
    assert_eq!(report["engine"], "rpgmaker-vxace");
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
        ("xp_min", "rpgmaker-xp"),
        ("vx_min", "rpgmaker-vx"),
        ("vxace_min", "rpgmaker-vxace"),
        ("mv_min", "rpgmaker-mv"),
        ("mz_min", "rpgmaker-mz"),
        ("renpy_supported_version_A", "renpy"),
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
    assert_eq!(read_report(&report_path)["engine"], "rpgmaker-vxace");
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
        Some("rpgmaker-vxace" | "rpgmaker-mv")
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

static RTP_ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn test_find_system_rtp_path_via_env_var() {
    let _lock = RTP_ENV_MUTEX.lock().unwrap();
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
    let _lock = RTP_ENV_MUTEX.lock().unwrap();
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
    let _lock = RTP_ENV_MUTEX.lock().unwrap();
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

fn run_inspect(working_dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .current_dir(working_dir)
        .arg("inspect")
        .args(args)
        .output()
        .unwrap()
}

fn run_build(working_dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .current_dir(working_dir)
        .arg("build")
        .args(args)
        .output()
        .unwrap()
}

fn setup_nested_mz_wrapper_fixture(dir: &Path, package_main: &str) {
    let payload_dir = dir.join("bin/www");
    fs::create_dir_all(payload_dir.join("data")).unwrap();
    fs::create_dir_all(payload_dir.join("js")).unwrap();
    fs::create_dir_all(dir.join("bin")).unwrap();

    fs::write(
        payload_dir.join("data/System.json"),
        r#"{"gameTitle":"Task3 MZ Game","optSideView":false}"#,
    )
    .unwrap();
    fs::write(
        payload_dir.join("index.html"),
        "<html><body><h1>Task3 MZ Game</h1></body></html>",
    )
    .unwrap();
    fs::write(payload_dir.join("js/rmmz_core.js"), "// rmmz_core").unwrap();
    fs::write(payload_dir.join("js/rmmz_managers.js"), "// rmmz_managers").unwrap();

    // Outer runner binaries and package descriptor
    fs::write(dir.join("bin/nw.dll"), b"MZ\x90\x00runner_dll").unwrap();
    fs::write(
        dir.join("bin/package.json"),
        format!(r#"{{"name":"task3-outer-runner","main":"{}"}}"#, package_main),
    )
    .unwrap();
}

/// Structured report accessors: Task 3 must assert the emitted report fields, not
/// the engine `Display` text or a bare exit status.
fn finding_for_path<'a>(report: &'a Value, logical_path: &str) -> Option<&'a Value> {
    report["findings"].as_array()?.iter().find(|finding| {
        finding["source_node_ids"]
            .as_array()
            .map(|nodes| nodes.iter().any(|node| node.as_str() == Some(logical_path)))
            .unwrap_or(false)
    })
}

fn graph_paths(report: &Value) -> Vec<String> {
    report["graph"]["nodes"]
        .as_array()
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(|node| node["logical_path"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn assert_payload_only_graph(report: &Value, context: &str) {
    let paths = graph_paths(report);
    for outer in ["bin/nw.dll", "bin/nw.exe", "bin/addon.node", "bin/native.so"] {
        assert!(
            !paths.iter().any(|path| path == outer),
            "{context}: outer wrapper file {outer} must not enter the payload-only asset graph: {paths:?}"
        );
    }
    for doubled in ["bin/bin/www", "bin/www/bin/www"] {
        assert!(
            !paths.iter().any(|path| path.starts_with(doubled)),
            "{context}: payload paths must not be double-prefixed with {doubled}: {paths:?}"
        );
    }
    assert!(
        paths.iter().any(|path| path == "index.html"),
        "{context}: payload root entry must stay payload-relative: {paths:?}"
    );
}

#[test]
fn test_cli_inspect_audit_build_parity_nested_wrapper_proven_and_stale_mutation() {
    let working_dir = temp_dir("cli-parity-proven");
    let fixture_dir = working_dir.join("game_source");
    fs::create_dir_all(&fixture_dir).unwrap();

    // 1. Proven nested wrapper: bin/package.json main points to www/index.html (inside payload)
    setup_nested_mz_wrapper_fixture(&fixture_dir, "www/index.html");

    // Inspect: outer runner bin/nw.dll is classified as CMP-106 Info
    let inspect_out = run_inspect(&working_dir, &[fixture_dir.to_str().unwrap()]);
    assert!(
        inspect_out.status.success(),
        "inspect must succeed: {}",
        String::from_utf8_lossy(&inspect_out.stderr)
    );
    let inspect_stdout = String::from_utf8_lossy(&inspect_out.stdout);
    assert!(
        inspect_stdout.contains("[CMP-106] [INFO]"),
        "inspect stdout must report CMP-106 at INFO severity for proven outer runner: {inspect_stdout}"
    );
    assert!(
        inspect_stdout.contains("bin/nw.dll"),
        "inspect stdout must name the outer runner path bin/nw.dll: {inspect_stdout}"
    );
    assert!(
        !inspect_stdout.contains("[CMP-102]"),
        "inspect must not flag proven outer runner as CMP-102: {inspect_stdout}"
    );

    // Audit: report JSON findings must include CMP-106 for bin/nw.dll with INFORMATIONAL disposition
    let report_path = working_dir.join("audit-report.json");
    let audit_out = run_audit(
        &working_dir,
        &[
            fixture_dir.to_str().unwrap(),
            "--no-html",
            "--output",
            report_path.to_str().unwrap(),
        ],
    );
    assert!(
        audit_out.status.success(),
        "audit must complete: {}",
        String::from_utf8_lossy(&audit_out.stderr)
    );
    let report_data = read_report(&report_path);
    let findings = report_data["findings"].as_array().expect("findings array");

    let nw_dll_finding = findings.iter().find(|f| {
        f["source_node_ids"]
            .as_array()
            .map(|nodes| nodes.iter().any(|n| n.as_str() == Some("bin/nw.dll")))
            .unwrap_or(false)
    });
    assert!(
        nw_dll_finding.is_some(),
        "Audit report must include outer bin/nw.dll finding: {findings:?}"
    );
    let nw_finding = nw_dll_finding.unwrap();
    assert_eq!(
        nw_finding["code"], "CMP-106",
        "bin/nw.dll finding code must be CMP-106"
    );
    assert_eq!(
        nw_finding["severity"], "INFO",
        "bin/nw.dll finding severity must be INFO"
    );
    assert_eq!(
        nw_finding["disposition"], "INFORMATIONAL",
        "bin/nw.dll finding disposition must be INFORMATIONAL"
    );
    assert_payload_only_graph(&report_data, "audit command");

    // Capture the distribution metadata hash before the equal-length mutation.
    let source_hash_before = GameSourceIndex::build(&fixture_dir)
        .expect("index proven fixture")
        .source_hash
        .clone();

    // Build with accept-audit flags: must succeed because CMP-106 is informational
    let build_out_dir = working_dir.join("build_out");
    let build_out = run_build(
        &working_dir,
        &[
            fixture_dir.to_str().unwrap(),
            "--output",
            build_out_dir.to_str().unwrap(),
            "--flavor",
            "beta",
            "--accept-audit-fail",
            "--accept-audit-unknown",
            "--no-compile",
            "--no-html",
        ],
    );
    assert!(
        build_out.status.success(),
        "Build must succeed for proven wrapper: STDOUT: {}\nSTDERR: {}",
        String::from_utf8_lossy(&build_out.stdout),
        String::from_utf8_lossy(&build_out.stderr)
    );

    // H1: the emitted BUILD audit report must prove the build auditor received the
    // full effective distribution, not just the scoped payload.
    let build_report = read_report(&build_out_dir.join("asset-audit-report.json"));
    assert_eq!(
        build_report["engine"], "rpgmaker-mz",
        "build report must be the MZ audit report"
    );
    assert_eq!(
        build_report["outcome"], "FAIL",
        "build report must carry the real incomplete-asset audit outcome, not a synthetic pass"
    );
    let build_nw = finding_for_path(&build_report, "bin/nw.dll").unwrap_or_else(|| {
        panic!(
            "build audit report must include the outer wrapper native bin/nw.dll: {:?}",
            build_report["findings"]
        )
    });
    assert_eq!(build_nw["code"], "CMP-106");
    assert_eq!(build_nw["severity"], "INFO");
    assert_eq!(build_nw["disposition"], "INFORMATIONAL");
    assert_payload_only_graph(&build_report, "build command");

    // 2. Mutate equal-length package main: www/index.html -> www/ind_x.html
    // Both entry strings are 14 bytes, so the distribution metadata hash is
    // unchanged while the wrapper proof is destroyed.
    let package_path = fixture_dir.join("bin/package.json");
    let original_package = fs::read(&package_path).unwrap();
    fs::write(
        &package_path,
        r#"{"name":"task3-outer-runner","main":"www/ind_x.html"}"#,
    )
    .unwrap();
    assert_eq!(
        fs::read(&package_path).unwrap().len(),
        original_package.len(),
        "mutation must keep the descriptor byte length identical"
    );
    // M5: the distribution metadata hash is path+size based, so the mutation must
    // leave `source_hash` unchanged while the wrapper verdict changes.
    let source_hash_after = GameSourceIndex::build(&fixture_dir)
        .expect("index mutated fixture")
        .source_hash
        .clone();
    assert_eq!(
        source_hash_before, source_hash_after,
        "equal-length mutation must not change the distribution metadata hash"
    );

    // Inspect on mutated fixture: wrapper is unproven, outer bin/nw.dll becomes CMP-102 Error
    let inspect_mut_out = run_inspect(&working_dir, &[fixture_dir.to_str().unwrap()]);
    let inspect_mut_stdout = String::from_utf8_lossy(&inspect_mut_out.stdout);
    assert!(
        inspect_mut_stdout.contains("[CMP-102] [ERROR]"),
        "Mutated wrapper must cause inspect to flag bin/nw.dll as CMP-102 ERROR: {inspect_mut_stdout}"
    );

    // Audit on mutated fixture: report must classify bin/nw.dll as CMP-102 Error / BLOCKING
    let report_mut_path = working_dir.join("audit-mut-report.json");
    let audit_mut_out = run_audit(
        &working_dir,
        &[
            fixture_dir.to_str().unwrap(),
            "--no-html",
            "--output",
            report_mut_path.to_str().unwrap(),
        ],
    );
    assert!(
        audit_mut_out.status.success(),
        "audit command itself must exit 0: {}",
        String::from_utf8_lossy(&audit_mut_out.stderr)
    );
    let report_mut_data = read_report(&report_mut_path);
    let findings_mut = report_mut_data["findings"].as_array().expect("findings array");
    let nw_mut_finding = findings_mut.iter().find(|f| {
        f["source_node_ids"]
            .as_array()
            .map(|nodes| nodes.iter().any(|n| n.as_str() == Some("bin/nw.dll")))
            .unwrap_or(false)
    });
    assert!(
        nw_mut_finding.is_some(),
        "Mutated audit report must include outer bin/nw.dll finding: {findings_mut:?}"
    );
    let nw_mut = nw_mut_finding.unwrap();
    assert_eq!(
        nw_mut["code"], "CMP-102",
        "Mutated bin/nw.dll must be CMP-102"
    );
    assert_eq!(
        nw_mut["severity"], "ERROR",
        "Mutated bin/nw.dll severity must be ERROR"
    );
    assert_eq!(
        nw_mut["disposition"], "BLOCKING",
        "Mutated bin/nw.dll disposition must be BLOCKING"
    );

    // Build on mutated fixture: MUST FAIL on CMP-102 despite accept-audit flags!
    let build_mut_dir = working_dir.join("build_mut_out");
    let build_mut_out = run_build(
        &working_dir,
        &[
            fixture_dir.to_str().unwrap(),
            "--output",
            build_mut_dir.to_str().unwrap(),
            "--flavor",
            "beta",
            "--accept-audit-fail",
            "--accept-audit-unknown",
            "--no-compile",
            "--no-html",
        ],
    );
    assert!(
        !build_mut_out.status.success(),
        "Build must fail closed on CMP-102 for unproven wrapper"
    );
    let build_mut_stderr = String::from_utf8_lossy(&build_mut_out.stderr);
    assert!(
        build_mut_stderr.contains("CMP-102"),
        "Build stderr must cite CMP-102: {build_mut_stderr}"
    );

    fs::remove_dir_all(working_dir).unwrap();
}

#[test]
fn test_cli_web_audit_and_build_cache_bypass_and_seeded_cache_ignored() {
    let working_dir = temp_dir("cli-cache-bypass");
    let fixture_dir = working_dir.join("game_source");
    fs::create_dir_all(&fixture_dir).unwrap();

    setup_nested_mz_wrapper_fixture(&fixture_dir, "www/index.html");

    // Default cache location is working_dir/.runtimepack/cache/audit
    let default_cache_dir = working_dir.join(".runtimepack/cache/audit");

    // 1. Run audit twice: web audit must NOT write any cache entry to .runtimepack/cache/audit
    let report1 = working_dir.join("report1.json");
    let out1 = run_audit(
        &working_dir,
        &[
            fixture_dir.to_str().unwrap(),
            "--no-html",
            "--output",
            report1.to_str().unwrap(),
        ],
    );
    assert!(out1.status.success());

    assert!(
        !default_cache_dir.exists()
            || fs::read_dir(&default_cache_dir).map(|mut d| d.next().is_none()).unwrap_or(true),
        "Web audit must NOT write any cache entry to .runtimepack/cache/audit"
    );

    let report2 = working_dir.join("report2.json");
    let out2 = run_audit(
        &working_dir,
        &[
            fixture_dir.to_str().unwrap(),
            "--no-html",
            "--output",
            report2.to_str().unwrap(),
        ],
    );
    assert!(out2.status.success());
    assert!(
        !default_cache_dir.exists()
            || fs::read_dir(&default_cache_dir).map(|mut d| d.next().is_none()).unwrap_or(true),
        "Repeated web audit must NOT write any cache entry"
    );

    // 2. Seed a misleading cached entry under the EXACT key production would compute.
    fs::create_dir_all(&default_cache_dir).unwrap();
    let audit_cache = AuditCache::new(&default_cache_dir);

    // Production (`cli/src/main.rs`) derives the build audit key from the scoped
    // payload hash plus limits, allow_network, detected_version, content_root and
    // the current ruleset. Mirror every field so the seeded entry is reachable.
    let source_idx = GameSourceIndex::build(&fixture_dir).unwrap();
    let payload_idx = source_idx.scoped_to("bin/www").unwrap();
    let production_limits = AssetAuditOptions::default().limits.clone();
    let seeded_key = AuditCacheKey::new(
        payload_idx.scoped_content_hash(),
        EngineId::RpgMakerMz,
        AuditMode::Standard,
        CURRENT_AUDIT_RULESET_VERSION,
        env!("CARGO_PKG_VERSION"),
    )
    .with_limits(production_limits)
    .with_allow_network(false)
    .with_detected_version(None)
    .with_content_root(Some("bin/www".to_string()))
    .unwrap();
    let seeded_path = default_cache_dir.join(format!("{}.json", seeded_key.to_sha256()));

    // A structurally valid report claiming PASS with one unique sentinel finding, so
    // serving it changes the emitted build report in an unmistakable way.
    let sentinel_id = "task3-seeded-fake-pass-sentinel";
    let mut fake_report = read_report(&report1);
    fake_report["outcome"] = serde_json::json!("PASS");
    fake_report["findings"] = serde_json::json!([{
        "id": sentinel_id,
        "code": "CMP-106",
        "severity": "INFO",
        "disposition": "INFORMATIONAL",
        "requiredness": "OPTIONAL",
        "message": "seeded fake PASS sentinel; must never be served",
        "source_node_ids": ["sentinel/nw.dll"],
        "orphan": false
    }]);
    fake_report["overrides"] = serde_json::json!([]);
    fake_report["statistics"] = serde_json::json!({
        "nodes": 0,
        "references": 0,
        "findings": 1
    });
    fake_report["completeness"] = serde_json::json!({
        "required_scope_complete": true,
        "covered_reference_classes": ["assets"],
        "uncovered_reference_classes": []
    });
    fake_report["graph"] = serde_json::json!({ "nodes": [], "edges": [] });

    let fake_report_bytes = serde_json::to_vec(&fake_report).unwrap();
    let fake_graph_bytes = b"{\"nodes\":[],\"edges\":[]}".to_vec();
    let fake_artifacts = CanonicalAuditArtifacts {
        report_sha256: format!("{:x}", Sha256::digest(&fake_report_bytes)),
        graph_sha256: format!("{:x}", Sha256::digest(&fake_graph_bytes)),
        report_json: fake_report_bytes,
        graph_json: fake_graph_bytes,
        html_bytes: None,
    };
    audit_cache.put(&seeded_key, &fake_artifacts).unwrap();

    // Guard: the seeded artifact must be a real, hash-consistent cache hit under the
    // production identity, otherwise the bypass assertions below would be vacuous.
    assert!(
        seeded_path.exists(),
        "seeded entry must land at the production key path {}",
        seeded_path.display()
    );
    assert!(
        audit_cache.get(&seeded_key).is_some(),
        "seeded entry must be readable through AuditCache under the production key"
    );
    let seeded_bytes_before = fs::read(&seeded_path).unwrap();

    // 3. Run audit again: it MUST bypass the cache and NOT return the fake PASS report!
    // The real audit will have outcome FAIL (because of incomplete assets like title1Name/windowskin)
    let report3 = working_dir.join("report3.json");
    let out3 = run_audit(
        &working_dir,
        &[
            fixture_dir.to_str().unwrap(),
            "--no-html",
            "--output",
            report3.to_str().unwrap(),
        ],
    );
    assert!(out3.status.success());
    let report3_data = read_report(&report3);
    assert_ne!(
        report3_data["outcome"], "PASS",
        "Web audit must bypass the seeded fake cache entry (outcome must not be PASS)"
    );
    let findings3 = report3_data["findings"].as_array().expect("findings");
    assert!(
        !findings3.is_empty(),
        "Web audit must bypass cache and produce real findings, not empty seeded list"
    );
    assert!(
        finding_for_path(&report3_data, "sentinel/nw.dll").is_none(),
        "Web audit must not serve the seeded sentinel finding: {findings3:?}"
    );
    let audit_nw = finding_for_path(&report3_data, "bin/nw.dll").unwrap_or_else(|| {
        panic!("web audit must recompute the full-distribution outer native finding: {findings3:?}")
    });
    assert_eq!(audit_nw["code"], "CMP-106");

    // 4. Run build: the build GET path must also bypass the seeded entry and must not PUT.
    let build_dir = working_dir.join("build_out");
    let build_out = run_build(
        &working_dir,
        &[
            fixture_dir.to_str().unwrap(),
            "--output",
            build_dir.to_str().unwrap(),
            "--flavor",
            "beta",
            "--accept-audit-fail",
            "--accept-audit-unknown",
            "--no-compile",
            "--no-html",
        ],
    );
    assert!(
        build_out.status.success(),
        "Build must succeed: STDOUT: {}\nSTDERR: {}",
        String::from_utf8_lossy(&build_out.stdout),
        String::from_utf8_lossy(&build_out.stderr)
    );
    let build_stdout = String::from_utf8_lossy(&build_out.stdout);
    assert!(
        !build_stdout.contains("[cache hit]"),
        "Web build must not read the audit cache: {build_stdout}"
    );

    let build_report = read_report(&build_dir.join("asset-audit-report.json"));
    assert_ne!(
        build_report["outcome"], "PASS",
        "Web build must not serve the seeded fake PASS report"
    );
    assert!(
        finding_for_path(&build_report, "sentinel/nw.dll").is_none(),
        "Web build must not serve the seeded sentinel finding: {:?}",
        build_report["findings"]
    );
    let build_nw = finding_for_path(&build_report, "bin/nw.dll").unwrap_or_else(|| {
        panic!(
            "web build must recompute the full-distribution outer native finding: {:?}",
            build_report["findings"]
        )
    });
    assert_eq!(build_nw["code"], "CMP-106");
    assert_eq!(build_nw["severity"], "INFO");
    assert_eq!(build_nw["disposition"], "INFORMATIONAL");

    // The seeded artifact itself is untouched (no serve, no overwrite, no delete).
    assert_eq!(
        fs::read(&seeded_path).unwrap(),
        seeded_bytes_before,
        "web build/audit must leave the seeded cache entry byte-identical"
    );

    // Verify cache directory still contains only the seeded fake entry (no new entry written by build)
    let cache_entries: Vec<_> = fs::read_dir(&default_cache_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(
        cache_entries,
        vec![seeded_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string()],
        "Build must NOT write a new cache entry for web: {cache_entries:?}"
    );

    fs::remove_dir_all(working_dir).unwrap();
}

#[test]
fn test_cli_outer_node_addon_remains_hard_blocker_in_audit_and_build() {
    let working_dir = temp_dir("cli-outer-node");
    let fixture_dir = working_dir.join("game_source");
    fs::create_dir_all(&fixture_dir).unwrap();

    setup_nested_mz_wrapper_fixture(&fixture_dir, "www/index.html");
    // Add outer native extensions in the wrapper directory: `.node` and `.so`
    fs::write(fixture_dir.join("bin/addon.node"), b"fake native node addon").unwrap();
    fs::write(fixture_dir.join("bin/native.so"), b"\x7fELFfake").unwrap();

    // Audit: report must contain CMP-103 for bin/addon.node with BLOCKING disposition
    let report_path = working_dir.join("audit-report.json");
    let audit_out = run_audit(
        &working_dir,
        &[
            fixture_dir.to_str().unwrap(),
            "--no-html",
            "--output",
            report_path.to_str().unwrap(),
        ],
    );
    assert!(audit_out.status.success());
    let report_data = read_report(&report_path);
    let findings = report_data["findings"].as_array().expect("findings array");
    let node_finding = findings.iter().find(|f| {
        f["source_node_ids"]
            .as_array()
            .map(|nodes| nodes.iter().any(|n| n.as_str() == Some("bin/addon.node")))
            .unwrap_or(false)
    });
    assert!(
        node_finding.is_some(),
        "Audit report must report outer bin/addon.node: {findings:?}"
    );
    let node = node_finding.unwrap();
    assert_eq!(node["code"], "CMP-103");
    assert_eq!(node["severity"], "ERROR");
    assert_eq!(node["disposition"], "BLOCKING");

    let so_finding = findings.iter().find(|f| {
        f["source_node_ids"]
            .as_array()
            .map(|nodes| nodes.iter().any(|n| n.as_str() == Some("bin/native.so")))
            .unwrap_or(false)
    });
    assert!(
        so_finding.is_some(),
        "Audit report must report outer bin/native.so: {findings:?}"
    );
    let so = so_finding.unwrap();
    assert_eq!(so["code"], "CMP-102");
    assert_eq!(so["severity"], "ERROR");
    assert_eq!(so["disposition"], "BLOCKING");

    // Build: must fail closed because CMP-103 is a hard compatibility blocker
    let build_dir = working_dir.join("build_out");
    let build_out = run_build(
        &working_dir,
        &[
            fixture_dir.to_str().unwrap(),
            "--output",
            build_dir.to_str().unwrap(),
            "--flavor",
            "beta",
            "--accept-audit-fail",
            "--accept-audit-unknown",
            "--no-compile",
            "--no-html",
        ],
    );
    assert!(
        !build_out.status.success(),
        "Build must fail closed on outer .node addon"
    );
    let build_stderr = String::from_utf8_lossy(&build_out.stderr);
    assert!(
        build_stderr.contains("CMP-103"),
        "Build stderr must cite CMP-103: {build_stderr}"
    );
    assert!(
        build_stderr.contains("CMP-102"),
        "Build stderr must cite CMP-102 for the outer .so: {build_stderr}"
    );

    fs::remove_dir_all(working_dir).unwrap();
}

#[test]
fn test_cli_build_staging_uses_staged_bytes_in_auditor_not_original() {
    let working_dir = temp_dir("cli-staging-audit");
    let fixture_dir = working_dir.join("game_source");
    let out_fail = working_dir.join("out_fail");
    let out_ok = working_dir.join("out_ok");
    fs::create_dir_all(&fixture_dir).unwrap();

    setup_nested_mz_wrapper_fixture(&fixture_dir, "www/index.html");

    let payload_dir = fixture_dir.join("bin/www");
    fs::create_dir_all(payload_dir.join("js/plugins")).unwrap();
    fs::write(
        payload_dir.join("js/plugins.js"),
        "var $plugins = [{\"name\":\"TestDesktop\",\"status\":true,\"description\":\"\",\"parameters\":{}}];",
    )
    .unwrap();

    let orig_plugin = b"function openDir() {\n\tconst cp = require('child_process');\n\tcp.exec('open');\n}\n";
    fs::write(payload_dir.join("js/plugins/TestDesktop.js"), orig_plugin).unwrap();

    // 1. Without profile: build must fail closed on CMP-104 (child_process). The
    // packaging compatibility policy is pinned to `block` so this assertion still
    // measures the staging profile alone; the CLI's default `warn` behaviour is
    // covered by its own tests in `build_test.rs`.
    let out_fail_cmd = run_build(
        &working_dir,
        &[
            fixture_dir.to_str().unwrap(),
            "--output",
            out_fail.to_str().unwrap(),
            "--flavor",
            "beta",
            "--compatibility-policy",
            "block",
            "--accept-audit-fail",
            "--accept-audit-unknown",
            "--no-compile",
            "--no-html",
        ],
    );
    assert!(
        !out_fail_cmd.status.success(),
        "Build without compat profile must fail closed on child_process"
    );
    let stderr_fail = String::from_utf8_lossy(&out_fail_cmd.stderr);
    assert!(
        stderr_fail.contains("CMP-104") || stderr_fail.contains("child_process"),
        "Failure must cite CMP-104 or child_process: {stderr_fail}"
    );

    // 2. Authored compat profile that rewrites the staged copy only
    let orig_sha = format!("{:x}", Sha256::digest(orig_plugin));
    let frag_start = 20; // after "function openDir() {"
    let frag_end = 73; // before "}\n"
    let fragment = &orig_plugin[frag_start..frag_end];
    let frag_sha = format!("{:x}", Sha256::digest(fragment));

    let replacement = "\n\tconsole.error('[RT-DESKTOP-DISABLED]');\n\tthrow new Error('disabled');\n";
    let mut post_bytes = Vec::new();
    post_bytes.extend_from_slice(&orig_plugin[..frag_start]);
    post_bytes.extend_from_slice(replacement.as_bytes());
    post_bytes.extend_from_slice(&orig_plugin[frag_end..]);
    let post_sha = format!("{:x}", Sha256::digest(&post_bytes));

    let profile_json = serde_json::json!({
        "schema_version": 1,
        "profile_id": "test-staging-audit-profile",
        "engine_id": "rpgmaker-mz",
        "capability_id": "CAP-WEB-028",
        "status": "DEGRADED",
        "diagnostic_code": "CMP-107",
        "description": "Disable child_process in TestDesktop",
        "patches": [
            {
                "target_anchor": "payload",
                "logical_path": "js/plugins/TestDesktop.js",
                "expected_file_size": orig_plugin.len(),
                "expected_file_sha256": orig_sha,
                "range": { "start": frag_start, "end": frag_end },
                "expected_fragment_sha256": frag_sha,
                "replacement": replacement,
                "result_file_sha256": post_sha
            }
        ]
    });
    let profile_path = working_dir.join("compat_profile.json");
    fs::write(
        &profile_path,
        serde_json::to_string_pretty(&profile_json).unwrap(),
    )
    .unwrap();

    // 3. With profile: build succeeds and the auditor consumes staged bytes
    let out_ok_cmd = run_build(
        &working_dir,
        &[
            fixture_dir.to_str().unwrap(),
            "--output",
            out_ok.to_str().unwrap(),
            "--flavor",
            "beta",
            "--compat-profile",
            profile_path.to_str().unwrap(),
            "--accept-audit-fail",
            "--accept-audit-unknown",
            "--no-compile",
            "--no-html",
        ],
    );
    assert!(
        out_ok_cmd.status.success(),
        "Build with profile must succeed: STDOUT: {}\nSTDERR: {}",
        String::from_utf8_lossy(&out_ok_cmd.stdout),
        String::from_utf8_lossy(&out_ok_cmd.stderr)
    );

    let staged_file = out_ok.join("staging/bin/www/js/plugins/TestDesktop.js");
    assert!(staged_file.exists(), "Staged file must exist");
    let staged_content = fs::read_to_string(&staged_file).unwrap();
    assert!(staged_content.contains("[RT-DESKTOP-DISABLED]"));
    assert!(!staged_content.contains("child_process"));

    // H3: the emitted build audit report must reflect the STAGED script bytes. The
    // legacy accept flags let the original CMP-104 through, so exit status alone
    // proves nothing; assert the absence of the path-specific native requirement.
    let staged_build_report = read_report(&out_ok.join("asset-audit-report.json"));
    let staged_findings = staged_build_report["findings"]
        .as_array()
        .expect("staged build findings");
    assert!(
        staged_findings
            .iter()
            .all(|finding| finding["code"] != "CMP-104"),
        "staged build audit must not emit CMP-104 for the patched plugin: {staged_findings:?}"
    );
    assert!(
        staged_findings.iter().all(|finding| {
            !finding["message"]
                .as_str()
                .map(|message| message.contains("child_process"))
                .unwrap_or(false)
        }),
        "staged build audit must not retain the original child_process requirement: {staged_findings:?}"
    );
    assert!(
        staged_findings.iter().all(|finding| {
            !finding["source_node_ids"]
                .as_array()
                .map(|nodes| {
                    nodes
                        .iter()
                        .any(|node| node.as_str() == Some("js/plugins/TestDesktop.js"))
                })
                .unwrap_or(false)
        }),
        "staged build audit must not carry a hard finding for the patched plugin: {staged_findings:?}"
    );
    // Distinguish the input views: the staged plugin is still an indexed asset node,
    // so an absent finding is staged evidence, not a missing-file shortcut.
    assert!(
        graph_paths(&staged_build_report)
            .iter()
            .any(|path| path == "js/plugins/TestDesktop.js"),
        "staged build audit graph must index the patched plugin: {:?}",
        graph_paths(&staged_build_report)
    );
    assert_payload_only_graph(&staged_build_report, "staged build");

    // Counterfactual input: the ORIGINAL source (no profile) does carry CMP-104 for
    // the same path, so the staged-report absence above is input-sensitive.
    let original_report_path = working_dir.join("original-audit-report.json");
    let original_audit = run_audit(
        &working_dir,
        &[
            fixture_dir.to_str().unwrap(),
            "--no-html",
            "--output",
            original_report_path.to_str().unwrap(),
        ],
    );
    assert!(
        original_audit.status.success(),
        "original-source audit must run: {}",
        String::from_utf8_lossy(&original_audit.stderr)
    );
    let original_report = read_report(&original_report_path);
    let original_cmp104 = finding_for_path(&original_report, "js/plugins/TestDesktop.js")
        .unwrap_or_else(|| {
            panic!(
                "original source must still require the native module for the plugin: {:?}",
                original_report["findings"]
            )
        });
    assert_eq!(
        original_cmp104["code"], "CMP-104",
        "original source must emit CMP-104 for the desktop plugin"
    );
    assert_eq!(original_cmp104["severity"], "ERROR");
    assert_eq!(original_cmp104["disposition"], "BLOCKING");

    // The immutable original source keeps the desktop proof; only the staged copy changed.
    let src_after = fs::read(payload_dir.join("js/plugins/TestDesktop.js")).unwrap();
    assert_eq!(src_after, orig_plugin);

    fs::remove_dir_all(working_dir).unwrap();
}

#[test]
fn test_cli_inspect_adversarial_issues_are_sanitized_without_mutating_report() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scratch_base = workspace_root()
        .join("_temp/web-portability-20261109/test-scratch")
        .join(format!("inspect_adv_{}_{}", std::process::id(), nonce));
    let working_dir = scratch_base.join("work");
    let fixture_dir = scratch_base.join("game");
    fs::create_dir_all(&working_dir).unwrap();
    fs::create_dir_all(fixture_dir.join("data")).unwrap();
    fs::create_dir_all(fixture_dir.join("js/plugins")).unwrap();

    fs::write(
        fixture_dir.join("data/System.json"),
        r#"{"gameTitle":"Adversarial Game","sounds":[]}"#,
    )
    .unwrap();
    fs::write(
        fixture_dir.join("index.html"),
        "<!DOCTYPE html><html><head>\
         <script src=\"js/rmmz_core.js\"></script>\
         <script src=\"js/plugins/AdvPlugin.js\"></script>\
         </head><body></body></html>",
    )
    .unwrap();
    fs::write(fixture_dir.join("js/rmmz_core.js"), "// rmmz_core").unwrap();

    // Adversarial literal require: escaped in JS source, decoded by lexer into raw \n, ESC (\u{1b}), U+2028, and U+202E
    fs::write(
        fixture_dir.join("js/plugins/AdvPlugin.js"),
        "require(\"./evil\\n\\u001b[31m\\u2028\\u202e.node\");\n",
    )
    .unwrap();

    let inspect_out = run_inspect(&working_dir, &[fixture_dir.to_str().unwrap()]);
    assert!(
        inspect_out.status.success(),
        "inspect must succeed: {}",
        String::from_utf8_lossy(&inspect_out.stderr)
    );
    let inspect_stdout = String::from_utf8_lossy(&inspect_out.stdout);

    // Terminal output must NOT contain raw control sequences
    assert!(
        !inspect_stdout.contains('\u{1b}'),
        "inspect output must not contain raw ESC: {inspect_stdout}"
    );
    assert!(
        !inspect_stdout.contains('\u{2028}'),
        "inspect output must not contain raw U+2028: {inspect_stdout}"
    );
    assert!(
        !inspect_stdout.contains('\u{202e}'),
        "inspect output must not contain raw U+202e: {inspect_stdout}"
    );
    // Control sequences must be visibly rendered:
    assert!(
        inspect_stdout.contains("\\n")
            && inspect_stdout.contains("\\u{1b}")
            && inspect_stdout.contains("\\u{2028}")
            && inspect_stdout.contains("\\u{202e}"),
        "inspect output must visibly render escaped control sequences: {inspect_stdout}"
    );

    // Verify authoritative report is NOT mutated:
    let source_index = GameSourceIndex::build(&fixture_dir).unwrap();
    let detectors = runtimepack_cli::detection_registry::default_detector_registry();
    let descriptor = detectors.resolve(&source_index);
    use runtimepack_core::compatibility::Analyzer;
    use runtimepack_engine_rpgmaker_web::RpgMakerWebAnalyzer;
    let report = RpgMakerWebAnalyzer.analyze(&source_index, &descriptor);

    let issue = report
        .issues
        .iter()
        .find(|i| i.message.contains("evil"))
        .expect("Report must contain issue for evil node");
    assert!(
        issue.message.contains('\n')
            && issue.message.contains('\u{1b}')
            && issue.message.contains('\u{2028}')
            && issue.message.contains('\u{202e}'),
        "Authoritative report message must NOT be mutated: {}",
        issue.message
    );
}
