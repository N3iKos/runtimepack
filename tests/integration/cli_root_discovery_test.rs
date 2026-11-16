use runtimepack_core::build_plan::BuildManifest;
use runtimepack_pack::GpackReader;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(1);

fn temp_dir(label: &str) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "runtimepack_cli_root_test_{}_{}_{}",
        std::process::id(),
        label,
        id
    ));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn write_file(base: &Path, rel: &str, content: &[u8]) {
    let full = base.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).expect("create parent directory");
    }
    fs::write(&full, content).expect("write file");
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of cli is workspace root")
        .to_path_buf()
}

fn fixture(name: &str) -> PathBuf {
    workspace_root().join("tests").join("fixtures").join(name)
}

fn run_cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .args(args)
        .output()
        .expect("execute runtimepack CLI binary")
}

fn create_nested_mz_fixture() -> PathBuf {
    let dir = temp_dir("nested_mz");
    write_file(&dir, "bin/www/index.html", b"<!DOCTYPE html><html><head><script src=\"js/rmmz_core.js\"></script></head><body></body></html>");
    write_file(&dir, "bin/www/js/rmmz_core.js", b"// MZ core engine");
    write_file(&dir, "bin/www/js/rmmz_managers.js", b"// MZ managers");
    write_file(&dir, "bin/www/data/System.json", b"{\"gameTitle\":\"Synthetic MZ Test\"}");
    write_file(&dir, "game.exe", b"MZ_DESKTOP_RUNNER_BINARY_BYTES");
    write_file(&dir, "nw.dll", b"NW_RUNNER_DLL");
    // NW.js wrapper descriptor: the desktop-component classification of the outer
    // `nw.dll` requires a proven package boundary (descriptor + local main entry +
    // engine marker + runner anchor), not a filename match alone.
    write_file(
        &dir,
        "package.json",
        b"{\"name\":\"nested-mz\",\"main\":\"bin/www/index.html\"}",
    );
    dir
}

/// Same nested layout as `create_nested_mz_fixture`, minus the package descriptor.
/// Used only to prove the boundary is *required*: without it the outer runner DLL
/// must stay a hard blocker and no desktop-component classification may appear.
fn create_nested_mz_fixture_without_descriptor() -> PathBuf {
    let dir = temp_dir("nested_mz_no_descriptor");
    write_file(&dir, "bin/www/index.html", b"<!DOCTYPE html><html><head><script src=\"js/rmmz_core.js\"></script></head><body></body></html>");
    write_file(&dir, "bin/www/js/rmmz_core.js", b"// MZ core engine");
    write_file(&dir, "bin/www/js/rmmz_managers.js", b"// MZ managers");
    write_file(&dir, "bin/www/data/System.json", b"{\"gameTitle\":\"Synthetic MZ Test\"}");
    write_file(&dir, "game.exe", b"MZ_DESKTOP_RUNNER_BINARY_BYTES");
    write_file(&dir, "nw.dll", b"NW_RUNNER_DLL");
    dir
}

fn create_top_level_mz_fixture() -> PathBuf {
    let dir = temp_dir("top_level_mz");
    write_file(&dir, "index.html", b"<!DOCTYPE html><html><head><script src=\"js/rmmz_core.js\"></script></head><body></body></html>");
    write_file(&dir, "js/rmmz_core.js", b"// MZ core engine");
    write_file(&dir, "js/rmmz_managers.js", b"// MZ managers");
    write_file(&dir, "data/System.json", b"{\"gameTitle\":\"Top Level MZ\"}");
    dir
}

fn create_vnm_fixture() -> PathBuf {
    let dir = temp_dir("vnm_min");
    write_file(&dir, "data/ENGINE.js", b"window.gs = {}; window.gs.GameStorage = function() {};");
    write_file(&dir, "data/SCRIPTS.js", b"vn.Component_MessageBehavior = {}; gs.Application = {};");
    write_file(&dir, "index.html", b"<!DOCTYPE html><html><head><script src=\"data/ENGINE.js\"></script><script src=\"data/SCRIPTS.js\"></script></head><body></body></html>");
    dir
}

#[test]
fn test_cli_inspect_nested_mz_reports_scoped_root() {
    let game_dir = create_nested_mz_fixture();
    let output = run_cli(&["inspect", game_dir.to_str().unwrap()]);
    assert!(output.status.success(), "inspect must exit 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Engine:         RPG Maker MZ"), "must detect MZ");
    assert!(stdout.contains("Confidence:     EXACT"), "confidence must be EXACT");
    assert!(stdout.contains("Score:          210"), "score must be 210");
    assert!(stdout.contains("Content Root:   bin/www"), "content root must report bin/www");
    assert!(stdout.contains("Status:         COMPATIBLE"), "independent analyzer status must be COMPATIBLE");
    assert!(stdout.contains("[CMP-106]"), "outer nw.dll must be classified as CMP-106 desktop excluded");
}

/// Mandatory counterexample for the row above: the same nested layout without a
/// package descriptor proves no wrapper boundary, so the outer runner DLL must
/// remain a hard blocker and must never be reported as `CMP-106`. Without this row
/// the classification could be "fixed" by weakening the boundary predicate.
#[test]
fn test_cli_inspect_nested_mz_without_descriptor_stays_unsupported_dependency() {
    let game_dir = create_nested_mz_fixture_without_descriptor();
    let output = run_cli(&["inspect", game_dir.to_str().unwrap()]);
    assert!(output.status.success(), "inspect must exit 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Engine:         RPG Maker MZ"), "must still detect MZ");
    assert!(stdout.contains("Content Root:   bin/www"), "content root must still report bin/www");
    assert!(
        !stdout.contains("[CMP-106]"),
        "no desktop-component classification may be invented without a proven boundary, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Status:         UNSUPPORTED_DEPENDENCY"),
        "an unproven outer runner DLL must keep the hard status, got:\n{stdout}"
    );
    assert!(
        stdout.contains("[CMP-102]"),
        "the outer runner DLL must stay a raw hard finding, got:\n{stdout}"
    );
}

#[test]
fn test_cli_inspect_top_level_mz_reports_root() {
    let game_dir = create_top_level_mz_fixture();
    let output = run_cli(&["inspect", game_dir.to_str().unwrap()]);
    assert!(output.status.success(), "inspect must exit 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Engine:         RPG Maker MZ"), "must detect MZ");
    assert!(stdout.contains("Content Root:   <root>"), "content root must report <root>");
}

#[test]
fn test_cli_explain_nested_mz_candidate_selection_and_rejection() {
    let game_dir = create_nested_mz_fixture();
    let output = run_cli(&["explain", game_dir.to_str().unwrap()]);
    assert!(output.status.success(), "explain must exit 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Evaluated Candidates:"), "must list evaluated candidates");
    assert!(stdout.contains("root: bin/www"), "candidate bin/www must be present");
    assert!(stdout.contains("[SELECTED]"), "selected candidate must be explicitly marked [SELECTED]");
    assert!(stdout.contains("[REJECTED]"), "outer or non-chosen candidate must be marked [REJECTED]");
    assert!(stdout.contains("Final Resolution:"), "must report final resolution");
    assert!(stdout.contains("Selected Engine: RPG Maker MZ"), "selected engine must be MZ");
    assert!(stdout.contains("Content Root:    bin/www"), "final resolution content root must be bin/www");
    assert!(stdout.contains("Score:           210"), "score must be 210");
}

#[test]
fn test_cli_explain_ambiguous_markers_fails_or_explains() {
    let dir = temp_dir("ambiguous_cli");
    write_file(&dir, "bin/www/js/rpg_core.js", b"// MV");
    write_file(&dir, "bin/www/js/rmmz_core.js", b"// MZ");
    write_file(&dir, "bin/www/data/System.json", b"{}");

    let output = run_cli(&["explain", dir.to_str().unwrap()]);
    assert!(output.status.success(), "explain command reports rationale even for ambiguous detection");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Conflict:") || stdout.contains("Ambiguous") || stdout.contains("DET-110"),
        "explain must detail conflicts or ambiguous state");
}

#[test]
fn test_cli_explain_two_competing_roots_shows_ambiguous_rationale_not_unknown_mismatch() {
    let dir = temp_dir("two_mz_roots");
    // Root 1
    write_file(&dir, "slot_a/index.html", b"<html></html>");
    write_file(&dir, "slot_a/js/rmmz_core.js", b"// MZ core 1");
    write_file(&dir, "slot_a/js/rmmz_managers.js", b"// MZ managers 1");
    write_file(&dir, "slot_a/data/System.json", b"{}");
    // Root 2
    write_file(&dir, "slot_b/index.html", b"<html></html>");
    write_file(&dir, "slot_b/js/rmmz_core.js", b"// MZ core 2");
    write_file(&dir, "slot_b/js/rmmz_managers.js", b"// MZ managers 2");
    write_file(&dir, "slot_b/data/System.json", b"{}");

    let output = run_cli(&["explain", dir.to_str().unwrap()]);
    assert!(output.status.success(), "explain must exit 0 even when ambiguous");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("DET-110"), "must cite DET-110 ambiguity");
    assert!(stdout.contains("slot_a"), "slot_a candidate must be listed");
    assert!(stdout.contains("slot_b"), "slot_b candidate must be listed");
    assert!(stdout.contains("[AMBIGUOUS]"), "competing roots must be annotated [AMBIGUOUS]");
    assert!(
        !stdout.contains("Engine superseded or does not match resolution (Unknown)")
            && !stdout.contains("does not match resolution (Unknown)"),
        "participating candidates must not report engine mismatch with Unknown; got:\n{}",
        stdout
    );
}

#[test]
fn test_cli_engine_override_and_policy() {
    let game_dir = create_nested_mz_fixture();

    // Invalid engine override must fail early with [DET-101]
    let output_invalid = run_cli(&["inspect", game_dir.to_str().unwrap(), "--engine", "invalid_unknown"]);
    assert!(!output_invalid.status.success(), "invalid engine override must exit non-zero");
    let stderr_invalid = String::from_utf8_lossy(&output_invalid.stderr);
    assert!(stderr_invalid.contains("[DET-101]"), "must cite DET-101 error code");

    // Forced engine flag on valid engine succeeds
    let output_forced = run_cli(&["inspect", game_dir.to_str().unwrap(), "--engine", "rpgmaker-mz"]);
    assert!(output_forced.status.success(), "forced valid engine must exit 0");
    let stdout_forced = String::from_utf8_lossy(&output_forced.stdout);
    assert!(stdout_forced.contains("Engine:         RPG Maker MZ"));
    assert!(stdout_forced.contains("Content Root:   bin/www"));
}

#[test]
fn test_cli_build_nested_mz_packaging_and_manifest() {
    let game_dir = create_nested_mz_fixture();
    let out_dir = temp_dir("nested_mz_out");

    let output = run_cli(&[
        "build",
        game_dir.to_str().unwrap(),
        "--output",
        out_dir.to_str().unwrap(),
        "--no-compile",
        "--flavor",
        "beta",
        "--accept-audit-fail",
        "--accept-audit-unknown",
    ]);
    assert!(output.status.success(), "build must succeed with --no-compile: {}", String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Content Root:  bin/www"), "build output must log Content Root: bin/www");
    assert!(stdout.contains("Nested payload detected. Packaging content from scoped root 'bin/www'"));

    // Verify build-manifest.json
    let manifest_bytes = fs::read(out_dir.join("build-manifest.json")).expect("read build-manifest.json");
    let manifest: BuildManifest = serde_json::from_slice(&manifest_bytes).expect("parse build manifest");
    assert_eq!(manifest.engine.content_root, Some("bin/www".to_string()), "manifest engine content_root must match");

    // Verify game.pack
    let pack_path = out_dir.join("game.pack");
    assert!(pack_path.exists(), "game.pack must exist");
    let reader = GpackReader::open(&pack_path).expect("open game.pack");
    assert_eq!(manifest.content.file_count, reader.file_count(), "manifest file_count must match GPACK reader file_count");

    // Entries in game.pack must be re-rooted: index.html at root, no outer game.exe
    assert!(reader.has_file("index.html"), "game.pack must contain index.html at root");
    assert!(reader.has_file("js/rmmz_core.js"), "game.pack must contain js/rmmz_core.js");
    assert!(!reader.has_file("bin/www/index.html"), "game.pack must not contain bin/www/ prefix");
    assert!(!reader.has_file("game.exe"), "game.pack must exclude outer desktop game.exe");
    assert!(!reader.has_file("nw.dll"), "game.pack must exclude outer desktop nw.dll");
}

#[test]
fn test_cli_regression_matrix_all_engines() {
    let cases = [
        ("xp_min", "RPG Maker XP", "<root>"),
        ("vx_min", "RPG Maker VX", "<root>"),
        ("vxace_min", "RPG Maker VX Ace", "<root>"),
        ("renpy_v8_min", "Ren'Py", "<root>"),
        ("tyrano_min", "TyranoBuilder / TyranoScript", "<root>"),
    ];

    for (fixture_name, expected_engine, expected_root) in cases {
        let fpath = fixture(fixture_name);
        if !fpath.exists() {
            continue;
        }
        let output = run_cli(&["inspect", fpath.to_str().unwrap()]);
        assert!(output.status.success(), "inspect {} must succeed", fixture_name);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(&format!("Engine:         {}", expected_engine)), "fixture {} must detect {}", fixture_name, expected_engine);
        assert!(stdout.contains(&format!("Content Root:   {}", expected_root)), "fixture {} must report content root {}", fixture_name, expected_root);
    }

    // VNM regression
    let vnm_dir = create_vnm_fixture();
    let output_vnm = run_cli(&["inspect", vnm_dir.to_str().unwrap()]);
    assert!(output_vnm.status.success(), "inspect vnm must succeed");
    let stdout_vnm = String::from_utf8_lossy(&output_vnm.stdout);
    assert!(stdout_vnm.contains("Engine:         Visual Novel Maker"));
    assert!(stdout_vnm.contains("Content Root:   <root>"));
}

#[test]
fn test_cli_build_output_inside_or_ancestor_of_source_fails_bld101() {
    let game_dir = create_nested_mz_fixture();

    // 1. Output inside game root must fail closed with [BLD-101] before any mutation
    let inside_out = game_dir.join("nested_out");
    let output_inside = run_cli(&[
        "build",
        game_dir.to_str().unwrap(),
        "--output",
        inside_out.to_str().unwrap(),
        "--no-compile",
    ]);
    assert!(!output_inside.status.success(), "output inside source must fail");
    let stderr_inside = String::from_utf8_lossy(&output_inside.stderr);
    assert!(stderr_inside.contains("[BLD-101]"), "must cite BLD-101 for output inside source");
    assert!(!inside_out.exists(), "must not create output directory inside source game");

    // 2. Output equal to game root must fail closed with [BLD-101]
    let output_equal = run_cli(&[
        "build",
        game_dir.to_str().unwrap(),
        "--output",
        game_dir.to_str().unwrap(),
        "--no-compile",
    ]);
    assert!(!output_equal.status.success(), "output equal to source must fail");
    let stderr_equal = String::from_utf8_lossy(&output_equal.stderr);
    assert!(stderr_equal.contains("[BLD-101]"), "must cite BLD-101 for output equal to source");
}

#[test]
fn test_cli_build_relative_output_dist_from_game_cwd_fails_bld101_and_preserves_immutability() {
    let game_dir = create_nested_mz_fixture();
    let nested_dist = game_dir.join("dist");

    // Run CLI with current_dir = game_dir, input = ".", output = "dist"
    let output_bad = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .current_dir(&game_dir)
        .args(&["build", ".", "--output", "dist", "--no-compile"])
        .output()
        .expect("execute runtimepack CLI binary");

    let stderr_bad = String::from_utf8_lossy(&output_bad.stderr);
    assert!(
        !output_bad.status.success(),
        "build with relative output 'dist' inside cwd game must fail; stderr:\n{}",
        stderr_bad
    );
    assert!(
        stderr_bad.contains("[BLD-101]"),
        "must cite BLD-101 error code, got stderr:\n{}",
        stderr_bad
    );
    assert!(
        !nested_dist.exists(),
        "input immutability violated: 'dist' directory was created inside source game!"
    );

    // Sibling relative output '../valid_dist' from cwd game must be permitted and not fail with false disjointness error
    let sibling_rel = "../valid_dist_output_task4";
    let output_good = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .current_dir(&game_dir)
        .args(&[
            "build",
            ".",
            "--output",
            sibling_rel,
            "--no-compile",
            "--flavor",
            "beta",
            "--accept-audit-fail",
            "--accept-audit-unknown",
        ])
        .output()
        .expect("execute runtimepack CLI binary");

    assert!(
        output_good.status.success(),
        "sibling output outside cwd game must succeed, got stderr:\n{}",
        String::from_utf8_lossy(&output_good.stderr)
    );
}

#[test]
fn test_cli_build_symlink_parent_dir_bypass_fails_bld101_and_preserves_immutability() {
    let game_dir = create_nested_mz_fixture();
    let outside_dir = temp_dir("outside_bypass_cli");
    let link_to_game = outside_dir.join("link_to_game");

    #[cfg(unix)]
    std::os::unix::fs::symlink(&game_dir, &link_to_game).unwrap();
    #[cfg(windows)]
    let _ = std::os::windows::fs::symlink_dir(&game_dir, &link_to_game);

    if !link_to_game.exists() && fs::symlink_metadata(&link_to_game).is_err() {
        return;
    }

    let game_name = game_dir.file_name().unwrap().to_str().unwrap();
    let illicit_sub = "cli_illicit_output";
    let bypass_output = outside_dir.join(format!("link_to_game/../{}/{}", game_name, illicit_sub));

    let output = run_cli(&[
        "build",
        game_dir.to_str().unwrap(),
        "--output",
        bypass_output.to_str().unwrap(),
        "--no-compile",
    ]);

    assert!(
        !output.status.success(),
        "Symlink parent-dir bypass pointing into game must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[BLD-101]"),
        "must cite BLD-101, got: {}",
        stderr
    );
    assert!(
        !game_dir.join(illicit_sub).exists(),
        "Immutability violated: directory was created inside source game"
    );
}

fn stream_file_sha256(path: &Path) -> Result<String, std::io::Error> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn compute_tree_hashes(dir: &Path) -> Result<BTreeMap<PathBuf, String>, std::io::Error> {
    let mut map = BTreeMap::new();
    fn walk(base: &Path, curr: &Path, acc: &mut BTreeMap<PathBuf, String>) -> Result<(), std::io::Error> {
        let entries = fs::read_dir(curr)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                walk(base, &path, acc)?;
            } else if file_type.is_file() {
                let rel = path
                    .strip_prefix(base)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
                    .to_path_buf();
                let hash = stream_file_sha256(&path)?;
                acc.insert(rel, hash);
            } else if file_type.is_symlink() {
                let target = fs::canonicalize(&path)?;
                if !target.starts_with(base) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        format!("Symlink escape detected at {}", path.display()),
                    ));
                }
                if target.is_dir() {
                    walk(base, &target, acc)?;
                } else {
                    let rel = path
                        .strip_prefix(base)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
                        .to_path_buf();
                    let hash = stream_file_sha256(&target)?;
                    acc.insert(rel, hash);
                }
            }
        }
        Ok(())
    }
    walk(dir, dir, &mut map)?;
    Ok(map)
}

fn validate_bidirectional_disjoint_output(game_root: &Path, out_dir: &Path) -> Result<(), String> {
    runtimepack_core::build_plan::validate_disjoint_output_dir(game_root, out_dir)
        .map_err(|e| e.to_string())
}

#[test]
#[ignore = "Opt-in real game QA harness; run with RUNTIMEPACK_TEST_GAME_ROOT=<path> cargo test ... -- --ignored"]
fn test_opt_in_real_game_qa_harness() {
    let game_root_str = match std::env::var("RUNTIMEPACK_TEST_GAME_ROOT") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            println!("RUNTIMEPACK_TEST_GAME_ROOT not set; skipping opt-in real game QA harness.");
            return;
        }
    };

    let game_root = PathBuf::from(&game_root_str);
    assert!(game_root.exists(), "RUNTIMEPACK_TEST_GAME_ROOT path must exist: {}", game_root.display());

    let out_dir = match std::env::var("RUNTIMEPACK_TEST_OUTPUT_ROOT") {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v),
        _ => temp_dir("real_game_qa_out"),
    };

    // Pre-creation validation: validate bidirectional disjointness BEFORE creating directory
    validate_bidirectional_disjoint_output(&game_root, &out_dir)
        .expect("output directory must be bidirectionally disjoint from immutable game source root");
    fs::create_dir_all(&out_dir).expect("create QA output directory");

    let expected_content_root = std::env::var("RUNTIMEPACK_TEST_EXPECTED_CONTENT_ROOT")
        .unwrap_or_else(|_| "bin/www".to_string());

    // 1. Take pre-inspection SHA-256 snapshot for immutability verification
    let pre_hashes = compute_tree_hashes(&game_root).expect("compute pre-inspection tree hashes without IO error");
    println!("Pre-test inventory: {} files in source tree.", pre_hashes.len());

    // 2. Run CLI inspect
    let inspect_out = run_cli(&["inspect", game_root.to_str().unwrap()]);
    assert!(inspect_out.status.success(), "real game inspect must succeed: {}", String::from_utf8_lossy(&inspect_out.stderr));
    let inspect_stdout = String::from_utf8_lossy(&inspect_out.stdout);
    println!("Inspect output:\n{}", inspect_stdout);

    // Assert exact MZ marker detection
    assert!(inspect_stdout.contains("Engine:         RPG Maker MZ"), "Real game must detect RPG Maker MZ");
    assert!(inspect_stdout.contains("Confidence:     EXACT"), "Real game must detect Exact confidence");
    assert!(inspect_stdout.contains("Score:          210"), "Real game must score 210");
    assert!(inspect_stdout.contains(&format!("Content Root:   {}", expected_content_root)),
        "Real game content root must be {}", expected_content_root);

    // 3. Run CLI explain
    let explain_out = run_cli(&["explain", game_root.to_str().unwrap()]);
    assert!(explain_out.status.success(), "real game explain must succeed");
    let explain_stdout = String::from_utf8_lossy(&explain_out.stdout);
    assert!(explain_stdout.contains(&format!("root: {}", expected_content_root)));
    assert!(explain_stdout.contains("[SELECTED]"));

    // 4. Run CLI build with --no-compile
    let build_out = run_cli(&[
        "build",
        game_root.to_str().unwrap(),
        "--output",
        out_dir.to_str().unwrap(),
        "--no-compile",
        "--flavor",
        "beta",
        "--accept-audit-fail",
        "--accept-audit-unknown",
    ]);
    assert!(build_out.status.success(), "real game build must succeed: {}", String::from_utf8_lossy(&build_out.stderr));

    // 5. Verify packaging results
    let pack_path = out_dir.join("game.pack");
    assert!(pack_path.exists(), "game.pack must exist");
    let reader = GpackReader::open(&pack_path).expect("open packaged game.pack");
    assert!(reader.has_file("index.html"), "game.pack must contain index.html at root");
    assert!(reader.has_file("js/rmmz_core.js"), "game.pack must contain js/rmmz_core.js");
    assert!(!reader.has_file(&format!("{}/index.html", expected_content_root)),
        "game.pack entries must not have scoped prefix");

    // 6. Verify source immutability
    let post_hashes = compute_tree_hashes(&game_root).expect("compute post-inspection tree hashes without IO error");
    assert_eq!(pre_hashes.len(), post_hashes.len(), "Source file count must remain 100% identical");
    assert_eq!(pre_hashes, post_hashes, "Source file digests must remain 100% byte-identical (input immutability violated if different)");
    println!("Source immutability verified: 100% byte identical across {} files.", post_hashes.len());
}

// =============================================================================
// D1 shared integration — validated engine override and manifest provenance
//
// Shape: a deployed MZ payload that still carries the dead MV core set, whose
// live entrypoint chain (`index.html` -> `js/main.js` -> literal `scriptUrls`
// consumed by the recognised loader) proves MZ only.
// =============================================================================

fn create_stale_mv_cores_mz_payload() -> PathBuf {
    let dir = temp_dir("g06_shape");
    write_file(&dir, "index.html", b"<!DOCTYPE html><html><head><script type=\"text/javascript\" src=\"js/main.js\"></script></head><body></body></html>");
    write_file(
        &dir,
        "js/main.js",
        concat!(
            "const scriptUrls = [\n",
            "    \"js/libs/pixi.js\",\n",
            "    \"js/rmmz_core.js\",\n",
            "    \"js/rmmz_managers.js\",\n",
            "    \"js/plugins.js\"\n",
            "];\n",
            "class Main {\n",
            "    run() {\n",
            "        this.loadMainScripts();\n",
            "    }\n",
            "    loadMainScripts() {\n",
            "        for (const url of scriptUrls) {\n",
            "            const script = document.createElement(\"script\");\n",
            "            script.src = url;\n",
            "            document.body.appendChild(script);\n",
            "        }\n",
            "    }\n",
            "}\n",
            "const main = new Main();\n",
            "main.run();\n"
        )
        .as_bytes(),
    );
    // Stale MV leftovers: present on disk, never loaded by the live chain.
    write_file(&dir, "js/rpg_core.js", b"// stale MV core");
    write_file(&dir, "js/rpg_managers.js", b"// stale MV managers");
    write_file(&dir, "js/rmmz_core.js", b"// MZ core engine");
    write_file(&dir, "js/rmmz_managers.js", b"// MZ managers");
    write_file(&dir, "data/System.json", b"{\"gameTitle\":\"Synthetic stale-core MZ\"}");
    write_file(&dir, "package.json", b"{\"name\":\"rmmz-game\",\"main\":\"index.html\"}");
    dir
}

fn build_args<'a>(game: &'a str, out: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec![
        "build",
        game,
        "--output",
        out,
        "--no-compile",
        "--flavor",
        "beta",
        "--accept-audit-fail",
        "--accept-audit-unknown",
    ];
    args.extend_from_slice(extra);
    args
}

fn read_manifest(out_dir: &Path) -> BuildManifest {
    let bytes = fs::read(out_dir.join("build-manifest.json")).expect("read build-manifest.json");
    serde_json::from_slice(&bytes).expect("parse build manifest")
}

#[test]
fn test_cli_build_stale_core_payload_auto_detects_mz_without_override() {
    let game_dir = create_stale_mv_cores_mz_payload();
    let out_dir = temp_dir("g06_auto_out");

    let output = run_cli(&build_args(
        game_dir.to_str().unwrap(),
        out_dir.to_str().unwrap(),
        &[],
    ));
    assert!(
        output.status.success(),
        "auto-detected stale-core MZ payload must build: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest = read_manifest(&out_dir);
    assert_eq!(manifest.engine.id, "RPG Maker MZ");
    assert_eq!(manifest.engine.confidence, "EXACT");
    assert!(
        !manifest.engine.overridden,
        "automatic selection must record overridden=false"
    );
    assert_eq!(manifest.engine.content_root, None);
    assert!(
        manifest
            .engine
            .evidence
            .iter()
            .any(|e| e.rule == "DET-ARB-MZ-MAIN"),
        "manifest must record the live entrypoint evidence, got: {:?}",
        manifest.engine.evidence
    );
    assert!(
        manifest
            .engine
            .evidence
            .iter()
            .any(|e| e.rule == "DET-ARB-EXCLUDED-CORE"),
        "manifest must record the stale MV core exclusion evidence, got: {:?}",
        manifest.engine.evidence
    );
    assert!(
        manifest.engine.conflicts.is_empty(),
        "automatic selection must not record conflicts, got: {:?}",
        manifest.engine.conflicts
    );

    // The manifest must round-trip through its own schema/serde contract.
    let reserialized = serde_json::to_string(&manifest).expect("serialize manifest");
    assert!(reserialized.contains("\"evidence\""), "engine evidence must serialize");
}

#[test]
fn test_cli_build_forced_mz_on_stale_core_payload_records_override() {
    let game_dir = create_stale_mv_cores_mz_payload();
    let out_dir = temp_dir("g06_forced_mz_out");

    let output = run_cli(&build_args(
        game_dir.to_str().unwrap(),
        out_dir.to_str().unwrap(),
        &["--engine", "rpgmaker-mz"],
    ));
    assert!(
        output.status.success(),
        "forced MZ on a live-MZ payload must build: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest = read_manifest(&out_dir);
    assert_eq!(manifest.engine.id, "RPG Maker MZ");
    assert!(
        manifest.engine.overridden,
        "explicit --engine selection must record overridden=true"
    );
    assert_eq!(manifest.engine.content_root, None);
    assert!(
        manifest
            .engine
            .evidence
            .iter()
            .any(|e| e.rule == "DET-ARB-MZ-MAIN"),
        "forced selection must still carry the liveness evidence, got: {:?}",
        manifest.engine.evidence
    );
    assert!(
        manifest.engine.evidence.iter().all(|e| !e.description.is_empty()),
        "evidence descriptions must never be fabricated empty strings"
    );
}

#[test]
fn test_cli_build_forced_stale_mv_rejected_det101_without_output() {
    let game_dir = create_stale_mv_cores_mz_payload();
    let out_dir = temp_dir("g06_forced_mv_out");

    let output = run_cli(&build_args(
        game_dir.to_str().unwrap(),
        out_dir.to_str().unwrap(),
        &["--engine", "rpgmaker-mv"],
    ));
    assert!(
        !output.status.success(),
        "forced MV contradicted by live MZ evidence must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[DET-101]"),
        "rejection must be reported as [DET-101], got:\n{stderr}"
    );
    assert!(
        !stderr.contains("[DET-110]"),
        "validated override rejection must not be reported as ambiguity:\n{stderr}"
    );
    assert!(
        !out_dir.join("build-manifest.json").exists(),
        "rejected override must not produce a build manifest"
    );
    assert!(
        !out_dir.join("game.pack").exists(),
        "rejected override must not package content"
    );
}

#[test]
fn test_cli_build_forced_family_without_evidence_rejected_det101() {
    let game_dir = create_stale_mv_cores_mz_payload();
    let out_dir = temp_dir("g06_forced_tyrano_out");

    let output = run_cli(&build_args(
        game_dir.to_str().unwrap(),
        out_dir.to_str().unwrap(),
        &["--engine", "tyrano"],
    ));
    assert!(
        !output.status.success(),
        "forced family without positive evidence must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[DET-101]"),
        "no-evidence forced family must cite [DET-101], got:\n{stderr}"
    );
    assert!(
        !out_dir.join("build-manifest.json").exists(),
        "no-evidence forced family must not produce a build manifest"
    );
}

#[test]
fn test_cli_build_forced_medium_requires_accept_detection_flag() {
    let game_dir = temp_dir("mv_medium_forced");
    write_file(&game_dir, "index.html", b"<!DOCTYPE html><html><body></body></html>");
    write_file(&game_dir, "js/rpg_managers.js", b"// MV managers only");
    write_file(&game_dir, "data/System.json", b"{\"gameTitle\":\"Medium\"}");

    let rejected_out = temp_dir("mv_medium_rejected_out");
    let rejected = run_cli(&build_args(
        game_dir.to_str().unwrap(),
        rejected_out.to_str().unwrap(),
        &["--engine", "rpgmaker-mv"],
    ));
    assert!(
        !rejected.status.success(),
        "forced MEDIUM without --accept-detection must fail"
    );
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(
        stderr.contains("[DET-102]"),
        "forced MEDIUM must cite [DET-102], got:\n{stderr}"
    );
    assert!(
        !rejected_out.join("build-manifest.json").exists(),
        "rejected MEDIUM must not produce a build manifest"
    );

    let accepted_out = temp_dir("mv_medium_accepted_out");
    let accepted = run_cli(&build_args(
        game_dir.to_str().unwrap(),
        accepted_out.to_str().unwrap(),
        &["--engine", "rpgmaker-mv", "--accept-detection"],
    ));
    assert!(
        accepted.status.success(),
        "forced MEDIUM with --accept-detection must build: {}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    let manifest = read_manifest(&accepted_out);
    assert_eq!(manifest.engine.id, "RPG Maker MV");
    assert!(manifest.engine.overridden);
}

#[test]
fn test_cli_build_forced_family_with_conflicting_live_chain_stays_det110() {
    // Both families are named by the live chain: the override must not claim a
    // contradiction (that would be a misleading DET-101) and must not select.
    let game_dir = temp_dir("d1_conflicting_chain");
    write_file(
        &game_dir,
        "index.html",
        b"<!DOCTYPE html><html><head><script type=\"text/javascript\" src=\"js/main.js\"></script></head><body></body></html>",
    );
    write_file(
        &game_dir,
        "js/main.js",
        b"const scriptUrls = [\"js/rpg_core.js\", \"js/rmmz_core.js\"];\nfor (const url of scriptUrls) {\n    script.src = url;\n}\n",
    );
    write_file(&game_dir, "js/rpg_core.js", b"// MV core");
    write_file(&game_dir, "js/rpg_managers.js", b"// MV managers");
    write_file(&game_dir, "js/rmmz_core.js", b"// MZ core");
    write_file(&game_dir, "js/rmmz_managers.js", b"// MZ managers");
    write_file(&game_dir, "data/System.json", b"{}");

    let out_dir = temp_dir("d1_conflicting_chain_out");
    let output = run_cli(&build_args(
        game_dir.to_str().unwrap(),
        out_dir.to_str().unwrap(),
        &["--engine", "rpgmaker-mz"],
    ));
    assert!(
        !output.status.success(),
        "conflicting live chain must not build even when forced"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[DET-110]"),
        "conflicting evidence must stay ambiguity, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("[DET-101]"),
        "conflicting evidence must not be reported as an invalid override, got:\n{stderr}"
    );
    assert!(
        !out_dir.join("build-manifest.json").exists(),
        "conflicting evidence must not produce a build manifest"
    );
}
