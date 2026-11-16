use runtimepack_core::compatibility::profile::{
    ByteRange, CompatibilityPatch, CompatibilityProfile, TargetAnchor,
};
use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::domain::{CompatibilityStatus, EngineId};
use runtimepack_core::ingest::staging::stage_and_apply_profile;
use runtimepack_core::ingest::GameSourceIndex;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

fn make_temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "runtimepack_staging_test_{}_{}_{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn compute_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn test_staging_and_apply_profile_success() {
    let _guard = std::env::set_var("RUNTIMEPACK_SKIP_DISK_CHECK", "1");

    let src_dir = make_temp_dir("src_ok");
    let out_dir = make_temp_dir("out_ok");

    let payload = src_dir.join("bin/www");
    fs::create_dir_all(payload.join("js/plugins")).unwrap();
    fs::write(payload.join("index.html"), "<!DOCTYPE html>").unwrap();
    fs::write(payload.join("js/rmmz_core.js"), "// core").unwrap();

    let orig_plugin = b"function test() {\n\tconst cp = require('child_process');\n\tcp.exec();\n}\n";
    let orig_file_sha = compute_sha256(orig_plugin);
    let orig_len = orig_plugin.len() as u64;

    // Fragment: "\n\tconst cp = require('child_process');\n\tcp.exec();\n"
    let frag_start = 17; // after "function test() {"
    let frag_end = 69;   // before "}\n"
    let fragment = &orig_plugin[frag_start..frag_end];
    let frag_sha = compute_sha256(fragment);

    let replacement = "\n\tconsole.error('[RT-DESKTOP-DISABLED]');\n\tthrow new Error('disabled');\n";
    let mut expected_post = Vec::new();
    expected_post.extend_from_slice(&orig_plugin[..frag_start]);
    expected_post.extend_from_slice(replacement.as_bytes());
    expected_post.extend_from_slice(&orig_plugin[frag_end..]);
    let post_sha = compute_sha256(&expected_post);

    fs::write(payload.join("js/plugins/TestPlugin.js"), orig_plugin).unwrap();

    let profile = CompatibilityProfile {
        schema_version: 1,
        profile_id: "test-staging-profile".to_string(),
        engine_id: Some(EngineId::RpgMakerMz),
        capability_id: Some("CAP-WEB-028".to_string()),
        capabilities: Vec::new(),
        status: CompatibilityStatus::Degraded,
        diagnostic_code: Some("CMP-107".to_string()),
        description: Some("Test staging transformation".to_string()),
        patches: vec![CompatibilityPatch {
            target_anchor: TargetAnchor::Payload,
            logical_path: "js/plugins/TestPlugin.js".to_string(),
            expected_file_size: orig_len,
            expected_file_sha256: orig_file_sha.clone(),
            range: ByteRange {
                start: frag_start,
                end: frag_end,
            },
            expected_fragment_sha256: frag_sha,
            replacement: replacement.to_string(),
            result_file_sha256: post_sha.clone(),
        }],
    };

    let src_index = GameSourceIndex::build(&src_dir).unwrap();
    let scoped_src = src_index.scoped_to("bin/www").unwrap();

    let result = stage_and_apply_profile(&scoped_src, scoped_src.content_root(), &out_dir, &profile).unwrap();

    // 1. Original source game file MUST remain 100% UNTOUCHED
    let src_bytes_after = fs::read(payload.join("js/plugins/TestPlugin.js")).unwrap();
    assert_eq!(src_bytes_after, orig_plugin, "Source game must be 100% immutable");

    // 2. Staged file contains replacement
    let staged_file = result.staging_path.join("bin/www/js/plugins/TestPlugin.js");
    let staged_bytes = fs::read(&staged_file).unwrap();
    assert_eq!(staged_bytes, expected_post, "Staged file must match expected patched bytes");
    assert_eq!(compute_sha256(&staged_bytes), post_sha);

    // 3. Staged index provenance
    assert_eq!(
        result.staged_index.original_source_hash,
        Some(scoped_src.source_hash.clone()),
        "Staged index must record original source hash for provenance"
    );
    assert_eq!(result.staged_index.content_root(), None);
    let scoped_staged = result.staged_index.scoped_to("bin/www").unwrap();
    assert_eq!(scoped_staged.content_root(), Some("bin/www"));
}

#[test]
fn test_staging_fails_on_pre_patch_hash_mismatch() {
    let _guard = std::env::set_var("RUNTIMEPACK_SKIP_DISK_CHECK", "1");

    let src_dir = make_temp_dir("src_mismatch");
    let out_dir = make_temp_dir("out_mismatch");

    let payload = src_dir.join("bin/www");
    fs::create_dir_all(payload.join("js/plugins")).unwrap();
    fs::write(payload.join("index.html"), "<!DOCTYPE html>").unwrap();
    fs::write(payload.join("js/plugins/TestPlugin.js"), b"modified content").unwrap();

    let profile = CompatibilityProfile {
        schema_version: 1,
        profile_id: "test-mismatch-profile".to_string(),
        engine_id: Some(EngineId::RpgMakerMz),
        capability_id: Some("CAP-WEB-028".to_string()),
        capabilities: Vec::new(),
        status: CompatibilityStatus::Degraded,
        diagnostic_code: Some("CMP-107".to_string()),
        description: None,
        patches: vec![CompatibilityPatch {
            target_anchor: TargetAnchor::Payload,
            logical_path: "js/plugins/TestPlugin.js".to_string(),
            expected_file_size: 16,
            expected_file_sha256: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            range: ByteRange { start: 0, end: 5 },
            expected_fragment_sha256: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            replacement: "safe".to_string(),
            result_file_sha256: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        }],
    };

    let src_index = GameSourceIndex::build(&src_dir).unwrap();
    let scoped_src = src_index.scoped_to("bin/www").unwrap();

    let err = stage_and_apply_profile(&scoped_src, scoped_src.content_root(), &out_dir, &profile).unwrap_err();
    assert_eq!(err.code(), Some(DiagnosticCode::CMP_PROFILE_INVALID));

    // Staging directory should NOT have been created since preflight failed!
    assert!(!out_dir.join("staging").exists(), "Preflight failure must abort before staging creation");
}

#[test]
fn test_staging_fails_on_existing_non_empty_staging_dir() {
    let _guard = std::env::set_var("RUNTIMEPACK_SKIP_DISK_CHECK", "1");

    let src_dir = make_temp_dir("src_existing");
    let out_dir = make_temp_dir("out_existing");

    let payload = src_dir.join("bin/www");
    fs::create_dir_all(payload.join("js/plugins")).unwrap();
    fs::write(payload.join("index.html"), "<!DOCTYPE html>").unwrap();
    let orig_plugin = b"function test() {}";
    fs::write(payload.join("js/plugins/TestPlugin.js"), orig_plugin).unwrap();

    // Create existing non-empty staging directory
    let existing_staging = out_dir.join("staging");
    fs::create_dir_all(&existing_staging).unwrap();
    fs::write(existing_staging.join("user_file.txt"), b"do not overwrite").unwrap();

    let profile = CompatibilityProfile {
        schema_version: 1,
        profile_id: "test-existing".to_string(),
        engine_id: Some(EngineId::RpgMakerMz),
        capability_id: Some("CAP-WEB-028".to_string()),
        capabilities: Vec::new(),
        status: CompatibilityStatus::Degraded,
        diagnostic_code: Some("CMP-107".to_string()),
        description: None,
        patches: vec![CompatibilityPatch {
            target_anchor: TargetAnchor::Payload,
            logical_path: "js/plugins/TestPlugin.js".to_string(),
            expected_file_size: orig_plugin.len() as u64,
            expected_file_sha256: compute_sha256(orig_plugin),
            range: ByteRange { start: 0, end: 8 },
            expected_fragment_sha256: compute_sha256(&orig_plugin[0..8]),
            replacement: "replaced".to_string(),
            result_file_sha256: compute_sha256(b"replaced test() {}"),
        }],
    };

    let src_index = GameSourceIndex::build(&src_dir).unwrap();
    let scoped_src = src_index.scoped_to("bin/www").unwrap();

    let err = stage_and_apply_profile(&scoped_src, scoped_src.content_root(), &out_dir, &profile).unwrap_err();
    assert_eq!(err.code(), Some(DiagnosticCode::CNT_TRAVERSAL_REJECTED));
}

#[test]
fn test_subject_game_profile_file_is_valid() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = if manifest_dir.ends_with("core") {
        manifest_dir.parent().unwrap().to_path_buf()
    } else {
        manifest_dir
    };
    let profile_path = workspace_root.join("compatibility-db/profiles/rpgmaker-mz-shiroin-saveui-disable-childproc.json");
    let profile = CompatibilityProfile::load_from_file(&profile_path).expect("Subject game profile must load and validate");
    assert_eq!(profile.profile_id, "rpgmaker-mz-shiroin-saveui-disable-childproc");
    assert_eq!(profile.capability_id.as_deref(), Some("CAP-WEB-028"));
    assert_eq!(profile.status, CompatibilityStatus::Degraded);
    assert_eq!(profile.patches.len(), 1);
    assert_eq!(profile.patches[0].logical_path, "js/plugins/Shiroin_SaveUI_MZ.js");
}

#[test]
fn test_staging_fails_on_escaping_symlink() {
    let _guard = std::env::set_var("RUNTIMEPACK_SKIP_DISK_CHECK", "1");

    let src_dir = make_temp_dir("src_symlink");
    let outside_dir = make_temp_dir("outside_symlink");
    let out_dir = make_temp_dir("out_symlink");

    let payload = src_dir.join("bin/www");
    fs::create_dir_all(payload.join("js/plugins")).unwrap();
    fs::write(payload.join("index.html"), "<!DOCTYPE html>").unwrap();
    fs::write(payload.join("js/rmmz_core.js"), "// core").unwrap();
    let orig_plugin = b"function test() {}";
    fs::write(payload.join("js/plugins/TestPlugin.js"), orig_plugin).unwrap();

    // Create outside target file
    let outside_file = outside_dir.join("secret.txt");
    fs::write(&outside_file, b"secret").unwrap();

    // Create escaping symlink inside source
    #[cfg(unix)]
    {
        let symlink_path = src_dir.join("escaping_link.txt");
        let _ = std::os::unix::fs::symlink(&outside_file, &symlink_path);
    }

    let profile = CompatibilityProfile {
        schema_version: 1,
        profile_id: "test-symlink".to_string(),
        engine_id: Some(EngineId::RpgMakerMz),
        capability_id: Some("CAP-WEB-028".to_string()),
        capabilities: Vec::new(),
        status: CompatibilityStatus::Degraded,
        diagnostic_code: Some("CMP-107".to_string()),
        description: None,
        patches: vec![CompatibilityPatch {
            target_anchor: TargetAnchor::Payload,
            logical_path: "js/plugins/TestPlugin.js".to_string(),
            expected_file_size: orig_plugin.len() as u64,
            expected_file_sha256: compute_sha256(orig_plugin),
            range: ByteRange { start: 0, end: 8 },
            expected_fragment_sha256: compute_sha256(&orig_plugin[0..8]),
            replacement: "replaced".to_string(),
            result_file_sha256: compute_sha256(b"replaced test() {}"),
        }],
    };

    // If symlink was created on unix, either GameSourceIndex build or staging fails closed with ING_SYMLINK_ESCAPE
    #[cfg(unix)]
    {
        let src_index_res = GameSourceIndex::build(&src_dir);
        if let Ok(src_index) = src_index_res {
            let res = stage_and_apply_profile(&src_index, src_index.content_root(), &out_dir, &profile);
            assert!(res.is_err());
            assert_eq!(res.unwrap_err().code(), Some(DiagnosticCode::ING_SYMLINK_ESCAPE));
        } else {
            assert_eq!(src_index_res.unwrap_err().code(), Some(DiagnosticCode::ING_SYMLINK_ESCAPE));
        }
    }
}

#[test]
fn test_staging_payload_anchor_does_not_shadow_root_file() {
    let _guard = std::env::set_var("RUNTIMEPACK_SKIP_DISK_CHECK", "1");

    let src_dir = make_temp_dir("src_shadow");
    let out_dir = make_temp_dir("out_shadow");

    // Root file
    fs::create_dir_all(src_dir.join("js/plugins")).unwrap();
    let root_bytes = b"function rootShared() { return 'ROOT'; }\n";
    fs::write(src_dir.join("js/plugins/Shared.js"), root_bytes).unwrap();

    // Payload nested files
    let payload = src_dir.join("bin/www");
    fs::create_dir_all(payload.join("js/plugins")).unwrap();
    fs::write(payload.join("index.html"), "<!DOCTYPE html>").unwrap();
    fs::write(payload.join("js/rmmz_core.js"), "// core").unwrap();

    let payload_bytes = b"function payloadShared() { return 'PAYLOAD'; }\n";
    fs::write(payload.join("js/plugins/Shared.js"), payload_bytes).unwrap();

    let payload_sha = compute_sha256(payload_bytes);
    let frag_start = 27; // after "function payloadShared() { "
    let frag_end = 43;   // before " }\n"
    let fragment = &payload_bytes[frag_start..frag_end];
    let frag_sha = compute_sha256(fragment);

    let replacement = "return 'PATCHED'";
    let mut post_bytes = Vec::new();
    post_bytes.extend_from_slice(&payload_bytes[..frag_start]);
    post_bytes.extend_from_slice(replacement.as_bytes());
    post_bytes.extend_from_slice(&payload_bytes[frag_end..]);
    let post_sha = compute_sha256(&post_bytes);

    // Profile declares payload anchor for js/plugins/Shared.js
    let profile = CompatibilityProfile {
        schema_version: 1,
        profile_id: "test-shadow-profile".to_string(),
        engine_id: Some(EngineId::RpgMakerMz),
        capability_id: Some("CAP-WEB-028".to_string()),
        capabilities: Vec::new(),
        status: CompatibilityStatus::Degraded,
        diagnostic_code: Some("CMP-107".to_string()),
        description: Some("Target payload without root shadow".to_string()),
        patches: vec![CompatibilityPatch {
            target_anchor: TargetAnchor::Payload,
            logical_path: "js/plugins/Shared.js".to_string(),
            expected_file_size: payload_bytes.len() as u64,
            expected_file_sha256: payload_sha,
            range: ByteRange { start: frag_start, end: frag_end },
            expected_fragment_sha256: frag_sha,
            replacement: replacement.to_string(),
            result_file_sha256: post_sha,
        }],
    };

    let src_index = GameSourceIndex::build(&src_dir).unwrap();
    let res = stage_and_apply_profile(&src_index, Some("bin/www"), &out_dir, &profile).unwrap();

    // Verify root file is unchanged
    let root_after = fs::read(src_dir.join("js/plugins/Shared.js")).unwrap();
    assert_eq!(root_after, root_bytes, "Root file must remain untouched");

    // Verify staged payload file received the patch
    let staged_payload = res.staging_path.join("bin/www/js/plugins/Shared.js");
    let staged_payload_bytes = fs::read(&staged_payload).unwrap();
    assert_eq!(staged_payload_bytes, post_bytes, "Payload file must receive patch");

    // Verify staged root file still has original root content
    let staged_root = res.staging_path.join("js/plugins/Shared.js");
    let staged_root_bytes = fs::read(&staged_root).unwrap();
    assert_eq!(staged_root_bytes, root_bytes, "Staged root file must NOT receive payload patch");
}

#[test]
fn test_staging_fails_on_duplicate_patch_targets() {
    let _guard = std::env::set_var("RUNTIMEPACK_SKIP_DISK_CHECK", "1");

    let src_dir = make_temp_dir("src_dup");
    let out_dir = make_temp_dir("out_dup");

    let payload = src_dir.join("bin/www");
    fs::create_dir_all(payload.join("js/plugins")).unwrap();
    fs::write(payload.join("index.html"), "<!DOCTYPE html>").unwrap();
    let sample = b"function sample() {}";
    fs::write(payload.join("js/plugins/Sample.js"), sample).unwrap();

    let patch1 = CompatibilityPatch {
        target_anchor: TargetAnchor::Payload,
        logical_path: "js/plugins/Sample.js".to_string(),
        expected_file_size: sample.len() as u64,
        expected_file_sha256: compute_sha256(sample),
        range: ByteRange { start: 0, end: 8 },
        expected_fragment_sha256: compute_sha256(&sample[0..8]),
        replacement: "replaced".to_string(),
        result_file_sha256: compute_sha256(b"replaced sample() {}"),
    };

    // Cross-anchor collision targeting the exact same file in bin/www!
    let patch2 = CompatibilityPatch {
        target_anchor: TargetAnchor::Source,
        logical_path: "bin/www/js/plugins/Sample.js".to_string(),
        expected_file_size: sample.len() as u64,
        expected_file_sha256: compute_sha256(sample),
        range: ByteRange { start: 9, end: 15 },
        expected_fragment_sha256: compute_sha256(&sample[9..15]),
        replacement: "test".to_string(),
        result_file_sha256: compute_sha256(b"function test() {}"),
    };

    let profile = CompatibilityProfile {
        schema_version: 1,
        profile_id: "test-dup".to_string(),
        engine_id: Some(EngineId::RpgMakerMz),
        capability_id: Some("CAP-WEB-028".to_string()),
        capabilities: Vec::new(),
        status: CompatibilityStatus::Degraded,
        diagnostic_code: Some("CMP-107".to_string()),
        description: None,
        patches: vec![patch1, patch2],
    };

    let src_index = GameSourceIndex::build(&src_dir).unwrap();
    let err = stage_and_apply_profile(&src_index, Some("bin/www"), &out_dir, &profile).unwrap_err();
    assert_eq!(err.code(), Some(DiagnosticCode::CMP_PROFILE_INVALID));
}

#[test]
fn test_staging_fails_disk_check_when_space_insufficient() {
    let out_dir = make_temp_dir("out_disk");
    std::env::remove_var("RUNTIMEPACK_SKIP_DISK_CHECK");

    // Asking for impossible 100 PB must fail with BLD-105
    let impossible_bytes = 100 * 1024 * 1024 * 1024 * 1024 * 1024;
    let res = runtimepack_core::ingest::staging::check_available_disk_space(&out_dir, impossible_bytes);
    #[cfg(unix)]
    {
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().code(), Some(DiagnosticCode::BLD_STORAGE_INSUFFICIENT));
    }
}

#[test]
fn test_rpgmaker_mz_desktop_subproc_external_url_profile_validates_and_applies() {
    let _guard = std::env::set_var("RUNTIMEPACK_SKIP_DISK_CHECK", "1");

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = if manifest_dir.ends_with("core") {
        manifest_dir.parent().unwrap().to_path_buf()
    } else {
        manifest_dir
    };
    let profile_path = workspace_root
        .join("compatibility-db/profiles/rpgmaker-mz-desktop-subproc-external-url.json");
    let profile = CompatibilityProfile::load_from_file(&profile_path)
        .expect("compatibility profile rpgmaker-mz-desktop-subproc-external-url.json must exist and validate");

    assert_eq!(profile.profile_id, "rpgmaker-mz-desktop-subproc-external-url");
    assert_eq!(profile.engine_id, Some(EngineId::RpgMakerMz));
    assert_eq!(profile.capability_id, Some("CAP-WEB-030".to_string()));
    assert_eq!(profile.collected_capabilities(), vec!["CAP-WEB-028", "CAP-WEB-030"]);
    assert_eq!(profile.status, CompatibilityStatus::Degraded);
    assert_eq!(profile.patches.len(), 5, "Profile must contain exactly 5 patches");

    // Verify 5th patch specifically (QJ-MPMZLib-AnsuzEffects.js)
    let p5 = &profile.patches[4];
    assert_eq!(p5.target_anchor, TargetAnchor::Payload);
    assert_eq!(p5.logical_path, "js/plugins/QJ-MPMZLib-AnsuzEffects.js");
    assert_eq!(p5.expected_file_size, 49343);
    assert_eq!(p5.expected_file_sha256, "9e47514d2cbb5b17f72a7d22645057d805cc03afcd23f52b3be7c8b20cd9b943");
    assert_eq!(p5.range.start, 2548);
    assert_eq!(p5.range.end, 2590);
    assert_eq!(p5.expected_fragment_sha256, "a388de60450b3bacfed65328aa296d0bfeecdd11c84ca00e775514e952c087ea");
    assert_eq!(p5.result_file_sha256, "54b38370f9271638db0080f53fc1f9f69a01045062fd28f4aa754239a9b94557");
    assert!(p5.replacement.contains("RuntimePack.openExternalUrl"));

    // Stage and apply test with deterministic synthetic buffer
    let src_dir = make_temp_dir("src_exturl_test");
    let out_dir = make_temp_dir("out_exturl_test");

    let payload = src_dir.join("bin/www");
    fs::create_dir_all(payload.join("js/plugins")).unwrap();
    fs::write(payload.join("index.html"), "<!DOCTYPE html>").unwrap();
    fs::write(payload.join("js/rmmz_core.js"), "// core").unwrap();

    let mut syn_bytes = vec![b' '; 49343];
    let frag = b"cordova.InAppBrowser.open(url, '_system');";
    syn_bytes[2548..2590].copy_from_slice(frag);
    let syn_sha = compute_sha256(&syn_bytes);

    let mut expected_post = Vec::new();
    expected_post.extend_from_slice(&syn_bytes[..2548]);
    expected_post.extend_from_slice(p5.replacement.as_bytes());
    expected_post.extend_from_slice(&syn_bytes[2590..]);
    let post_sha = compute_sha256(&expected_post);

    fs::write(payload.join("js/plugins/QJ-MPMZLib-AnsuzEffects.js"), &syn_bytes).unwrap();

    let p5_profile = CompatibilityProfile {
        schema_version: 1,
        profile_id: "test-p5-sub".to_string(),
        engine_id: Some(EngineId::RpgMakerMz),
        capability_id: Some("CAP-WEB-030".to_string()),
        capabilities: vec!["CAP-WEB-028".to_string(), "CAP-WEB-030".to_string()],
        status: CompatibilityStatus::Degraded,
        diagnostic_code: Some("CMP-107".to_string()),
        description: None,
        patches: vec![CompatibilityPatch {
            target_anchor: p5.target_anchor,
            logical_path: p5.logical_path.clone(),
            expected_file_size: p5.expected_file_size,
            expected_file_sha256: syn_sha,
            range: p5.range.clone(),
            expected_fragment_sha256: p5.expected_fragment_sha256.clone(),
            replacement: p5.replacement.clone(),
            result_file_sha256: post_sha.clone(),
        }],
    };

    let src_index = GameSourceIndex::build(&src_dir).unwrap();
    let res = stage_and_apply_profile(&src_index, Some("bin/www"), &out_dir, &p5_profile).unwrap();

    let staged_file = res.staging_path.join("bin/www/js/plugins/QJ-MPMZLib-AnsuzEffects.js");
    let staged_bytes = fs::read(&staged_file).unwrap();
    assert_eq!(compute_sha256(&staged_bytes), post_sha);
}

#[test]
fn test_profile_capabilities_metadata_driven_not_heuristic_id() {
    let mut neutral_profile = CompatibilityProfile::default();
    neutral_profile.profile_id = "neutral-profile".to_string();
    neutral_profile.capabilities = vec!["CAP-WEB-028".to_string(), "CAP-WEB-030".to_string()];
    let caps = neutral_profile.collected_capabilities();
    assert_eq!(caps, vec!["CAP-WEB-028", "CAP-WEB-030"]);

    let mut misleading_profile = CompatibilityProfile::default();
    misleading_profile.profile_id = "rpgmaker-mz-desktop-subproc-external-url".to_string();
    misleading_profile.capabilities = vec!["CAP-WEB-028".to_string()];
    // If not declared in capabilities metadata, CAP-WEB-030 must NOT be automatically registered
    let caps_misleading = misleading_profile.collected_capabilities();
    assert!(!caps_misleading.contains(&"CAP-WEB-030".to_string()));
}

#[test]
#[ignore = "Opt-in real game QA harness; run with RUNTIMEPACK_TEST_GAME_ROOT=<path> cargo test ... -- --ignored"]
fn test_opt_in_subject_game_ansuz_patch_real_bytes() {
    let game_root = match std::env::var("RUNTIMEPACK_TEST_GAME_ROOT") {
        Ok(val) => PathBuf::from(val),
        Err(_) => return,
    };
    let real_plugin = game_root.join("bin/www/js/plugins/QJ-MPMZLib-AnsuzEffects.js");
    if !real_plugin.exists() {
        return;
    }
    let real_bytes = fs::read(&real_plugin).unwrap();
    assert_eq!(real_bytes.len(), 49343);
    assert_eq!(compute_sha256(&real_bytes), "9e47514d2cbb5b17f72a7d22645057d805cc03afcd23f52b3be7c8b20cd9b943");
}
