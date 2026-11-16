use runtimepack_core::detection::DetectorRegistry;
use runtimepack_core::domain::{DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_engine_renpy::RenPyDetector;
use runtimepack_engine_rgss::{RpgMakerVxAceDetector, RpgMakerVxDetector, RpgMakerXpDetector};
use runtimepack_engine_rpgmaker_web::{RpgMakerMvDetector, RpgMakerMzDetector};
use runtimepack_engine_tyrano::TyranoDetector;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn create_temp_game(prefix: &str) -> PathBuf {
    let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "runtimepack_det_{}_{}_{}",
        prefix,
        std::process::id(),
        id
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(base: &Path, rel: &str, content: &str) {
    let path = base.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, content).unwrap();
}

fn setup_registry() -> DetectorRegistry {
    let mut reg = DetectorRegistry::new();
    reg.register(Box::new(RpgMakerXpDetector));
    reg.register(Box::new(RpgMakerVxDetector));
    reg.register(Box::new(RpgMakerVxAceDetector));
    reg.register(Box::new(RpgMakerMvDetector));
    reg.register(Box::new(RpgMakerMzDetector));
    reg.register(Box::new(RenPyDetector));
    reg.register(Box::new(TyranoDetector));
    reg
}

fn fixture(name: &str) -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    root.join("tests").join("fixtures").join(name)
}

#[test]
fn test_det_xp_001() {
    let reg = setup_registry();
    let idx = GameSourceIndex::build(&fixture("xp_min")).unwrap();
    let desc = reg.resolve(&idx);
    assert_eq!(desc.id, EngineId::RpgMakerXp);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
}

#[test]
fn test_det_vx_001() {
    let reg = setup_registry();
    let idx = GameSourceIndex::build(&fixture("vx_min")).unwrap();
    let desc = reg.resolve(&idx);
    assert_eq!(desc.id, EngineId::RpgMakerVx);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
}

#[test]
fn test_det_vxace_001() {
    let reg = setup_registry();
    let idx = GameSourceIndex::build(&fixture("vxace_min")).unwrap();
    let desc = reg.resolve(&idx);
    assert_eq!(desc.id, EngineId::RpgMakerVxAce);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
}

#[test]
fn test_det_mv_001() {
    let reg = setup_registry();
    let idx = GameSourceIndex::build(&fixture("mv_min")).unwrap();
    let desc = reg.resolve(&idx);
    assert_eq!(desc.id, EngineId::RpgMakerMv);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
}

#[test]
fn test_det_mz_001() {
    let reg = setup_registry();
    let idx = GameSourceIndex::build(&fixture("mz_min")).unwrap();
    let desc = reg.resolve(&idx);
    assert_eq!(desc.id, EngineId::RpgMakerMz);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
}

#[test]
fn test_det_renpy_001() {
    let reg = setup_registry();
    let idx = GameSourceIndex::build(&fixture("renpy_supported_version_A")).unwrap();
    let desc = reg.resolve(&idx);
    assert_eq!(desc.id, EngineId::RenPy);
    assert!(desc.confidence >= DetectionConfidence::Medium);
}

#[test]
fn test_det_tyrano_001() {
    let reg = setup_registry();
    let idx = GameSourceIndex::build(&fixture("tyrano_min")).unwrap();
    let desc = reg.resolve(&idx);
    assert_eq!(desc.id, EngineId::Tyrano);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
}

#[test]
fn test_det_generic_html_negative() {
    let reg = setup_registry();
    let idx = GameSourceIndex::build(&fixture("generic_html_negative")).unwrap();
    let desc = reg.resolve(&idx);
    assert_eq!(desc.id, EngineId::Unknown);
    assert_eq!(desc.confidence, DetectionConfidence::None);
}

#[test]
fn test_det_ambiguous_mv_mz() {
    let reg = setup_registry();
    let idx = GameSourceIndex::build(&fixture("ambiguous_mv_mz")).unwrap();
    let desc = reg.resolve(&idx);
    assert_eq!(desc.id, EngineId::Unknown);
    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
}

#[test]
fn test_det_mz_nested_arbitrary() {
    let dir = create_temp_game("mz_nested");
    write_file(&dir, "bin/nw.exe", "MZ");
    write_file(&dir, "bin/www/index.html", "<html></html>");
    write_file(&dir, "bin/www/js/rmmz_core.js", "// core");
    write_file(&dir, "bin/www/js/rmmz_managers.js", "// managers");
    write_file(&dir, "bin/www/data/System.json", "{}");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMz);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert_eq!(desc.score, 210);
    assert_eq!(desc.content_root, Some("bin/www".to_string()));
}

#[test]
fn test_det_split_marker_isolation() {
    let dir = create_temp_game("split_isolation");
    write_file(&dir, "index.html", "<html></html>");
    write_file(&dir, "bin/www/js/rmmz_core.js", "// core");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();

    let cands = reg.detect_all(&idx);
    let mz_cands: Vec<_> = cands
        .into_iter()
        .filter(|c| c.engine == EngineId::RpgMakerMz)
        .collect();

    let root_cand = mz_cands
        .iter()
        .find(|c| c.content_root.is_none())
        .expect("root candidate must exist");
    assert_eq!(root_cand.score, 0);
    assert_eq!(root_cand.confidence, DetectionConfidence::None);

    let inner_cand = mz_cands
        .iter()
        .find(|c| c.content_root.as_deref() == Some("bin/www"))
        .expect("bin/www candidate must exist");
    assert_eq!(inner_cand.score, 100);
    assert_eq!(inner_cand.confidence, DetectionConfidence::Medium);

    let desc = reg.resolve(&idx);
    assert_eq!(desc.id, EngineId::RpgMakerMz);
    assert_eq!(desc.confidence, DetectionConfidence::Medium);
    assert_eq!(desc.score, 100);
    assert_eq!(desc.content_root, Some("bin/www".to_string()));
}

#[test]
fn test_det_competing_identical_mz_ambiguous() {
    let dir = create_temp_game("competing_mz");
    for root in &["payload_a/www", "payload_b/www"] {
        write_file(&dir, &format!("{}/index.html", root), "<html></html>");
        write_file(&dir, &format!("{}/js/rmmz_core.js", root), "// core");
        write_file(&dir, &format!("{}/js/rmmz_managers.js", root), "// managers");
        write_file(&dir, &format!("{}/data/System.json", root), "{}");
    }

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::Unknown);
    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
    assert!(
        desc.conflicts.iter().any(|c| c.contains("DET-110")),
        "conflicts must contain DET-110, got: {:?}",
        desc.conflicts
    );
}

#[test]
fn test_det_forced_engine_competing_same_engine() {
    let dir = create_temp_game("forced_same_mz");
    for root in &["payload_a/www", "payload_b/www"] {
        write_file(&dir, &format!("{}/index.html", root), "<html></html>");
        write_file(&dir, &format!("{}/js/rmmz_core.js", root), "// core");
        write_file(&dir, &format!("{}/js/rmmz_managers.js", root), "// managers");
        write_file(&dir, &format!("{}/data/System.json", root), "{}");
    }

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve_with_engine(&idx, EngineId::RpgMakerMz);

    assert_eq!(desc.id, EngineId::Unknown);
    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
    assert!(
        desc.conflicts.iter().any(|c| c.contains("DET-110")),
        "conflicts must contain DET-110, got: {:?}",
        desc.conflicts
    );
}

#[test]
fn test_det_mixed_engine_resolution() {
    let dir = create_temp_game("mixed_mv_mz");
    // Root MV
    write_file(&dir, "index.html", "<html></html>");
    write_file(&dir, "js/rpg_core.js", "// core");
    write_file(&dir, "js/rpg_managers.js", "// managers");
    write_file(&dir, "data/System.json", "{}");
    // Nested MZ
    write_file(&dir, "bin/www/index.html", "<html></html>");
    write_file(&dir, "bin/www/js/rmmz_core.js", "// core");
    write_file(&dir, "bin/www/js/rmmz_managers.js", "// managers");
    write_file(&dir, "bin/www/data/System.json", "{}");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();

    // Without --engine: ambiguous
    let desc_unforced = reg.resolve(&idx);
    assert_eq!(desc_unforced.id, EngineId::Unknown);
    assert_eq!(desc_unforced.confidence, DetectionConfidence::Ambiguous);

    // With --engine rpgmaker-mz: disambiguated
    let desc_mz = reg.resolve_with_engine(&idx, EngineId::RpgMakerMz);
    assert_eq!(desc_mz.id, EngineId::RpgMakerMz);
    assert_eq!(desc_mz.confidence, DetectionConfidence::Exact);
    assert_eq!(desc_mz.score, 210);
    assert_eq!(desc_mz.content_root, Some("bin/www".to_string()));
    assert!(desc_mz.overridden);
}

#[test]
fn test_det_intra_root_conflict() {
    let dir = create_temp_game("intra_root_conflict");
    write_file(&dir, "bin/www/index.html", "<html></html>");
    write_file(&dir, "bin/www/js/rpg_core.js", "// MV core");
    write_file(&dir, "bin/www/js/rmmz_core.js", "// MZ core");
    write_file(&dir, "bin/www/data/System.json", "{}");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::Unknown);
    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
    assert!(
        desc.conflicts.iter().any(|c| c.contains("DET-110") || c.contains("core")),
        "conflicts must cite conflict, got: {:?}",
        desc.conflicts
    );
}

#[test]
fn test_det_multiple_medium_roots_accept_detection_fails() {
    let dir = create_temp_game("multiple_medium");
    write_file(&dir, "root_a/js/rmmz_core.js", "// core only");
    write_file(&dir, "root_b/js/rmmz_core.js", "// core only");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    use runtimepack_core::detection::DetectionOptions;
    let opts = DetectionOptions {
        forced_engine: None,
        accept_detection: true,
    };
    let desc = reg.resolve_with_options(&idx, &opts);

    assert_eq!(desc.id, EngineId::Unknown);
    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
    assert!(
        desc.conflicts.iter().any(|c| c.contains("DET-110")),
        "conflicts must contain DET-110, got: {:?}",
        desc.conflicts
    );
}

#[test]
fn test_det_arbitrary_unicode_and_case_root() {
    let dir = create_temp_game("unicode_mz");
    write_file(&dir, "ゲーム/Data_MZ/index.html", "<html></html>");
    write_file(&dir, "ゲーム/Data_MZ/js/rmmz_core.js", "// core");
    write_file(&dir, "ゲーム/Data_MZ/js/rmmz_managers.js", "// managers");
    write_file(&dir, "ゲーム/Data_MZ/data/System.json", "{}");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMz);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert_eq!(desc.score, 210);
    assert_eq!(desc.content_root, Some("ゲーム/Data_MZ".to_string()));
}

#[test]
fn test_det_mv_nested_arbitrary() {
    let dir = create_temp_game("mv_nested");
    write_file(&dir, "app/package/index.html", "<html></html>");
    write_file(&dir, "app/package/js/rpg_core.js", "// core");
    write_file(&dir, "app/package/js/rpg_managers.js", "// managers");
    write_file(&dir, "app/package/data/System.json", "{}");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMv);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert_eq!(desc.score, 210);
    assert_eq!(desc.content_root, Some("app/package".to_string()));
}

#[test]
fn test_det_mz_standard_www() {
    let dir = create_temp_game("mz_www");
    write_file(&dir, "www/index.html", "<html></html>");
    write_file(&dir, "www/js/rmmz_core.js", "// core");
    write_file(&dir, "www/js/rmmz_managers.js", "// managers");
    write_file(&dir, "www/data/System.json", "{}");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMz);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert_eq!(desc.score, 210);
    assert_eq!(desc.content_root, Some("www".to_string()));
}

#[test]
fn test_det_marker_discovery_order_determinism() {
    let dir = create_temp_game("order_det");
    // Write out of order: z_root, a_root, m_root
    write_file(&dir, "z_root/js/rmmz_core.js", "// core");
    write_file(&dir, "a_root/js/rmmz_core.js", "// core");
    write_file(&dir, "m_root/js/rmmz_core.js", "// core");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let candidates = reg.detect_all(&idx);
    let mz_roots: Vec<Option<String>> = candidates
        .into_iter()
        .filter(|c| c.engine == EngineId::RpgMakerMz)
        .map(|c| c.content_root)
        .collect();

    assert_eq!(
        mz_roots,
        vec![
            None,
            Some("a_root".to_string()),
            Some("m_root".to_string()),
            Some("z_root".to_string())
        ]
    );
}

#[test]
fn test_cli_inspect_invalid_engine_override_fails_det101() {
    let dir = create_temp_game("cli_inspect_invalid_eng");
    write_file(&dir, "index.html", "<html></html>");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("inspect")
        .arg(&dir)
        .arg("--engine")
        .arg("totally_nonexistent_engine")
        .output()
        .expect("failed to execute runtimepack inspect");

    assert!(!output.status.success(), "expected inspect to fail with invalid engine");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[DET-101]"),
        "expected stderr to contain [DET-101], got: {}",
        stderr
    );
}

#[test]
fn test_cli_explain_invalid_engine_override_fails_det101() {
    let dir = create_temp_game("cli_explain_invalid_eng");
    write_file(&dir, "index.html", "<html></html>");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("explain")
        .arg(&dir)
        .arg("--engine")
        .arg("totally_nonexistent_engine")
        .output()
        .expect("failed to execute runtimepack explain");

    assert!(!output.status.success(), "expected explain to fail with invalid engine");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[DET-101]"),
        "expected stderr to contain [DET-101], got: {}",
        stderr
    );
}

#[test]
fn test_cli_build_invalid_engine_override_fails_det101() {
    let dir = create_temp_game("cli_build_invalid_eng");
    write_file(&dir, "index.html", "<html></html>");
    let out = create_temp_game("cli_build_invalid_eng_out");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&dir)
        .arg("--output")
        .arg(&out)
        .arg("--engine")
        .arg("totally_nonexistent_engine")
        .output()
        .expect("failed to execute runtimepack build");

    assert!(!output.status.success(), "expected build to fail with invalid engine");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[DET-101]"),
        "expected stderr to contain [DET-101], got: {}",
        stderr
    );
}

#[test]
fn test_cli_build_medium_without_accept_detection_fails_early_det102() {
    let dir = create_temp_game("cli_build_medium_noaccept");
    // Medium confidence MZ fixture: core only (score 100)
    write_file(&dir, "bin/www/js/rmmz_core.js", "// core only");
    let out = create_temp_game("cli_build_medium_noaccept_out");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&dir)
        .arg("--output")
        .arg(&out)
        .output()
        .expect("failed to execute runtimepack build");

    assert!(!output.status.success(), "expected build without --accept-detection to fail on MEDIUM");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[DET-102]"),
        "expected stderr to contain [DET-102], got: {}",
        stderr
    );
    // Asserts early failure PRIOR to heavy pipeline (no Android project created)
    assert!(
        !out.join("android-project").exists() && !out.join("app").exists(),
        "build must fail early before generating android project"
    );
}

#[test]
fn test_cli_build_low_confidence_fails_even_with_accept_detection_det103() {
    let dir = create_temp_game("cli_build_low_fail");
    // Only Game.exe in root -> score 10 for XP/VX/VXAce detectors -> max score 10 (< 30) or low
    write_file(&dir, "Game.exe", "binary");
    let out = create_temp_game("cli_build_low_fail_out");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&dir)
        .arg("--output")
        .arg(&out)
        .arg("--accept-detection")
        .output()
        .expect("failed to execute runtimepack build");

    assert!(!output.status.success(), "expected build to fail on LOW / NONE confidence");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[DET-100]") || stderr.contains("[DET-103]"),
        "expected stderr to contain [DET-100] or [DET-103], got: {}",
        stderr
    );
}

#[test]
fn test_cli_build_ambiguous_fails_even_with_accept_detection_det110() {
    let dir = create_temp_game("cli_build_ambiguous_fail");
    for root in &["payload_a/www", "payload_b/www"] {
        write_file(&dir, &format!("{}/index.html", root), "<html></html>");
        write_file(&dir, &format!("{}/js/rmmz_core.js", root), "// core");
        write_file(&dir, &format!("{}/js/rmmz_managers.js", root), "// managers");
        write_file(&dir, &format!("{}/data/System.json", root), "{}");
    }
    let out = create_temp_game("cli_build_ambiguous_fail_out");

    let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .arg("build")
        .arg(&dir)
        .arg("--output")
        .arg(&out)
        .arg("--accept-detection")
        .output()
        .expect("failed to execute runtimepack build");

    assert!(!output.status.success(), "expected build to fail on AMBIGUOUS even with --accept-detection");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[DET-110]"),
        "expected stderr to contain [DET-110], got: {}",
        stderr
    );
}

#[test]
fn test_policy_detection_build_gate_unit() {
    use runtimepack_core::detection::can_proceed_to_build;

    assert!(can_proceed_to_build(DetectionConfidence::Exact, false).is_ok());
    assert!(can_proceed_to_build(DetectionConfidence::High, false).is_ok());
    assert_eq!(
        can_proceed_to_build(DetectionConfidence::Medium, false),
        Err("DET-102")
    );
    assert!(can_proceed_to_build(DetectionConfidence::Medium, true).is_ok());
    assert_eq!(
        can_proceed_to_build(DetectionConfidence::Low, false),
        Err("DET-103")
    );
    assert_eq!(
        can_proceed_to_build(DetectionConfidence::Low, true),
        Err("DET-103")
    );
    assert_eq!(
        can_proceed_to_build(DetectionConfidence::None, true),
        Err("DET-103")
    );
    assert_eq!(
        can_proceed_to_build(DetectionConfidence::Ambiguous, true),
        Err("DET-110")
    );
}

fn build_test_asar(asar_path: &Path, files: &[(&str, &[u8])]) {
    use std::io::Write;
    if let Some(parent) = asar_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut root_files = serde_json::Map::new();
    let mut offset = 0u64;

    for (path, content) in files {
        let parts: Vec<&str> = path.split('/').collect();
        let mut curr_map = &mut root_files;
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                let mut leaf = serde_json::Map::new();
                leaf.insert(
                    "size".to_string(),
                    serde_json::Value::Number(content.len().into()),
                );
                leaf.insert(
                    "offset".to_string(),
                    serde_json::Value::String(offset.to_string()),
                );
                curr_map.insert(part.to_string(), serde_json::Value::Object(leaf));
                offset += content.len() as u64;
            } else {
                if !curr_map.contains_key(*part) {
                    let mut dir_obj = serde_json::Map::new();
                    dir_obj.insert(
                        "files".to_string(),
                        serde_json::Value::Object(serde_json::Map::new()),
                    );
                    curr_map.insert(part.to_string(), serde_json::Value::Object(dir_obj));
                }
                let next_obj = curr_map.get_mut(*part).unwrap().as_object_mut().unwrap();
                curr_map = next_obj.get_mut("files").unwrap().as_object_mut().unwrap();
            }
        }
    }

    let mut root = serde_json::Map::new();
    root.insert("files".to_string(), serde_json::Value::Object(root_files));
    let json_bytes = serde_json::to_vec(&serde_json::Value::Object(root)).unwrap();
    let json_len = json_bytes.len() as u32;

    let magic = 4u32;
    let header_size = json_len + 8;
    let header_size_minus_4 = json_len + 4;

    let mut out = fs::File::create(asar_path).unwrap();
    out.write_all(&magic.to_le_bytes()).unwrap();
    out.write_all(&header_size.to_le_bytes()).unwrap();
    out.write_all(&header_size_minus_4.to_le_bytes()).unwrap();
    out.write_all(&json_len.to_le_bytes()).unwrap();
    out.write_all(&json_bytes).unwrap();

    for (_, content) in files {
        out.write_all(content).unwrap();
    }
}

#[test]
fn test_det_tyrano_nested_arbitrary() {
    let dir = create_temp_game("tyrano_nested");
    write_file(&dir, "bin/app/index.html", "<html></html>");
    write_file(&dir, "bin/app/tyrano/tyrano.js", "// tyrano core");
    write_file(&dir, "bin/app/tyrano/plugins/kag/kag.js", "// kag");
    write_file(&dir, "bin/app/data/system/Config.tjs", "// config");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::Tyrano);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert_eq!(desc.score, 240);
    assert_eq!(desc.content_root, Some("bin/app".to_string()));
}

#[test]
fn test_det_tyrano_nested_base_js() {
    let dir = create_temp_game("tyrano_base_js");
    write_file(&dir, "wrapper/index.html", "<html></html>");
    write_file(&dir, "wrapper/tyrano/tyrano.base.js", "// tyrano base core");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::Tyrano);
    assert_eq!(desc.confidence, DetectionConfidence::High);
    assert_eq!(desc.score, 160);
    assert_eq!(desc.content_root, Some("wrapper".to_string()));
}

#[test]
fn test_det_tyrano_split_marker_isolation_unit() {
    let dir = create_temp_game("tyrano_split");
    write_file(&dir, "index.html", "<html></html>");
    write_file(&dir, "nested/tyrano/tyrano.js", "// tyrano");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let candidates = reg.detect_all(&idx);
    let tyrano_cands: Vec<_> = candidates.into_iter().filter(|c| c.engine == EngineId::Tyrano).collect();

    let root_cand = tyrano_cands.iter().find(|c| c.content_root.is_none()).expect("root candidate");
    assert_eq!(root_cand.score, 0);

    let nested_cand = tyrano_cands.iter().find(|c| c.content_root == Some("nested".to_string())).expect("nested candidate");
    assert_eq!(nested_cand.score, 80);
    assert_eq!(nested_cand.confidence, DetectionConfidence::Medium);
}

#[test]
fn test_det_tyrano_competing_identical_ambiguous() {
    let dir = create_temp_game("tyrano_ambiguous");
    for root in &["payload_a", "payload_b"] {
        write_file(&dir, &format!("{}/index.html", root), "<html></html>");
        write_file(&dir, &format!("{}/tyrano/tyrano.js", root), "// core");
        write_file(&dir, &format!("{}/tyrano/plugins/kag/kag.js", root), "// kag");
        write_file(&dir, &format!("{}/data/system/Config.tjs", root), "// config");
    }

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::Unknown);
    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
    assert_eq!(desc.content_root, None);
}

#[test]
fn test_det_tyrano_nested_asar_discovery() {
    let dir = create_temp_game("tyrano_nested_asar");
    let asar_path = dir.join("package").join("resources").join("app.asar");
    build_test_asar(&asar_path, &[
        ("index.html", b"<html></html>"),
        ("tyrano/tyrano.js", b"// tyrano"),
        ("tyrano/plugins/kag/kag.js", b"// kag"),
        ("data/system/Config.tjs", b"// config"),
    ]);

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::Tyrano);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert_eq!(desc.score, 360);
    assert_eq!(desc.content_root, Some("package".to_string()));
}

#[test]
fn test_det_tyrano_generic_asar_rejected() {
    let dir = create_temp_game("tyrano_generic_asar");
    let asar_path = dir.join("resources").join("app.asar");
    build_test_asar(&asar_path, &[
        ("index.html", b"<html></html>"),
        ("main.js", b"console.log('electron app');"),
    ]);

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::Unknown);
    assert_eq!(desc.confidence, DetectionConfidence::None);
    assert_eq!(desc.score, 0);
}

#[test]
fn test_det_renpy_nested_distribution_root_exact() {
    let dir = create_temp_game("renpy_nested_exact");
    let root = "nested/release";
    write_file(&dir, &format!("{}/renpy/__init__.py", root), "# renpy engine");
    for i in 1..=5 {
        write_file(&dir, &format!("{}/game/script{}.rpy", root, i), "label start: return");
    }
    for i in 1..=10 {
        write_file(&dir, &format!("{}/game/script{}.rpyc", root, i), "compiled rpyc");
    }
    for i in 1..=3 {
        write_file(&dir, &format!("{}/game/archive{}.rpa", root, i), "RPA-3.0 archive");
    }

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RenPy);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert_eq!(desc.score, 250);
    assert_eq!(desc.content_root, Some("nested/release".to_string()));
}

#[test]
fn test_det_renpy_nested_distribution_root_minimal_medium() {
    let dir = create_temp_game("renpy_nested_minimal_medium");
    let root = "nested/release";
    write_file(&dir, &format!("{}/renpy/__init__.py", root), "# renpy engine");
    write_file(&dir, &format!("{}/game/script.rpy", root), "label start: return");
    write_file(&dir, &format!("{}/game/script.rpyc", root), "compiled rpyc");
    write_file(&dir, &format!("{}/game/archive.rpa", root), "RPA-3.0 archive");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RenPy);
    assert_eq!(desc.confidence, DetectionConfidence::Medium);
    assert_eq!(desc.score, 125);
    assert_eq!(desc.content_root, Some("nested/release".to_string()));
}

#[test]
fn test_det_renpy_nested_case_and_unicode() {
    let dir = create_temp_game("renpy_nested_case_unicode");
    let root = "Bundle Release 日本語";
    write_file(&dir, &format!("{}/RENPY/__init__.py", root), "# renpy");
    for i in 1..=5 {
        write_file(&dir, &format!("{}/GAME/script{}.rpy", root, i), "label start: return");
    }

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RenPy);
    assert_eq!(desc.confidence, DetectionConfidence::High);
    assert_eq!(desc.score, 140);
    assert_eq!(desc.content_root, Some("Bundle Release 日本語".to_string()));
}

#[test]
fn test_det_renpy_generic_game_dir_negative() {
    let dir = create_temp_game("renpy_generic_game_dir");
    write_file(&dir, "game/readme.txt", "just a text file");
    write_file(&dir, "game/audio.ogg", "not a renpy file");
    write_file(&dir, "game/data.bin", "binary");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::Unknown);
    assert_eq!(desc.confidence, DetectionConfidence::None);
    assert_eq!(desc.score, 0);
}

#[test]
fn test_det_renpy_competing_eligible_roots_ambiguous() {
    let dir = create_temp_game("renpy_competing_eligible");
    for root in &["dist_a", "dist_b"] {
        write_file(&dir, &format!("{}/renpy/__init__.py", root), "# renpy");
        for i in 1..=5 {
            write_file(&dir, &format!("{}/game/script{}.rpy", root, i), "label start: return");
        }
        for i in 1..=10 {
            write_file(&dir, &format!("{}/game/script{}.rpyc", root, i), "rpyc");
        }
        for i in 1..=3 {
            write_file(&dir, &format!("{}/game/archive{}.rpa", root, i), "rpa");
        }
    }

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
    assert_eq!(desc.content_root, None);
    assert!(
        desc.conflicts.iter().any(|c| c.contains("[DET-110]")),
        "conflicts must contain [DET-110], got: {:?}",
        desc.conflicts
    );
}

#[test]
fn test_det_renpy_competing_medium_roots_ambiguous() {
    let dir = create_temp_game("renpy_competing_medium");
    for root in &["dist_a", "dist_b"] {
        write_file(&dir, &format!("{}/renpy/__init__.py", root), "# renpy");
        write_file(&dir, &format!("{}/game/script.rpy", root), "label start: return");
    }

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
    assert_eq!(desc.content_root, None);
    assert!(
        desc.conflicts.iter().any(|c| c.contains("[DET-110]")),
        "conflicts must contain [DET-110], got: {:?}",
        desc.conflicts
    );
}

#[test]
fn test_det_renpy_split_markers_no_cross_merge() {
    let dir = create_temp_game("renpy_split_markers");
    write_file(&dir, "renpy/__init__.py", "# root renpy");
    write_file(&dir, "nested/game/script.rpy", "label start: return");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let candidates = reg.detect_all(&idx);
    let renpy_cands: Vec<_> = candidates.into_iter().filter(|c| c.engine == EngineId::RenPy).collect();

    let root_cand = renpy_cands.iter().find(|c| c.content_root.is_none()).expect("root candidate");
    assert_eq!(root_cand.score, 90);
    assert_eq!(root_cand.confidence, DetectionConfidence::Medium);

    let nested_cand = renpy_cands.iter().find(|c| c.content_root == Some("nested".to_string())).expect("nested candidate");
    assert_eq!(nested_cand.score, 10);
    assert_eq!(nested_cand.confidence, DetectionConfidence::None);
}

#[test]
fn test_det_rgss_xp_flat() {
    let dir = create_temp_game("rgss_xp_flat");
    write_file(&dir, "Data/Scripts.rxdata", "dummy scripts");
    write_file(
        &dir,
        "Game.ini",
        "[Game]\nLibrary=RGSS102E.dll\nScripts=Data\\Scripts.rxdata\nTitle=XP Flat\n",
    );
    write_file(&dir, "Game.exe", "exe");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerXp);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert_eq!(desc.score, 240);
    assert_eq!(desc.content_root, None);
}

#[test]
fn test_det_rgss_vx_flat() {
    let dir = create_temp_game("rgss_vx_flat");
    write_file(&dir, "Data/Scripts.rvdata", "dummy scripts");
    write_file(
        &dir,
        "Game.ini",
        "[Game]\nLibrary=RGSS202E.dll\nScripts=Data\\Scripts.rvdata\nTitle=VX Flat\n",
    );
    write_file(&dir, "Game.exe", "exe");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerVx);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert_eq!(desc.score, 240);
    assert_eq!(desc.content_root, None);
}

#[test]
fn test_det_rgss_vxace_flat() {
    let dir = create_temp_game("rgss_vxa_flat");
    write_file(&dir, "Data/Scripts.rvdata2", "dummy scripts");
    write_file(
        &dir,
        "Game.ini",
        "[Game]\nLibrary=System\\RGSS301.dll\nScripts=Data\\Scripts.rvdata2\nTitle=VX Ace Flat\n",
    );
    write_file(&dir, "Game.exe", "exe");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerVxAce);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert_eq!(desc.score, 240);
    assert_eq!(desc.content_root, None);
}

#[test]
fn test_det_rgss_xp_nested_unicode() {
    let dir = create_temp_game("rgss_xp_unicode");
    let root = "release_日本語/エピソード1";
    write_file(&dir, &format!("{}/Data/Scripts.rxdata", root), "dummy scripts");
    write_file(
        &dir,
        &format!("{}/Game.ini", root),
        "[Game]\nLibrary=RGSS104E.dll\nScripts=Data\\Scripts.rxdata\nTitle=XP Nested\n",
    );
    write_file(&dir, &format!("{}/Game.exe", root), "exe");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerXp);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert_eq!(desc.score, 240);
    assert_eq!(desc.content_root, Some(root.to_string()));
}

#[test]
fn test_det_rgss_vx_nested_case() {
    let dir = create_temp_game("rgss_vx_case");
    let root = "Wrapper/Game_VX";
    write_file(&dir, &format!("{}/Data/Scripts.rvdata", root), "dummy scripts");
    write_file(
        &dir,
        &format!("{}/Game.ini", root),
        "[Game]\nLibrary=RGSS200.dll\nScripts=Data\\Scripts.rvdata\nTitle=VX Nested\n",
    );
    write_file(&dir, &format!("{}/Game.exe", root), "exe");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerVx);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert_eq!(desc.score, 240);
    assert_eq!(desc.content_root, Some(root.to_string()));
}

#[test]
fn test_det_rgss_vxace_nested_arbitrary() {
    let dir = create_temp_game("rgss_vxa_nested");
    let root = "Bin/Payload";
    write_file(&dir, &format!("{}/Data/Scripts.rvdata2", root), "dummy scripts");
    write_file(
        &dir,
        &format!("{}/Game.ini", root),
        "[Game]\nLibrary=System\\RGSS301.dll\nScripts=Data\\Scripts.rvdata2\nTitle=VXA Nested\n",
    );
    write_file(&dir, &format!("{}/Game.exe", root), "exe");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerVxAce);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert_eq!(desc.score, 240);
    assert_eq!(desc.content_root, Some(root.to_string()));
}

#[test]
fn test_det_rgss_arbitrary_archive_basename() {
    let dir = create_temp_game("rgss_archive_basename");
    let archive_path = dir.join("nested/game/To The Moon.rgssad");
    if let Some(p) = archive_path.parent() {
        fs::create_dir_all(p).unwrap();
    }
    // Synthetic valid XP archive header (S47: magic RGSSAD\0, version byte 1)
    let mut header = b"RGSSAD\0\x01".to_vec();
    header.extend_from_slice(&[0u8; 16]); // dummy trailing
    fs::write(&archive_path, header).unwrap();

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerXp);
    assert_eq!(desc.confidence, DetectionConfidence::Medium);
    assert_eq!(desc.score, 120);
    assert_eq!(desc.content_root, Some("nested/game".to_string()));
}

#[test]
fn test_det_rgss_split_marker_isolation() {
    let dir = create_temp_game("rgss_split_marker");
    let root = "nested/game";
    write_file(&dir, &format!("{}/Data/Scripts.rvdata2", root), "dummy scripts");
    write_file(
        &dir,
        &format!("{}/Game.ini", root),
        "[Game]\nLibrary=System\\RGSS301.dll\nScripts=Data\\Scripts.rvdata2\nTitle=VXA Split\n",
    );
    // Outer Game.exe at root must NOT bleed into nested root
    write_file(&dir, "Game.exe", "outer exe");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let candidates = reg.detect_all(&idx);

    let vxa_cands: Vec<_> = candidates
        .into_iter()
        .filter(|c| c.engine == EngineId::RpgMakerVxAce)
        .collect();

    let root_cand = vxa_cands.iter().find(|c| c.content_root.is_none()).expect("root candidate");
    assert_eq!(root_cand.score, 10);
    assert_eq!(root_cand.confidence, DetectionConfidence::None);

    let nested_cand = vxa_cands
        .iter()
        .find(|c| c.content_root == Some(root.to_string()))
        .expect("nested candidate");
    assert_eq!(nested_cand.score, 230);
    assert_eq!(nested_cand.confidence, DetectionConfidence::Exact);
}

#[test]
fn test_det_rgss_same_engine_competing_roots_ambiguous() {
    let dir = create_temp_game("rgss_same_competing");
    for root in &["payload_a", "payload_b"] {
        write_file(&dir, &format!("{}/Data/Scripts.rxdata", root), "dummy scripts");
        write_file(
            &dir,
            &format!("{}/Game.ini", root),
            "[Game]\nLibrary=RGSS102E.dll\nScripts=Data\\Scripts.rxdata\nTitle=XP\n",
        );
    }

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
    assert_eq!(desc.content_root, None);
    assert!(
        desc.conflicts.iter().any(|c| c.contains("[DET-110]")),
        "conflicts must contain [DET-110], got: {:?}",
        desc.conflicts
    );

    // Forced engine override CANNOT bypass same-engine root ambiguity!
    let mut opts = runtimepack_core::detection::DetectionOptions::default();
    opts.forced_engine = Some(EngineId::RpgMakerXp);
    let forced_desc = reg.resolve_with_options(&idx, &opts);
    assert_eq!(forced_desc.confidence, DetectionConfidence::Ambiguous);
}

#[test]
fn test_det_rgss_mixed_engine_resolution() {
    let dir = create_temp_game("rgss_mixed_engine");
    write_file(&dir, "xp_root/Data/Scripts.rxdata", "dummy scripts");
    write_file(
        &dir,
        "xp_root/Game.ini",
        "[Game]\nLibrary=RGSS102E.dll\nScripts=Data\\Scripts.rxdata\nTitle=XP\n",
    );

    write_file(&dir, "vxa_root/Data/Scripts.rvdata2", "dummy scripts");
    write_file(
        &dir,
        "vxa_root/Game.ini",
        "[Game]\nLibrary=System\\RGSS301.dll\nScripts=Data\\Scripts.rvdata2\nTitle=VXA\n",
    );

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();

    // Unforced: fails ambiguous
    let desc = reg.resolve(&idx);
    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);

    // Forced --engine rpgmaker-vxace disambiguates to VX Ace at vxa_root
    let mut opts = runtimepack_core::detection::DetectionOptions::default();
    opts.forced_engine = Some(EngineId::RpgMakerVxAce);
    let resolved = reg.resolve_with_options(&idx, &opts);
    assert_eq!(resolved.id, EngineId::RpgMakerVxAce);
    assert_eq!(resolved.confidence, DetectionConfidence::Exact);
    assert_eq!(resolved.content_root, Some("vxa_root".to_string()));
}

#[test]
fn test_det_rgss_archive_header2_rejected() {
    let dir = create_temp_game("rgss_header2_rejected");
    let archive_path = dir.join("Game.rgssad");
    // Non-standard header byte 2
    let mut header = b"RGSSAD\0\x02".to_vec();
    header.extend_from_slice(&[0u8; 16]);
    fs::write(&archive_path, header).unwrap();

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    // Archive with header byte 2 receives 0 archive detection points!
    assert_eq!(desc.score, 0);
    assert_eq!(desc.confidence, DetectionConfidence::None);
}

#[test]
fn test_det_rgss_vx_archive_v1_accepted() {
    let dir = create_temp_game("rgss_vx_archive_v1");
    let archive_path = dir.join("Game.rgss2a");
    // Standard VX archive header (S47: magic RGSSAD\0, version byte 1 ALSO)
    let mut header = b"RGSSAD\0\x01".to_vec();
    header.extend_from_slice(&[0u8; 16]);
    fs::write(&archive_path, header).unwrap();

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let candidates = reg.detect_all(&idx);

    let vx_cand = candidates
        .into_iter()
        .find(|c| c.engine == EngineId::RpgMakerVx)
        .expect("VX candidate");
    assert_eq!(vx_cand.score, 120);
    assert_eq!(vx_cand.confidence, DetectionConfidence::Medium);
}

// =============================================================================
// D1 — MV/MZ live entrypoint arbitration (engine slice)
//
// A stale opposite-family core set must not force DET-110 when the recognised
// live entrypoint chain proves exactly one family. Only literal, actually
// consumed entrypoint evidence counts; comments, string literals, dynamic or
// unrecognised loader shapes and oversize evidence stay insufficient.
// =============================================================================

/// Literal `scriptUrls` array consumed by the recognised MZ/MV loader loop.
///
/// Mirrors the shipped MV/MZ `main.js` shape: the loop lives in the
/// `loadMainScripts()` class method, `run()` calls `this.loadMainScripts()`,
/// and the module ends with `const main = new Main(); main.run();`. Without that
/// call chain the method body is dead code and must not count as liveness.
fn loader_main_js(urls: &[&str]) -> String {
    let mut out = String::from(
        "//=============================================================================\n// main.js\n//=============================================================================\n\nconst scriptUrls = [\n",
    );
    for url in urls {
        out.push_str(&format!("    \"{url}\",\n"));
    }
    out.push_str(
        "];\n\nclass Main {\n    constructor() {\n        this.loadCount = 0;\n    }\n\n    run() {\n        this.showLoadingSpinner();\n        this.loadMainScripts();\n    }\n\n    showLoadingSpinner() {\n        const spinner = document.createElement(\"div\");\n        document.body.appendChild(spinner);\n    }\n\n    loadMainScripts() {\n        for (const url of scriptUrls) {\n            const script = document.createElement(\"script\");\n            script.type = \"text/javascript\";\n            script.src = url;\n            script.async = false;\n            document.body.appendChild(script);\n        }\n        this.numScripts = scriptUrls.length;\n    }\n}\n\nconst main = new Main();\nmain.run();\n",
    );
    out
}

fn index_with_main_script() -> String {
    "<!DOCTYPE html><html><head></head><body>\n<script type=\"text/javascript\" src=\"js/main.js\"></script>\n</body></html>\n"
        .to_string()
}

/// Writes the stale opposite-family core set plus `index.html`/`js/main.js`.
fn write_mv_mz_leftovers(dir: &Path, root_prefix: &str, main_js: &str, index_html: &str) {
    let at = |rel: &str| {
        if root_prefix.is_empty() {
            rel.to_string()
        } else {
            format!("{root_prefix}/{rel}")
        }
    };
    write_file(dir, &at("index.html"), index_html);
    write_file(dir, &at("js/main.js"), main_js);
    write_file(dir, &at("js/rpg_core.js"), "// stale MV core\n");
    write_file(dir, &at("js/rpg_managers.js"), "// stale MV managers\n");
    write_file(dir, &at("js/rmmz_core.js"), "// MZ core\n");
    write_file(dir, &at("js/rmmz_managers.js"), "// MZ managers\n");
    write_file(dir, &at("data/System.json"), "{\"gameTitle\":\"fixture\"}");
}

fn evidence_rules(desc: &runtimepack_core::detection::EngineDescriptor) -> Vec<String> {
    desc.evidence.iter().map(|e| e.rule.clone()).collect()
}

#[test]
fn test_det_mv_mz_live_chain_selects_mz_with_stale_mv_cores() {
    let dir = create_temp_game("d1_live_mz");
    write_mv_mz_leftovers(
        &dir,
        "",
        &loader_main_js(&["js/libs/pixi.js", "js/rmmz_core.js", "js/rmmz_managers.js", "js/plugins.js"]),
        &index_with_main_script(),
    );
    write_file(&dir, "package.json", "{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMz);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert_eq!(desc.score, 210);
    assert_eq!(desc.content_root, None);
    assert!(
        desc.conflicts.is_empty(),
        "stale MV cores must not remain a conflict, got: {:?}",
        desc.conflicts
    );

    let rules = evidence_rules(&desc);
    assert!(
        rules.iter().any(|r| r == "DET-ARB-MZ-MAIN"),
        "expected DET-ARB-MZ-MAIN liveness evidence, got: {rules:?}"
    );
    assert!(
        rules.iter().any(|r| r == "DET-ARB-EXCLUDED-CORE"),
        "expected zero-weight DET-ARB-EXCLUDED-CORE evidence for the stale MV core, got: {rules:?}"
    );
    assert!(
        rules.iter().any(|r| r == "DET-ARB-PKG-MZ"),
        "expected package.json corroboration only, got: {rules:?}"
    );
    let arbitration = desc
        .evidence
        .iter()
        .filter(|e| e.rule.starts_with("DET-ARB-"))
        .collect::<Vec<_>>();
    assert!(!arbitration.is_empty());
    assert!(
        arbitration.iter().all(|e| e.weight == 0),
        "arbitration evidence must carry weight 0, got: {arbitration:?}"
    );
}

#[test]
fn test_det_mv_mz_live_chain_selects_mv_inverse() {
    let dir = create_temp_game("d1_live_mv");
    write_mv_mz_leftovers(
        &dir,
        "",
        &loader_main_js(&["js/libs/pixi.js", "js/rpg_core.js", "js/rpg_managers.js", "js/plugins.js"]),
        &index_with_main_script(),
    );

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMv);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert_eq!(desc.score, 210);
    assert!(desc.conflicts.is_empty(), "conflicts: {:?}", desc.conflicts);

    let rules = evidence_rules(&desc);
    assert!(rules.iter().any(|r| r == "DET-ARB-MV-MAIN"), "got: {rules:?}");
    assert!(
        rules.iter().any(|r| r == "DET-ARB-EXCLUDED-CORE"),
        "stale MZ core must be recorded as exclusion evidence, got: {rules:?}"
    );
}

#[test]
fn test_det_mv_mz_direct_engine_script_tag_selects_mz() {
    let dir = create_temp_game("d1_direct_mz");
    write_file(
        &dir,
        "index.html",
        "<!DOCTYPE html><html><body>\n<script type=\"text/javascript\" src=\"./js/rmmz_core.js\"></script>\n<script type=\"text/javascript\" src=\"js/rmmz_managers.js\"></script>\n</body></html>\n",
    );
    write_file(&dir, "js/rmmz_core.js", "// MZ core\n");
    write_file(&dir, "js/rmmz_managers.js", "// MZ managers\n");
    write_file(&dir, "js/rpg_core.js", "// stale MV core\n");
    write_file(&dir, "js/rpg_managers.js", "// stale MV managers\n");
    write_file(&dir, "data/System.json", "{}");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMz);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert!(desc.conflicts.is_empty(), "conflicts: {:?}", desc.conflicts);
    assert!(evidence_rules(&desc).iter().any(|r| r == "DET-ARB-MZ-SCRIPT"));
}

#[test]
fn test_det_mv_mz_both_families_live_stays_ambiguous() {
    let dir = create_temp_game("d1_both_live");
    write_mv_mz_leftovers(
        &dir,
        "",
        &loader_main_js(&["js/rpg_core.js", "js/rmmz_core.js"]),
        &index_with_main_script(),
    );

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::Unknown);
    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
    assert!(
        desc.conflicts.iter().any(|c| c.contains("DET-110")),
        "conflicts: {:?}",
        desc.conflicts
    );
}

#[test]
fn test_det_mv_mz_marker_only_without_entry_chain_stays_ambiguous() {
    let dir = create_temp_game("d1_marker_only");
    write_file(&dir, "index.html", "<!DOCTYPE html><html><body></body></html>");
    write_file(&dir, "js/rpg_core.js", "// MV core\n");
    write_file(&dir, "js/rpg_managers.js", "// MV managers\n");
    write_file(&dir, "js/rmmz_core.js", "// MZ core\n");
    write_file(&dir, "js/rmmz_managers.js", "// MZ managers\n");
    write_file(&dir, "data/System.json", "{}");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::Unknown);
    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
    assert!(
        desc.conflicts.iter().any(|c| c.contains("DET-110")),
        "conflicts: {:?}",
        desc.conflicts
    );
}

#[test]
fn test_det_mv_mz_deceptive_markers_are_not_live_evidence() {
    let dir = create_temp_game("d1_deceptive_markers");
    // Fakes point at MV everywhere; the only real chain is MZ.
    let index = concat!(
        "<!DOCTYPE html><html><body>\n",
        "<!-- <script type=\"text/javascript\" src=\"js/rpg_core.js\"></script> -->\n",
        "<script type=\"text/javascript\">\n",
        "var fakeTag = '<script type=\"text/javascript\" src=\"js/rpg_core.js\"></script>';\n",
        "</script>\n",
        "<script type=\"text/javascript\" src=\"js/main.js\"></script>\n",
        "</body></html>\n"
    );
    let main_js = concat!(
        "// const scriptUrls = [\"js/rpg_core.js\"];\n",
        "/* const scriptUrls = [\"js/rpg_core.js\"]; */\n",
        "const decoy = \"for (const url of scriptUrls) { script.src = url; }\";\n",
        "const scriptUrls = [\n",
        "    \"js/rmmz_core.js\",\n",
        "    \"js/rmmz_managers.js\"\n",
        "];\n",
        "class Main {\n",
        "    run() {\n",
        "        this.loadMainScripts();\n",
        "    }\n",
        "    loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "}\n",
        "const main = new Main();\n",
        "main.run();\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, index);

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMz);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert!(
        !evidence_rules(&desc).iter().any(|r| r.starts_with("DET-ARB-MV")),
        "deceptive MV text must not produce MV liveness evidence: {:?}",
        evidence_rules(&desc)
    );
}

#[test]
fn test_det_mv_mz_deceptive_markers_only_stay_ambiguous() {
    let dir = create_temp_game("d1_deceptive_only");
    let index = concat!(
        "<!DOCTYPE html><html><body>\n",
        "<!-- <script type=\"text/javascript\" src=\"js/rpg_core.js\"></script> -->\n",
        "<script type=\"text/javascript\">\n",
        "var fakeTag = '<script src=\"js/rpg_core.js\"></script>';\n",
        "</script>\n",
        "</body></html>\n"
    );
    let main_js = concat!(
        "// const scriptUrls = [\"js/rpg_core.js\"];\n",
        "const note = \"scriptUrls = [\\\"js/rpg_core.js\\\"]\";\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, index);

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::Unknown);
    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
    assert!(
        desc.conflicts.iter().any(|c| c.contains("DET-110")),
        "conflicts: {:?}",
        desc.conflicts
    );
}

#[test]
fn test_det_mv_mz_unrecognized_loader_shape_is_insufficient() {
    // Literal list, but consumed by an unknown bootstrap helper.
    let dir = create_temp_game("d1_unknown_loader");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\", \"js/rmmz_managers.js\"];\n",
        "bootstrapAll(scriptUrls);\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);
    assert_eq!(desc.id, EngineId::Unknown);
    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);

    // Dynamic list expression is equally insufficient.
    let dir2 = create_temp_game("d1_dynamic_list");
    let main_js2 = concat!(
        "const scriptUrls = buildScriptList();\n",
        "for (const url of scriptUrls) {\n",
        "    script.src = url;\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir2, "", main_js2, &index_with_main_script());

    let idx2 = GameSourceIndex::build(&dir2).unwrap();
    let desc2 = reg.resolve(&idx2);
    assert_eq!(desc2.id, EngineId::Unknown);
    assert_eq!(desc2.confidence, DetectionConfidence::Ambiguous);
}

#[test]
fn test_det_mv_mz_oversize_entry_evidence_stays_ambiguous() {
    let dir = create_temp_game("d1_oversize_main");
    let mut main_js = loader_main_js(&["js/rmmz_core.js", "js/rmmz_managers.js"]);
    main_js.push_str("/*\n");
    main_js.push_str(&"x".repeat(70_000));
    main_js.push_str("\n*/\n");
    assert!(main_js.len() > 65_536);

    write_mv_mz_leftovers(&dir, "", &main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::Unknown);
    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
    assert!(
        desc.conflicts.iter().any(|c| c.contains("DET-110")),
        "conflicts: {:?}",
        desc.conflicts
    );
}

#[test]
fn test_det_mv_mz_competing_live_roots_stay_ambiguous_and_deterministic() {
    let dir = create_temp_game("d1_competing_live_mz");
    for root in ["payload_a/www", "payload_b/www"] {
        write_mv_mz_leftovers(
            &dir,
            root,
            &loader_main_js(&["js/rmmz_core.js", "js/rmmz_managers.js"]),
            &index_with_main_script(),
        );
        // Remove stale MV cores so each root is an unambiguous live MZ payload.
        fs::remove_file(dir.join(root).join("js/rpg_core.js")).unwrap();
        fs::remove_file(dir.join(root).join("js/rpg_managers.js")).unwrap();
    }

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::Unknown);
    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
    assert!(
        desc.conflicts.iter().any(|c| c.contains("DET-110")),
        "conflicts: {:?}",
        desc.conflicts
    );

    // Deterministic: identical result and evidence ordering on repeat.
    let again = reg.resolve(&idx);
    assert_eq!(again.id, desc.id);
    assert_eq!(again.confidence, desc.confidence);
    assert_eq!(again.conflicts, desc.conflicts);
    assert_eq!(
        evidence_rules(&again),
        evidence_rules(&desc),
        "arbitration evidence ordering must be deterministic"
    );

    // Forced same-family selection cannot clear genuine competing roots.
    use runtimepack_core::detection::DetectionOptions;
    let opts = DetectionOptions {
        forced_engine: Some(EngineId::RpgMakerMz),
        accept_detection: true,
    };
    let forced = reg.resolve_with_options(&idx, &opts);
    assert_eq!(forced.id, EngineId::Unknown);
    assert_eq!(forced.confidence, DetectionConfidence::Ambiguous);
    assert!(
        forced.conflicts.iter().any(|c| c.contains("DET-110")),
        "conflicts: {:?}",
        forced.conflicts
    );
}

#[test]
fn test_det_mv_mz_forced_wrong_family_is_not_accepted() {
    use runtimepack_core::detection::can_proceed_to_build;

    let dir = create_temp_game("d1_forced_wrong_family");
    write_mv_mz_leftovers(
        &dir,
        "",
        &loader_main_js(&["js/rmmz_core.js", "js/rmmz_managers.js"]),
        &index_with_main_script(),
    );

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve_with_engine(&idx, EngineId::RpgMakerMv);

    // Wrong forced family must never be reported as a usable MV result.
    // (Structured DET-101 mapping is the shared integration step.)
    assert_ne!(desc.id, EngineId::RpgMakerMv);
    assert_eq!(desc.id, EngineId::Unknown);
    assert!(
        can_proceed_to_build(desc.confidence, true).is_err(),
        "forced wrong family must not be build-proceedable, confidence {:?}",
        desc.confidence
    );
}

// =============================================================================
// D1 engine fix round 1 — HIGH-1/HIGH-2 + MEDIUM-3/MEDIUM-4/MEDIUM-6 regressions
//
// Every case below is a decoy that previously produced a wrong live-family
// proof. All expectations are fail-closed (`Insufficient` -> DET-110), except
// the positive controls which must keep the recognised loader working.
// =============================================================================

fn assert_ambiguous_without_live_claim(desc: &runtimepack_core::detection::EngineDescriptor) {
    assert_eq!(desc.id, EngineId::Unknown, "evidence: {:?}", desc.evidence);
    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
    let rules = evidence_rules(desc);
    assert!(
        !rules.iter().any(|r| r.starts_with("DET-ARB-MZ") || r.starts_with("DET-ARB-MV")),
        "decoy markup must not produce a live family claim, got: {rules:?}"
    );
}

#[test]
fn test_det_mv_mz_lazy_data_src_is_not_live_evidence() {
    let dir = create_temp_game("d1_fix_data_src");
    write_mv_mz_leftovers(
        &dir,
        "",
        "const unrelated = 1;\n",
        "<!DOCTYPE html><html><body>\n\
         <script type=\"text/x-lazy\" data-src=\"js/rmmz_core.js\"></script>\n\
         </body></html>\n",
    );

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_src_inside_attribute_value_is_not_a_tag() {
    let dir = create_temp_game("d1_fix_attr_embedded");
    write_mv_mz_leftovers(
        &dir,
        "",
        "const unrelated = 1;\n",
        concat!(
            "<!DOCTYPE html><html><head>\n",
            "<meta name=\"x\" content=\"src='js/rpg_core.js'\">\n",
            "<meta name=\"y\" content=\"<script src='js/rmmz_core.js'></script>\">\n",
            "</head><body></body></html>\n"
        ),
    );

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_template_script_is_inert() {
    let dir = create_temp_game("d1_fix_template");
    write_mv_mz_leftovers(
        &dir,
        "",
        "const unrelated = 1;\n",
        concat!(
            "<!DOCTYPE html><html><body>\n",
            "<template id=\"t\"><script src=\"js/rmmz_core.js\"></script></template>\n",
            "<textarea><script src=\"js/rpg_core.js\"></script></textarea>\n",
            "</body></html>\n"
        ),
    );

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_shadowed_loader_loop_is_insufficient() {
    let dir = create_temp_game("d1_fix_shadowed_loop");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "function legacy(scriptUrls) {\n",
        "    for (const url of scriptUrls) {\n",
        "        script.src = url;\n",
        "    }\n",
        "}\n",
        "legacy(loadListFromNetwork());\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_src_assignment_outside_loop_body_is_insufficient() {
    let dir = create_temp_game("d1_fix_src_after_loop");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "for (const url of scriptUrls) {\n",
        "    const script = document.createElement(\"script\");\n",
        "    document.body.appendChild(script);\n",
        "}\n",
        "script.src = url;\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_unconsumed_decoy_array_is_insufficient() {
    let dir = create_temp_game("d1_fix_decoy_array");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\", \"js/rmmz_managers.js\"];\n",
        "const otherList = [\"js/plugins.js\"];\n",
        "for (const entry of otherList) {\n",
        "    script.src = entry;\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_reassigned_script_urls_is_insufficient() {
    let dir = create_temp_game("d1_fix_reassigned");
    let main_js = concat!(
        "let scriptUrls = [\"js/rmmz_core.js\"];\n",
        "scriptUrls = buildScriptList();\n",
        "for (const url of scriptUrls) {\n",
        "    script.src = url;\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_multiple_declarations_are_insufficient() {
    let dir = create_temp_game("d1_fix_multi_decl");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "function other() {\n",
        "    const scriptUrls = [\"js/rpg_core.js\"];\n",
        "    return scriptUrls;\n",
        "}\n",
        "for (const url of scriptUrls) {\n",
        "    script.src = url;\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_external_url_entry_fails_closed() {
    let dir = create_temp_game("d1_fix_external_url");
    let main_js = concat!(
        "const scriptUrls = [\n",
        "    \"https://cdn.example.com/lib.js\",\n",
        "    \"js/rmmz_core.js\"\n",
        "];\n",
        "for (const url of scriptUrls) {\n",
        "    script.src = url;\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    // An unverifiable external entry means the loader set is not provably
    // complete, so the chain must not claim a family.
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_oversize_index_html_is_insufficient() {
    let dir = create_temp_game("d1_fix_oversize_index");
    let mut index = String::from("<!DOCTYPE html><html><body>\n<!--");
    index.push_str(&"x".repeat(70_000));
    index.push_str("-->\n");
    index.push_str("<script type=\"text/javascript\" src=\"js/main.js\"></script>\n</body></html>\n");
    assert!(index.len() > 65_536);

    write_mv_mz_leftovers(
        &dir,
        "",
        &loader_main_js(&["js/rmmz_core.js", "js/rmmz_managers.js"]),
        &index,
    );

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_non_utf8_index_html_is_insufficient() {
    let dir = create_temp_game("d1_fix_non_utf8_index");
    write_mv_mz_leftovers(
        &dir,
        "",
        &loader_main_js(&["js/rmmz_core.js", "js/rmmz_managers.js"]),
        "<!DOCTYPE html><html><body></body></html>",
    );
    fs::write(
        dir.join("index.html"),
        [0x3c, 0x68, 0x74, 0x6d, 0x6c, 0x3e, 0xff, 0xfe, 0x00, 0x3c],
    )
    .unwrap();

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_real_mz_template_loader_shape_is_live() {
    // Shape captured from a real deployed RPG Maker MZ payload
    // (`js/main.js v1.10.0`): top-level literal `scriptUrls`, consumed by the
    // `loadMainScripts()` method loop with `script.src = url` in the loop body.
    let dir = create_temp_game("d1_fix_real_mz_shape");
    let main_js = concat!(
        "//=============================================================================\n",
        "// main.js v1.10.0\n",
        "//=============================================================================\n\n",
        "const scriptUrls = [\n",
        "    \"js/libs/pixi.js\",\n",
        "    \"js/rmmz_core.js\",\n",
        "    \"js/rmmz_managers.js\",\n",
        "    \"js/plugins.js\"\n",
        "];\n",
        "const effekseerWasmUrl = \"js/libs/effekseer.wasm\";\n\n",
        "class Main {\n",
        "    constructor() {\n",
        "        this.xhrSucceeded = false;\n",
        "        this.loadCount = 0;\n",
        "    }\n\n",
        "    run() {\n",
        "        this.showLoadingSpinner();\n",
        "        this.loadMainScripts();\n",
        "    }\n\n",
        "    loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            const script = document.createElement(\"script\");\n",
        "            script.type = \"text/javascript\";\n",
        "            script.src = url;\n",
        "            script.async = false;\n",
        "            document.body.appendChild(script);\n",
        "        }\n",
        "        this.numScripts = scriptUrls.length;\n",
        "        window.addEventListener(\"load\", this.onWindowLoad.bind(this));\n",
        "    }\n",
        "}\n",
        "const main = new Main();\n",
        "main.run();\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMz, "evidence: {:?}", desc.evidence);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert!(
        evidence_rules(&desc).iter().any(|r| r == "DET-ARB-MZ-MAIN"),
        "real MZ loader shape must stay recognised, got: {:?}",
        evidence_rules(&desc)
    );
}

#[test]
fn test_det_mv_mz_uppercase_script_src_is_live() {
    let dir = create_temp_game("d1_fix_uppercase_src");
    write_mv_mz_leftovers(
        &dir,
        "",
        "const unrelated = 1;\n",
        "<!DOCTYPE html><html><head><script type=\"text/javascript\" src=\"JS/RMMZ_CORE.JS\"></script></head><body></body></html>\n",
    );

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMz, "evidence: {:?}", desc.evidence);
    assert!(
        evidence_rules(&desc).iter().any(|r| r == "DET-ARB-MZ-SCRIPT"),
        "case-folded script src must be recognised, got: {:?}",
        evidence_rules(&desc)
    );
}

#[test]
fn test_det_mv_mz_conflicting_chain_emits_no_live_family_claim() {
    let dir = create_temp_game("d1_fix_conflicting");
    write_mv_mz_leftovers(
        &dir,
        "",
        &loader_main_js(&["js/rpg_core.js", "js/rmmz_core.js"]),
        &index_with_main_script(),
    );

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);

    assert_eq!(desc.id, EngineId::Unknown);
    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
    let rules = evidence_rules(&desc);
    assert!(
        !rules
            .iter()
            .any(|r| r.starts_with("DET-ARB-MZ") || r.starts_with("DET-ARB-MV")),
        "contradictory evidence must not be reported as a live family claim, got: {rules:?}"
    );
    assert!(
        desc.conflicts.iter().any(|c| c.contains("DET-110")),
        "conflicts: {:?}",
        desc.conflicts
    );
}

#[test]
fn test_det_mv_mz_excluded_loser_carries_no_positive_weight() {
    let dir = create_temp_game("d1_fix_loser_weight");
    write_mv_mz_leftovers(
        &dir,
        "",
        &loader_main_js(&["js/rmmz_core.js", "js/rmmz_managers.js"]),
        &index_with_main_script(),
    );

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let candidates = reg.detect_all(&idx);
    let mv_candidate = candidates
        .iter()
        .find(|c| c.engine == EngineId::RpgMakerMv)
        .expect("MV candidate must exist");

    assert!(
        !mv_candidate.confidence.allows_auto_build(),
        "excluded MV candidate must not be build-eligible, confidence {:?}",
        mv_candidate.confidence
    );
    assert!(
        mv_candidate
            .conflicts
            .iter()
            .any(|c| c.contains("[DET-ARB-EXCLUDED]")),
        "excluded candidate must record its rejection, conflicts: {:?}",
        mv_candidate.conflicts
    );
    assert!(
        mv_candidate.evidence.iter().all(|e| e.weight == 0),
        "excluded candidate must not keep positive weight evidence, got: {:?}",
        mv_candidate.evidence
    );
    assert!(
        mv_candidate
            .conflicts
            .iter()
            .any(|c| c.contains("RPG Maker MZ")),
        "rejection reason must use the engine display name, conflicts: {:?}",
        mv_candidate.conflicts
    );
}

// =============================================================================
// D1 fix round 2 — HIGH-1/HIGH-2 + IMPORTANT evidence holes
// =============================================================================

#[test]
fn test_det_mv_mz_conflicting_chain_without_opposite_core_stays_ambiguous() {
    // The live chain names both families, but only the MZ core set exists on
    // disk. Contradictory evidence must never become build-eligible.
    let dir = create_temp_game("d1_f2_conflict_no_opposite_core");
    write_file(
        &dir,
        "index.html",
        "<!DOCTYPE html><html><body><script src=\"js/main.js\"></script></body></html>",
    );
    write_file(
        &dir,
        "js/main.js",
        "const scriptUrls = [\"js/rpg_core.js\", \"js/rmmz_core.js\"];\nfor (const url of scriptUrls) {\n    script.src = url;\n}\n",
    );
    write_file(&dir, "js/rmmz_core.js", "// MZ core");
    write_file(&dir, "js/rmmz_managers.js", "// MZ managers");
    write_file(&dir, "data/System.json", "{}");

    let idx = GameSourceIndex::build(&dir).unwrap();
    let reg = setup_registry();
    let desc = reg.resolve(&idx);

    assert_eq!(desc.id, EngineId::Unknown, "evidence: {:?}", desc.evidence);
    assert_eq!(desc.confidence, DetectionConfidence::Ambiguous);
    assert!(
        desc.conflicts.iter().any(|c| c.contains("DET-110")),
        "conflicting chain must record DET-110 even without the opposite core file, conflicts: {:?}",
        desc.conflicts
    );

    // Forced MZ must stay ambiguity, never an invalid-override claim.
    let forced = reg.resolve_with_engine(&idx, EngineId::RpgMakerMz);
    assert_eq!(forced.id, EngineId::Unknown);
    assert_eq!(forced.confidence, DetectionConfidence::Ambiguous);
    assert!(
        forced.conflicts.iter().any(|c| c.contains("DET-110")),
        "conflicts: {:?}",
        forced.conflicts
    );
    assert!(
        runtimepack_core::detection::engine_override_rejection(&forced).is_none(),
        "contradictory evidence must not be reported as an invalid override"
    );
}

#[test]
fn test_det_mv_mz_brace_less_loop_body_is_insufficient() {
    let dir = create_temp_game("d1_f2_brace_less_loop");
    let main_js = concat!(
        "const scriptUrls = [\"js/rpg_core.js\"];\n",
        "for (const url of scriptUrls) console.log(url);\n",
        "function boot() {\n",
        "    script.src = url;\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_loop_body_must_start_at_header() {
    // A later unrelated block must never be mistaken for the loop body.
    let dir = create_temp_game("d1_f2_later_block");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "for (const url of scriptUrls) doSomething(url);\n",
        "if (ready) {\n",
        "    script.src = url;\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_never_called_function_is_insufficient() {
    let dir = create_temp_game("d1_f2_never_called");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "function neverCalled() {\n",
        "    for (const url of scriptUrls) {\n",
        "        script.src = url;\n",
        "    }\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_noscript_script_is_inert() {
    let dir = create_temp_game("d1_f2_noscript");
    write_mv_mz_leftovers(
        &dir,
        "",
        "const unrelated = 1;\n",
        concat!(
            "<!DOCTYPE html><html><body>\n",
            "<noscript><script src=\"js/rmmz_core.js\"></script></noscript>\n",
            "</body></html>\n"
        ),
    );

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_tag_name_prefix_is_not_a_tag() {
    let dir = create_temp_game("d1_f2_tag_prefix");
    write_mv_mz_leftovers(
        &dir,
        "",
        "const unrelated = 1;\n",
        concat!(
            "<!DOCTYPE html><html><body>\n",
            "<templatex><script src=\"js/rpg_core.js\"></script></templatex>\n",
            "<script src=\"js/main.js\"></scriptfoo>\n",
            "</body></html>\n"
        ),
    );

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_minified_declaration_spacing_is_live() {
    // `var scriptUrls=[` (no spaces) must still resolve to the declaration
    // identifier instead of being read as a reassignment.
    let dir = create_temp_game("d1_f2_minified_decl");
    let main_js = concat!(
        "var scriptUrls=[\"js/rmmz_core.js\"];\n",
        "function load() {\n",
        "    for (const url of scriptUrls) {\n",
        "        script.src = url;\n",
        "    }\n",
        "}\n",
        "load();\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMz, "evidence: {:?}", desc.evidence);
    assert!(
        evidence_rules(&desc).iter().any(|r| r == "DET-ARB-MZ-MAIN"),
        "minified declaration must stay recognised, got: {:?}",
        evidence_rules(&desc)
    );
}

#[test]
fn test_det_mv_mz_real_g06_template_loader_shape_is_live() {
    // Structural replica of the real g06 `js/main.js v1.5.0` (extracted
    // read-only from the shipped archive, sha256
    // cf90cf748c5563e8161eeb479fe6b93e621f7315332c61bd8184f798135a0002):
    // top-level literal `scriptUrls`, the loop inside the `loadMainScripts()`
    // class method, `this.loadMainScripts()` called from `run()`, and
    // `const main = new Main(); main.run();` at module top level.
    let dir = create_temp_game("d1_f2_real_g06_shape");
    let main_js = concat!(
        "//=============================================================================\n",
        "// main.js v1.5.0\n",
        "//=============================================================================\n\n",
        "const scriptUrls = [\n",
        "    \"js/libs/pixi.js\",\n",
        "    \"js/libs/pako.min.js\",\n",
        "    \"js/rmmz_core.js\",\n",
        "    \"js/rmmz_managers.js\",\n",
        "    \"js/plugins.js\"\n",
        "];\n",
        "const effekseerWasmUrl = \"js/libs/effekseer.wasm\";\n\n",
        "class Main {\n",
        "    constructor() {\n",
        "        this.xhrSucceeded = false;\n",
        "        this.loadCount = 0;\n",
        "        this.error = null;\n",
        "    }\n\n",
        "    run() {\n",
        "        this.showLoadingSpinner();\n",
        "        this.testXhr();\n",
        "        this.hookNwjsClose();\n",
        "        this.loadMainScripts();\n",
        "    }\n\n",
        "    showLoadingSpinner() {\n",
        "        const loadingSpinner = document.createElement(\"div\");\n",
        "        document.body.appendChild(loadingSpinner);\n",
        "    }\n\n",
        "    testXhr() {\n",
        "        const xhr = new XMLHttpRequest();\n",
        "        xhr.open(\"GET\", document.currentScript.src);\n",
        "        xhr.send();\n",
        "    }\n\n",
        "    hookNwjsClose() {\n",
        "        if (typeof nw === \"object\") {\n",
        "            nw.Window.get().on(\"close\", () => nw.App.quit());\n",
        "        }\n",
        "    }\n\n",
        "    loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            const script = document.createElement(\"script\");\n",
        "            script.type = \"text/javascript\";\n",
        "            script.src = url;\n",
        "            script.async = false;\n",
        "            script.defer = true;\n",
        "            document.body.appendChild(script);\n",
        "        }\n",
        "        this.numScripts = scriptUrls.length;\n",
        "        window.addEventListener(\"load\", this.onWindowLoad.bind(this));\n",
        "    }\n",
        "}\n\n",
        "const main = new Main();\n",
        "main.run();\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMz, "evidence: {:?}", desc.evidence);
    assert_eq!(desc.confidence, DetectionConfidence::Exact);
    assert!(
        evidence_rules(&desc).iter().any(|r| r == "DET-ARB-MZ-MAIN"),
        "real g06 loader shape must stay recognised, got: {:?}",
        evidence_rules(&desc)
    );
    assert!(
        evidence_rules(&desc)
            .iter()
            .any(|r| r == "DET-ARB-EXCLUDED-CORE"),
        "stale MV core must be excluded, got: {:?}",
        evidence_rules(&desc)
    );
}

#[test]
fn test_arb_rule_ids_match_core_contract() {
    // Cross-crate drift guard: the engine emits these rule ids and core
    // consumes them through its own constants. A one-sided rename must fail
    // here instead of silently disabling the override/conflict guards.
    let dir = create_temp_game("d1_f2_rule_contract");
    write_mv_mz_leftovers(
        &dir,
        "",
        &loader_main_js(&["js/rmmz_core.js", "js/rmmz_managers.js"]),
        &index_with_main_script(),
    );
    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    let rules = evidence_rules(&desc);

    assert!(
        runtimepack_core::detection::ARB_LIVE_RULES_MZ
            .iter()
            .any(|r| rules.iter().any(|e| e == r)),
        "engine liveness rule must be one of core's MZ rules, got: {rules:?}"
    );
    assert!(
        rules
            .iter()
            .any(|r| r == runtimepack_core::detection::ARB_EXCLUDED_CORE_RULE),
        "engine exclusion rule must equal core's, got: {rules:?}"
    );

    // Excluded-candidate marker consumed by core's override validation.
    let candidates = setup_registry().detect_all(&idx);
    let mv_candidate = candidates
        .iter()
        .find(|c| c.engine == EngineId::RpgMakerMv)
        .expect("MV candidate");
    assert!(
        mv_candidate.conflicts.iter().any(|c| {
            c.starts_with(runtimepack_core::detection::ARB_EXCLUDED_CONFLICT_MARKER)
        }),
        "exclusion marker must match core's constant, conflicts: {:?}",
        mv_candidate.conflicts
    );

    // Contradictory evidence rule consumed by core's conflict special-case.
    let dir2 = create_temp_game("d1_f2_rule_contract_conflict");
    write_mv_mz_leftovers(
        &dir2,
        "",
        &loader_main_js(&["js/rpg_core.js", "js/rmmz_core.js"]),
        &index_with_main_script(),
    );
    let idx2 = GameSourceIndex::build(&dir2).unwrap();
    let desc2 = setup_registry().resolve(&idx2);
    assert!(
        evidence_rules(&desc2)
            .iter()
            .any(|r| r == runtimepack_core::detection::ARB_CONFLICT_RULE),
        "conflict rule must match core's constant, got: {:?}",
        evidence_rules(&desc2)
    );

    // MV direction must be covered too, so a one-sided MV rename is caught.
    let dir3 = create_temp_game("d1_f2_rule_contract_mv");
    write_mv_mz_leftovers(
        &dir3,
        "",
        &loader_main_js(&["js/rpg_core.js", "js/rpg_managers.js"]),
        &index_with_main_script(),
    );
    let idx3 = GameSourceIndex::build(&dir3).unwrap();
    let desc3 = setup_registry().resolve(&idx3);
    let rules3 = evidence_rules(&desc3);
    assert!(
        runtimepack_core::detection::ARB_LIVE_RULES_MV
            .iter()
            .any(|r| rules3.iter().any(|e| e == r)),
        "engine liveness rule must be one of core's MV rules, got: {rules3:?}"
    );
}




// =============================================================================
// D1 fix round 3 — reachability must cover class/object methods and must not
// accept definitions, `new` expressions or foreign receivers as calls.
// =============================================================================

#[test]
fn test_det_mv_mz_never_called_class_method_is_insufficient() {
    // Real MV/MZ shape: the loader loop lives in a class method. With no call
    // chain the method body is dead code, so no family may be claimed even
    // though the opposite (MZ) core set exists on disk.
    let dir = create_temp_game("d1_f3_never_called_class_method");
    let main_js = concat!(
        "const scriptUrls = [\"js/rpg_core.js\"];\n",
        "class Main {\n",
        "    loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_never_called_object_method_is_insufficient() {
    let dir = create_temp_game("d1_f3_never_called_object_method");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "const registry = {\n",
        "    loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "};\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_class_method_without_instance_binding_is_insufficient() {
    // `run()` calls the loader method, but nothing constructs `Main`, so the
    // chain never starts at module scope.
    let dir = create_temp_game("d1_f3_method_no_instance");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "class Main {\n",
        "    run() {\n",
        "        this.loadMainScripts();\n",
        "    }\n",
        "    loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_self_recursive_method_is_insufficient() {
    let dir = create_temp_game("d1_f3_self_recursive_method");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "class Main {\n",
        "    loadMainScripts() {\n",
        "        this.loadMainScripts();\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_property_definition_is_not_a_call() {
    // A same-named object-literal property definition is not a call site.
    let dir = create_temp_game("d1_f3_property_definition");
    let main_js = concat!(
        "const scriptUrls = [\"js/rpg_core.js\"];\n",
        "function neverCalled() {\n",
        "    for (const url of scriptUrls) {\n",
        "        script.src = url;\n",
        "    }\n",
        "}\n",
        "const registry = { neverCalled() { return 1; } };\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_foreign_receiver_is_not_a_call() {
    // `other.neverCalled()` cannot prove the top-level function body runs.
    let dir = create_temp_game("d1_f3_foreign_receiver");
    let main_js = concat!(
        "const scriptUrls = [\"js/rpg_core.js\"];\n",
        "function neverCalled() {\n",
        "    for (const url of scriptUrls) {\n",
        "        script.src = url;\n",
        "    }\n",
        "}\n",
        "const other = {};\n",
        "other.neverCalled();\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_new_expression_is_not_a_call() {
    // `new neverCalled()` is a constructor expression, not an invocation of the
    // function body that contains the loop.
    let dir = create_temp_game("d1_f3_new_expression");
    let main_js = concat!(
        "const scriptUrls = [\"js/rpg_core.js\"];\n",
        "function neverCalled() {\n",
        "    for (const url of scriptUrls) {\n",
        "        script.src = url;\n",
        "    }\n",
        "}\n",
        "const instance = new neverCalled();\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_aliased_method_is_insufficient() {
    // An alias (`const f = main.run`) is dynamic dispatch: unprovable.
    let dir = create_temp_game("d1_f3_alias");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "class Main {\n",
        "    loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "}\n",
        "const main = new Main();\n",
        "const boot = main.loadMainScripts;\n",
        "boot();\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_event_registration_is_insufficient() {
    let dir = create_temp_game("d1_f3_event_registration");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "function loadMainScripts() {\n",
        "    for (const url of scriptUrls) {\n",
        "        script.src = url;\n",
        "    }\n",
        "}\n",
        "window.addEventListener(\"load\", loadMainScripts);\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_object_method_invoked_via_own_binding_is_live() {
    // Supported positive: the method is called through the object literal's own
    // binding, which is the only receiver that can prove the body runs.
    let dir = create_temp_game("d1_f3_object_method_live");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "const registry = {\n",
        "    loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "};\n",
        "registry.loadMainScripts();\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMz, "evidence: {:?}", desc.evidence);
    assert!(
        evidence_rules(&desc).iter().any(|r| r == "DET-ARB-MZ-MAIN"),
        "object method invoked through its own binding must stay live, got: {:?}",
        evidence_rules(&desc)
    );
}

#[test]
fn test_det_mv_mz_bare_function_with_real_call_site_is_live() {
    let dir = create_temp_game("d1_f3_bare_function_live");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "function loadMainScripts() {\n",
        "    for (const url of scriptUrls) {\n",
        "        script.src = url;\n",
        "    }\n",
        "}\n",
        "loadMainScripts();\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMz, "evidence: {:?}", desc.evidence);
    assert!(
        evidence_rules(&desc).iter().any(|r| r == "DET-ARB-MZ-MAIN"),
        "bare function with a real call site must stay live, got: {:?}",
        evidence_rules(&desc)
    );
}

// =============================================================================
// D1 fix round 4 — a call site proves a *binding*, not a name. Two blocking
// defects remain: (A) a receiver identifier shadowed by an enclosing parameter
// or lexical declaration is resolved to the module-level binding, and (B) a
// named function expression is classified as a module-level declaration even
// though its name is only visible inside its own body.
// =============================================================================

/// Forcing a family must never turn an unproven chain into build-proceedable
/// evidence.
fn assert_forced_family_not_buildable(idx: &GameSourceIndex, engine: EngineId) {
    let forced = setup_registry().resolve_with_engine(idx, engine);
    assert_eq!(forced.id, EngineId::Unknown, "evidence: {:?}", forced.evidence);
    assert!(
        runtimepack_core::detection::can_proceed_to_build(forced.confidence, true).is_err(),
        "forced {engine:?} must not be build-proceedable, confidence {:?}",
        forced.confidence
    );
}

#[test]
fn test_det_mv_mz_shadowed_receiver_parameter_is_insufficient() {
    // `main` at the call site is the parameter of `boot`, not the module-level
    // `new Main()` instance. Argument values are not evaluated, so this shape is
    // unprovable: the loader method body is not proven to run.
    let dir = create_temp_game("d1_f4_shadowed_receiver_param");
    let main_js = concat!(
        "const scriptUrls = [\"js/rpg_core.js\"];\n",
        "class Main {\n",
        "    loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "}\n",
        "const main = new Main();\n",
        "function boot(main) {\n",
        "    main.loadMainScripts();\n",
        "}\n",
        "boot(someOtherThing);\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
    assert_forced_family_not_buildable(&idx, EngineId::RpgMakerMv);
}

#[test]
fn test_det_mv_mz_shadowed_receiver_block_binding_is_insufficient() {
    // The nearest visible `main` at the call site is a block-scoped `let`, not
    // the module-level `new Main()` instance.
    let dir = create_temp_game("d1_f4_shadowed_receiver_block");
    let main_js = concat!(
        "const scriptUrls = [\"js/rpg_core.js\"];\n",
        "class Main {\n",
        "    loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "}\n",
        "const main = new Main();\n",
        "if (ready) {\n",
        "    let main = other;\n",
        "    main.loadMainScripts();\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_named_function_expression_property_is_insufficient() {
    // `function loadMainScripts()` is a named *function expression*: its name is
    // visible only inside its own body, so the bare module-level call is a
    // ReferenceError at runtime and cannot prove the loader loop is live.
    let dir = create_temp_game("d1_f4_named_fe_property");
    let main_js = concat!(
        "const scriptUrls = [\"js/rpg_core.js\"];\n",
        "const o = {\n",
        "    loadMainScripts: function loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "};\n",
        "loadMainScripts();\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
    assert_forced_family_not_buildable(&idx, EngineId::RpgMakerMv);
}

#[test]
fn test_det_mv_mz_named_function_expression_self_scope_is_not_global() {
    // `named` is bound only inside the function expression; a bare `named()`
    // at module scope does not reach it.
    let dir = create_temp_game("d1_f4_named_fe_binding");
    let main_js = concat!(
        "const scriptUrls = [\"js/rpg_core.js\"];\n",
        "const f = function named() {\n",
        "    for (const url of scriptUrls) {\n",
        "        script.src = url;\n",
        "    }\n",
        "};\n",
        "named();\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_named_function_expression_property_via_own_binding_is_live() {
    // Positive control for the same shape: invoked through the object literal's
    // own binding, the named function expression is provably reachable, so the
    // fix must classify it as an object method rather than reject all such
    // shapes.
    let dir = create_temp_game("d1_f4_named_fe_property_live");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "const o = {\n",
        "    loadMainScripts: function loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "};\n",
        "o.loadMainScripts();\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMz, "evidence: {:?}", desc.evidence);
    assert!(
        evidence_rules(&desc).iter().any(|r| r == "DET-ARB-MZ-MAIN"),
        "named function expression invoked through its own binding must stay live, got: {:?}",
        evidence_rules(&desc)
    );
}

#[test]
fn test_det_mv_mz_named_function_expression_property_mismatched_key_is_insufficient() {
    // The property key and the function expression name differ, so a call to
    // the property cannot be proven to reach this body.
    let dir = create_temp_game("d1_f4_named_fe_key_mismatch");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "const o = {\n",
        "    other: function loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "};\n",
        "o.loadMainScripts();\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

// =============================================================================
// D1 fix round 5 — the remaining lexical binding forms. `catch (name)` binds a
// parameter that shadows an outer declaration, and the nearest-binding check
// must also respect declaration order (TDZ) and declarations that appear after
// the call site inside the same lexical scope.
// =============================================================================

#[test]
fn test_det_mv_mz_catch_parameter_shadow_is_insufficient() {
    // `main` inside the catch block is the caught value, not the module-level
    // `new Main()` instance. Calling the loader on it throws at runtime, so the
    // chain is not proven live.
    let dir = create_temp_game("d1_f5_catch_param");
    let main_js = concat!(
        "const scriptUrls = [\"js/rpg_core.js\"];\n",
        "class Main {\n",
        "    loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "}\n",
        "const main = new Main();\n",
        "try { boot(); } catch (main) {\n",
        "    main.loadMainScripts();\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
    assert_forced_family_not_buildable(&idx, EngineId::RpgMakerMv);
}

#[test]
fn test_det_mv_mz_destructured_catch_parameter_shadow_is_insufficient() {
    // A destructured catch binding cannot be read statically, so it must fail
    // closed instead of falling back to the outer binding.
    let dir = create_temp_game("d1_f5_catch_destructured");
    let main_js = concat!(
        "const scriptUrls = [\"js/rpg_core.js\"];\n",
        "class Main {\n",
        "    loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "}\n",
        "const main = new Main();\n",
        "try { boot(); } catch ({ main }) {\n",
        "    main.loadMainScripts();\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_optional_catch_binding_keeps_outer_binding_live() {
    // `catch {` binds nothing, so the outer instance really is what the call
    // uses. The catch fix must not become a blanket rejection.
    let dir = create_temp_game("d1_f5_catch_optional_live");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "class Main {\n",
        "    loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "}\n",
        "const main = new Main();\n",
        "try { boot(); } catch {\n",
        "    main.loadMainScripts();\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMz, "evidence: {:?}", desc.evidence);
    assert!(
        evidence_rules(&desc).iter().any(|r| r == "DET-ARB-MZ-MAIN"),
        "optional catch binding must keep the outer binding live, got: {:?}",
        evidence_rules(&desc)
    );
}

#[test]
fn test_det_mv_mz_var_declared_after_call_site_is_insufficient() {
    // `var main` hoists to the whole function scope, so the receiver at the call
    // site is the (undefined) local, not the module-level instance.
    let dir = create_temp_game("d1_f5_var_after_call");
    let main_js = concat!(
        "const scriptUrls = [\"js/rpg_core.js\"];\n",
        "class Main {\n",
        "    loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "}\n",
        "const main = new Main();\n",
        "function boot() {\n",
        "    main.loadMainScripts();\n",
        "    var main = other;\n",
        "}\n",
        "boot();\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_parenthesised_arrow_called_before_declaration_is_insufficient() {
    // `const load = () => {…}` is in the temporal dead zone at the earlier call
    // site. This reader also treats an arrow with a parenthesised parameter list
    // as an `Anonymous` barrier, so the shape is rejected for both reasons.
    let dir = create_temp_game("d1_f5_arrow_before_decl");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "load();\n",
        "const load = () => {\n",
        "    for (const url of scriptUrls) {\n",
        "        script.src = url;\n",
        "    }\n",
        "};\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_bound_scope_call_before_its_header_is_insufficient() {
    // A single-parameter arrow is an `Anonymous` barrier (its trailing identifier
    // is a parameter, not a binding), so this shape is rejected for that reason
    // as well as for the temporal dead zone at the earlier call site.
    let dir = create_temp_game("d1_f5_bound_before_header");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "load();\n",
        "const f = load => {\n",
        "    for (const url of scriptUrls) {\n",
        "        script.src = url;\n",
        "    }\n",
        "};\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_hoisted_function_called_before_declaration_is_live() {
    // Control: `function` declarations hoist, so a call before the declaration
    // in the same scope is a genuine entrypoint.
    let dir = create_temp_game("d1_f5_decl_before_call_live");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "loadMainScripts();\n",
        "function loadMainScripts() {\n",
        "    for (const url of scriptUrls) {\n",
        "        script.src = url;\n",
        "    }\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMz, "evidence: {:?}", desc.evidence);
    assert!(
        evidence_rules(&desc).iter().any(|r| r == "DET-ARB-MZ-MAIN"),
        "hoisted function declaration must stay live, got: {:?}",
        evidence_rules(&desc)
    );
}

#[test]
fn test_det_mv_mz_block_scoped_function_declaration_is_not_module_visible() {
    // A `function` declared inside a block is not assumed to be a module-level
    // binding (that would need sloppy-mode Annex B hoisting), so a bare call
    // outside the block cannot prove the loader loop runs.
    let dir = create_temp_game("d1_f4_block_scoped_decl");
    let main_js = concat!(
        "const scriptUrls = [\"js/rpg_core.js\"];\n",
        "if (ready) {\n",
        "    function loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "}\n",
        "loadMainScripts();\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
}

#[test]
fn test_det_mv_mz_single_parameter_arrow_parameter_is_not_a_binding() {
    // Final gate review §4. A single-parameter arrow names its *parameter*, not
    // the binding the arrow is stored in, so a bare call of that parameter name
    // can never be a proven invocation of the arrow body.
    //
    // Repro A: `url()` is a ReferenceError; the loader loop is dead code.
    let dir = create_temp_game("d1_final_arrow_param_a");
    let main_js = concat!(
        "const scriptUrls = [\"js/rpg_core.js\"];\n",
        "const boot = url => {\n",
        "    for (const url of scriptUrls) {\n",
        "        script.src = url;\n",
        "    }\n",
        "};\n",
        "url();\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);
    assert_ambiguous_without_live_claim(&desc);
    assert_forced_family_not_buildable(&idx, EngineId::RpgMakerMv);

    // Repro B: `run()` calls the function, never the arrow stored in `init`.
    let dir2 = create_temp_game("d1_final_arrow_param_b");
    let main_js2 = concat!(
        "const scriptUrls = [\"js/rpg_core.js\"];\n",
        "function run() { return 1; }\n",
        "const init = run => {\n",
        "    for (const url of scriptUrls) {\n",
        "        script.src = url;\n",
        "    }\n",
        "};\n",
        "run();\n"
    );
    write_mv_mz_leftovers(&dir2, "", main_js2, &index_with_main_script());

    let idx2 = GameSourceIndex::build(&dir2).unwrap();
    let desc2 = setup_registry().resolve(&idx2);
    assert_ambiguous_without_live_claim(&desc2);
    assert_forced_family_not_buildable(&idx2, EngineId::RpgMakerMv);
}

#[test]
fn test_det_mv_mz_block_scoped_function_called_in_its_block_is_live() {
    // Positive control: the same declaration stays provable when the call sits
    // in the same block, where the binding genuinely is in scope.
    let dir = create_temp_game("d1_f4_block_scoped_decl_live");
    let main_js = concat!(
        "const scriptUrls = [\"js/rmmz_core.js\"];\n",
        "if (ready) {\n",
        "    function loadMainScripts() {\n",
        "        for (const url of scriptUrls) {\n",
        "            script.src = url;\n",
        "        }\n",
        "    }\n",
        "    loadMainScripts();\n",
        "}\n"
    );
    write_mv_mz_leftovers(&dir, "", main_js, &index_with_main_script());

    let idx = GameSourceIndex::build(&dir).unwrap();
    let desc = setup_registry().resolve(&idx);

    assert_eq!(desc.id, EngineId::RpgMakerMz, "evidence: {:?}", desc.evidence);
    assert!(
        evidence_rules(&desc).iter().any(|r| r == "DET-ARB-MZ-MAIN"),
        "block-scoped declaration called in its own block must stay live, got: {:?}",
        evidence_rules(&desc)
    );
}
