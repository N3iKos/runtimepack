use runtimepack_core::asset_audit::{
    render_audit_html, render_canonical_artifacts, write_audit_artifacts,
    write_audit_artifacts_with_options, AssetAuditReport, AssetKind, AssetNode, AuditCache,
    AuditCacheKey, AuditCompleteness, AuditFinding, AuditLimits, AuditMode, AuditOutcome,
    AuditOverride, AuditStatistics, DependencyGraph, EvidenceStage, EvidenceTrail,
    FindingDisposition, FindingSeverity, ReferenceEdge, ReferenceRequiredness, ReferenceResolution,
    RuntimeValidationState, CURRENT_AUDIT_RULESET_VERSION,
};
use runtimepack_core::domain::EngineId;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "runtimepack-report-test-{name}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn sample_report_pair() -> (AssetAuditReport, AssetAuditReport) {
    let node_a = AssetNode {
        id: "image:Graphics/Titles/Title.png".into(),
        kind: AssetKind::Image,
        logical_path: Some("Graphics/Titles/Title.png".into()),
    };
    let node_b = AssetNode {
        id: "audio:Audio/BGM/Title.ogg".into(),
        kind: AssetKind::Audio,
        logical_path: Some("Audio/BGM/Title.ogg".into()),
    };
    let node_c = AssetNode {
        id: "database:Data/System.json".into(),
        kind: AssetKind::DatabaseEntity,
        logical_path: Some("Data/System.json".into()),
    };

    let edge_1 = ReferenceEdge {
        source: "database:Data/System.json".into(),
        target: "image:Graphics/Titles/Title.png".into(),
        relation: "title_graphic".into(),
        requiredness: ReferenceRequiredness::Required,
        evidence: EvidenceTrail::through(EvidenceStage::FileReadable),
        resolution: ReferenceResolution::Resolved,
    };
    let edge_2 = ReferenceEdge {
        source: "database:Data/System.json".into(),
        target: "audio:Audio/BGM/Title.ogg".into(),
        relation: "title_bgm".into(),
        requiredness: ReferenceRequiredness::Required,
        evidence: EvidenceTrail::through(EvidenceStage::FileReadable),
        resolution: ReferenceResolution::Resolved,
    };

    let finding_1 = AuditFinding {
        id: "finding-001".into(),
        code: "CMP-AUD-002".into(),
        severity: FindingSeverity::Error,
        disposition: FindingDisposition::Blocking,
        requiredness: ReferenceRequiredness::Required,
        message: "Required title graphic missing".into(),
        source_node_ids: vec![
            "database:Data/System.json".into(),
            "image:Graphics/Titles/Title.png".into(),
        ],
        orphan: false,
    };
    let finding_2 = AuditFinding {
        id: "finding-002".into(),
        code: "CMP-AUD-003".into(),
        severity: FindingSeverity::Warning,
        disposition: FindingDisposition::Warning,
        requiredness: ReferenceRequiredness::Optional,
        message: "Optional audio file warning".into(),
        source_node_ids: vec!["audio:Audio/BGM/Title.ogg".into()],
        orphan: false,
    };

    let report_1 = AssetAuditReport {
        schema_version: 1,
        engine: EngineId::RpgMakerMv,
        mode: AuditMode::Standard,
        outcome: AuditOutcome::Fail,
        completeness: AuditCompleteness {
            required_scope_complete: true,
            covered_reference_classes: vec!["database".into(), "audio".into(), "image".into()],
            uncovered_reference_classes: vec!["video".into(), "plugin".into()],
        },
        statistics: AuditStatistics {
            nodes: 3,
            references: 2,
            findings: 2,
        },
        graph: DependencyGraph {
            nodes: vec![node_a.clone(), node_b.clone(), node_c.clone()],
            edges: vec![edge_1.clone(), edge_2.clone()],
        },
        findings: vec![finding_1.clone(), finding_2.clone()],
        overrides: vec![
            AuditOverride {
                flag: "--accept-audit-fail".into(),
                rationale: "Dev override".into(),
            },
            AuditOverride {
                flag: "--accept-audit-unknown".into(),
                rationale: "Testing".into(),
            },
        ],
        runtime_validation: RuntimeValidationState::NotRun,
    };

    // Shuffled version with reversed collections and swapped order in arrays
    let report_2 = AssetAuditReport {
        schema_version: 1,
        engine: EngineId::RpgMakerMv,
        mode: AuditMode::Standard,
        outcome: AuditOutcome::Fail,
        completeness: AuditCompleteness {
            required_scope_complete: true,
            covered_reference_classes: vec!["image".into(), "database".into(), "audio".into()],
            uncovered_reference_classes: vec!["plugin".into(), "video".into()],
        },
        statistics: AuditStatistics {
            nodes: 3,
            references: 2,
            findings: 2,
        },
        graph: DependencyGraph {
            nodes: vec![node_c, node_a, node_b],
            edges: vec![edge_2, edge_1],
        },
        findings: vec![
            finding_2,
            AuditFinding {
                source_node_ids: vec![
                    "image:Graphics/Titles/Title.png".into(),
                    "database:Data/System.json".into(),
                ],
                ..finding_1
            },
        ],
        overrides: vec![
            AuditOverride {
                flag: "--accept-audit-unknown".into(),
                rationale: "Testing".into(),
            },
            AuditOverride {
                flag: "--accept-audit-fail".into(),
                rationale: "Dev override".into(),
            },
        ],
        runtime_validation: RuntimeValidationState::NotRun,
    };

    (report_1, report_2)
}

#[test]
fn test_canonical_artifacts_determinism_regardless_of_in_memory_order() {
    let (report_1, report_2) = sample_report_pair();

    let artifacts_1 = render_canonical_artifacts(&report_1).expect("render report 1");
    let artifacts_2 = render_canonical_artifacts(&report_2).expect("render report 2");

    assert_eq!(
        artifacts_1.report_json, artifacts_2.report_json,
        "report_json must be byte-identical regardless of insertion order"
    );
    assert_eq!(
        artifacts_1.graph_json, artifacts_2.graph_json,
        "graph_json must be byte-identical regardless of insertion order"
    );
    assert_eq!(
        artifacts_1.report_sha256, artifacts_2.report_sha256,
        "report_sha256 must match exactly"
    );
    assert_eq!(
        artifacts_1.graph_sha256, artifacts_2.graph_sha256,
        "graph_sha256 must match exactly"
    );

    // Trailing newline requirement
    assert!(
        artifacts_1.report_json.ends_with(b"\n"),
        "report_json must end with newline"
    );
    assert!(
        artifacts_1.graph_json.ends_with(b"\n"),
        "graph_json must end with newline"
    );

    // Verify SHA-256 integrity against raw bytes
    let computed_report_hash = format!("{:x}", Sha256::digest(&artifacts_1.report_json));
    assert_eq!(artifacts_1.report_sha256, computed_report_hash);

    let computed_graph_hash = format!("{:x}", Sha256::digest(&artifacts_1.graph_json));
    assert_eq!(artifacts_1.graph_sha256, computed_graph_hash);
}

#[test]
fn test_path_sanitization_removes_host_and_user_paths() {
    let mut report = sample_report_pair().0;

    // Inject absolute host paths with user-identifying info and backslashes
    report.graph.nodes = vec![
        AssetNode {
            id: r"database:C:\Users\SecretUser\Projects\Game\Data\System.json".into(),
            kind: AssetKind::DatabaseEntity,
            logical_path: Some(r"C:\Users\SecretUser\Projects\Game\Data\System.json".into()),
        },
        AssetNode {
            id: "audio:/home/secretdev/work/Game/Audio/BGM/battle.ogg".into(),
            kind: AssetKind::Audio,
            logical_path: Some("/home/secretdev/work/Game/Audio/BGM/battle.ogg".into()),
        },
        AssetNode {
            id: r"image:Graphics\Titles\Title.png".into(),
            kind: AssetKind::Image,
            logical_path: Some(r"Graphics\Titles\Title.png".into()),
        },
    ];

    report.findings = vec![AuditFinding {
        id: "f-01".into(),
        code: "CMP-AUD-002".into(),
        severity: FindingSeverity::Error,
        disposition: FindingDisposition::Blocking,
        requiredness: ReferenceRequiredness::Required,
        message: r"Missing file at C:\Users\SecretUser\Projects\Game\Data\System.json.".into(),
        source_node_ids: vec![
            r"database:C:\Users\SecretUser\Projects\Game\Data\System.json".into(),
        ],
        orphan: false,
    }];

    let artifacts = render_canonical_artifacts(&report).expect("render sanitized artifacts");
    let json_text = String::from_utf8(artifacts.report_json.clone()).expect("valid utf-8 json");

    // Must not leak host drives or user identifying strings
    assert!(
        !json_text.contains("C:\\"),
        "Report JSON must not contain Windows drive backslashes"
    );
    assert!(
        !json_text.contains("C:/"),
        "Report JSON must not contain Windows drive forward-slashes"
    );
    assert!(
        !json_text.contains("SecretUser"),
        "Report JSON must not leak username 'SecretUser'"
    );
    assert!(
        !json_text.contains("secretdev"),
        "Report JSON must not leak username 'secretdev'"
    );
    assert!(
        !json_text.contains("/home/"),
        "Report JSON must not contain Unix /home/ path"
    );

    // Must contain normalized logical paths
    assert!(
        json_text.contains("Data/System.json"),
        "Report JSON must contain sanitized relative logical path Data/System.json"
    );
    assert!(
        json_text.contains("Audio/BGM/battle.ogg"),
        "Report JSON must contain sanitized relative logical path Audio/BGM/battle.ogg"
    );
    assert!(
        json_text.contains("Graphics/Titles/Title.png"),
        "Report JSON must contain forward-slash path Graphics/Titles/Title.png"
    );
    // Trailing punctuation preserved outside path
    assert!(
        json_text.contains("Data/System.json."),
        "Report JSON must preserve trailing punctuation outside sanitized path"
    );
}

#[test]
fn test_audit_cache_key_properties_and_determinism() {
    let key1 = AuditCacheKey::new(
        "abc123hash",
        EngineId::RpgMakerMv,
        AuditMode::Standard,
        CURRENT_AUDIT_RULESET_VERSION,
        "0.1.0",
    );
    let key2 = AuditCacheKey::new(
        "abc123hash",
        EngineId::RpgMakerMv,
        AuditMode::Standard,
        CURRENT_AUDIT_RULESET_VERSION,
        "0.1.0",
    );

    assert_eq!(
        key1.to_sha256(),
        key2.to_sha256(),
        "Identical keys must produce identical hashes"
    );
    assert_eq!(key1.cache_hash(), key1.to_sha256());

    // Changing source hash changes key hash
    let key_diff_src = AuditCacheKey::new(
        "diff_src_hash",
        EngineId::RpgMakerMv,
        AuditMode::Standard,
        CURRENT_AUDIT_RULESET_VERSION,
        "0.1.0",
    );
    assert_ne!(key1.to_sha256(), key_diff_src.to_sha256());

    // Changing engine changes key hash
    let key_diff_engine = AuditCacheKey::new(
        "abc123hash",
        EngineId::RpgMakerMz,
        AuditMode::Standard,
        CURRENT_AUDIT_RULESET_VERSION,
        "0.1.0",
    );
    assert_ne!(key1.to_sha256(), key_diff_engine.to_sha256());

    // Changing mode changes key hash
    let key_diff_mode = AuditCacheKey::new(
        "abc123hash",
        EngineId::RpgMakerMv,
        AuditMode::Deep,
        CURRENT_AUDIT_RULESET_VERSION,
        "0.1.0",
    );
    assert_ne!(key1.to_sha256(), key_diff_mode.to_sha256());

    // Changing ruleset version changes key hash
    let key_diff_ruleset = AuditCacheKey::new(
        "abc123hash",
        EngineId::RpgMakerMv,
        AuditMode::Standard,
        CURRENT_AUDIT_RULESET_VERSION + 1,
        "0.1.0",
    );
    assert_ne!(key1.to_sha256(), key_diff_ruleset.to_sha256());

    // Changing toolchain version changes key hash
    let key_diff_toolchain = AuditCacheKey::new(
        "abc123hash",
        EngineId::RpgMakerMv,
        AuditMode::Standard,
        CURRENT_AUDIT_RULESET_VERSION,
        "0.2.0",
    );
    assert_ne!(key1.to_sha256(), key_diff_toolchain.to_sha256());

    // Changing limits changes key hash
    let key_diff_limits = key1.clone().with_limits(AuditLimits {
        max_files: 100,
        max_total_bytes: 1024,
        max_single_file_bytes: 512,
    });
    assert_ne!(key1.to_sha256(), key_diff_limits.to_sha256());

    // Toggling allow_network changes key hash
    let key_network_allowed = key1.clone().with_allow_network(true);
    assert_ne!(key1.to_sha256(), key_network_allowed.to_sha256());

    // Setting detected_version changes key hash
    let key_with_version = key1.clone().with_detected_version(Some("1.2.3".into()));
    assert_ne!(key1.to_sha256(), key_with_version.to_sha256());

    let key_with_diff_version = key1.clone().with_detected_version(Some("1.2.4".into()));
    assert_ne!(
        key_with_version.to_sha256(),
        key_with_diff_version.to_sha256()
    );
}

#[test]
fn test_audit_cache_hit_produces_byte_identical_artifacts() {
    let cache_dir = temp_dir("cache-hit");
    let cache = AuditCache::new(&cache_dir);

    let (report, _) = sample_report_pair();
    let artifacts = render_canonical_artifacts(&report).expect("render artifacts");

    let key = AuditCacheKey::new(
        "source-hash-001",
        EngineId::RpgMakerMv,
        AuditMode::Standard,
        CURRENT_AUDIT_RULESET_VERSION,
        "0.1.0",
    );

    // Initial cache miss
    assert!(
        cache.get(&key).is_none(),
        "Cache must miss when entry is not present"
    );

    // Put artifacts into cache
    cache.put(&key, &artifacts).expect("put cache entry");

    // Cache hit
    let cached = cache.get(&key).expect("cached entry must exist on hit");
    assert_eq!(
        cached.report_json, artifacts.report_json,
        "Cached report JSON must be byte-identical"
    );
    assert_eq!(
        cached.graph_json, artifacts.graph_json,
        "Cached graph JSON must be byte-identical"
    );
    assert_eq!(
        cached.report_sha256, artifacts.report_sha256,
        "Cached report SHA-256 must match"
    );
    assert_eq!(
        cached.graph_sha256, artifacts.graph_sha256,
        "Cached graph SHA-256 must match"
    );

    let _ = fs::remove_dir_all(cache_dir);
}

#[test]
fn test_corrupt_cache_entries_fail_closed() {
    let cache_dir = temp_dir("cache-corrupt");
    let cache = AuditCache::new(&cache_dir);

    let (report, _) = sample_report_pair();
    let artifacts = render_canonical_artifacts(&report).expect("render artifacts");

    let key = AuditCacheKey::new(
        "source-hash-corrupt",
        EngineId::RpgMakerMv,
        AuditMode::Standard,
        CURRENT_AUDIT_RULESET_VERSION,
        "0.1.0",
    );

    cache.put(&key, &artifacts).expect("put cache entry");

    let cache_entry_path = cache_dir.join(format!("{}.json", key.to_sha256()));
    assert!(cache_entry_path.exists());

    // Case 1: Corrupt JSON file with truncated data -> fails closed to None
    fs::write(&cache_entry_path, b"{ truncated json: ").unwrap();
    assert!(
        cache.get(&key).is_none(),
        "Truncated cache JSON must fail closed to None"
    );

    // Restore valid cache
    cache.put(&key, &artifacts).expect("restore cache");

    // Case 2: Tamper with report content so SHA-256 does not match recorded report_sha256
    let content = fs::read_to_string(&cache_entry_path).unwrap();
    let tampered = content.replace("Data/System.json", "Tampered/Data.json");
    fs::write(&cache_entry_path, tampered).unwrap();
    assert!(
        cache.get(&key).is_none(),
        "Tampered cache content with mismatched hash must fail closed to None"
    );

    let _ = fs::remove_dir_all(cache_dir);
}

#[test]
fn test_write_audit_artifacts_atomic_and_preserves_on_failure() {
    let output_dir = temp_dir("write-artifacts");
    let (report, _) = sample_report_pair();
    let artifacts = render_canonical_artifacts(&report).expect("render artifacts");

    // Initial write
    write_audit_artifacts(&output_dir, &artifacts).expect("write audit artifacts");

    let report_file = output_dir.join("asset-audit-report.json");
    let graph_file = output_dir.join("asset-dependency-graph.json");

    assert!(report_file.exists());
    assert!(graph_file.exists());

    assert_eq!(fs::read(&report_file).unwrap(), artifacts.report_json);
    assert_eq!(fs::read(&graph_file).unwrap(), artifacts.graph_json);

    // Verify no temporary files remain
    let temp_files: Vec<_> = fs::read_dir(&output_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .filter(|name| name.starts_with('.') && name.ends_with(".tmp"))
        .collect();
    assert!(
        temp_files.is_empty(),
        "No temporary files should be left: {temp_files:?}"
    );

    // Pre-populate with existing canary content
    let canary = b"ORIGINAL CANARY CONTENT";
    fs::write(&report_file, canary).unwrap();

    // Verify that writing to an invalid / read-only location or non-directory error
    // preserves the existing canary file
    let invalid_target = output_dir.join("non_directory_file");
    fs::write(&invalid_target, b"regular file").unwrap();

    // Attempting to write into a non-directory as output_dir must fail
    let write_result = write_audit_artifacts(&invalid_target, &artifacts);
    assert!(write_result.is_err(), "Must fail when output_dir is a file");

    // Original canary in output_dir remains untouched
    assert_eq!(
        fs::read(&report_file).unwrap(),
        canary,
        "Existing file must be preserved upon write failure"
    );

    let _ = fs::remove_dir_all(output_dir);
}

#[test]
fn test_audit_cache_validates_outside_source_root() {
    let source_dir = temp_dir("source-root");
    let inside_source = source_dir.join(".runtimepack/cache/audit");

    let cache = AuditCache::new(&inside_source);
    let result = cache.validate_outside_source(&source_dir);
    assert!(
        result.is_err(),
        "Cache inside source root must be rejected to protect source immutability"
    );

    let outside_dir = temp_dir("cache-outside");
    let outside_cache = AuditCache::new(&outside_dir);
    assert!(
        outside_cache.validate_outside_source(&source_dir).is_ok(),
        "Cache outside source root must be accepted"
    );

    let _ = fs::remove_dir_all(source_dir);
    let _ = fs::remove_dir_all(outside_dir);
}

#[test]
fn test_build_uses_audit_cache_on_repeat_runs() {
    let working_dir = temp_dir("build-cache-work");
    let out_dir = temp_dir("build-cache-repeat");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/fixtures/vxace_min");

    // First build run: executes audit and stores in cache
    let run1 = std::process::Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .current_dir(&working_dir)
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--no-compile")
        .arg("--accept-audit-fail")
        .output()
        .unwrap();

    let stdout1 = String::from_utf8_lossy(&run1.stdout);
    assert!(run1.status.success(), "first build must succeed: {stdout1}");
    assert!(stdout1.contains("Audit:"));

    // Second build run: should hit cache
    let run2 = std::process::Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .current_dir(&working_dir)
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&out_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--no-compile")
        .arg("--accept-audit-fail")
        .output()
        .unwrap();

    let stdout2 = String::from_utf8_lossy(&run2.stdout);
    assert!(
        run2.status.success(),
        "second build must succeed: {stdout2}"
    );
    assert!(stdout2.contains("Audit:"));
    assert!(
        stdout2.contains("[cache hit]"),
        "second build must use cache hit on repeat run. STDOUT:\n{stdout2}"
    );

    let _ = fs::remove_dir_all(working_dir);
    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn test_render_audit_html_is_completely_offline_and_self_contained() {
    let (report, _) = sample_report_pair();
    let artifacts = render_canonical_artifacts(&report).expect("render artifacts");
    let html_bytes = render_audit_html(&artifacts).expect("render html");
    let html = String::from_utf8(html_bytes).expect("valid utf-8 html");

    // Zero external URLs or CDN links
    assert!(
        !html.contains("http://"),
        "HTML report must not contain http:// links"
    );
    assert!(
        !html.contains("https://"),
        "HTML report must not contain https:// links"
    );

    // Embeds canonical JSON payload in script element
    assert!(
        html.contains(r#"<script type="application/json" id="audit-data">"#),
        "HTML report must embed json payload in #audit-data script tag"
    );

    // Verify embedded JSON is valid and matches canonical report
    let script_start = html
        .find(r#"<script type="application/json" id="audit-data">"#)
        .unwrap();
    let json_start = html[script_start..].find('>').unwrap() + script_start + 1;
    let json_end = html[json_start..].find("</script>").unwrap() + json_start;
    let embedded_json = html[json_start..json_end].trim();
    let parsed_json: serde_json::Value =
        serde_json::from_str(embedded_json).expect("embedded JSON must be valid");
    let canonical_json: serde_json::Value =
        serde_json::from_slice(&artifacts.report_json).unwrap();
    assert_eq!(
        parsed_json, canonical_json,
        "Embedded JSON must match canonical JSON"
    );
}

#[test]
fn test_render_audit_html_escapes_game_controlled_text() {
    let mut report = sample_report_pair().0;
    report.findings.push(AuditFinding {
        id: "finding-xss".into(),
        code: "CMP-AUD-002".into(),
        severity: FindingSeverity::Error,
        disposition: FindingDisposition::Blocking,
        requiredness: ReferenceRequiredness::Required,
        message: "XSS test: <script>alert('xss & dangerous \"quotes\"')</script>".into(),
        source_node_ids: vec!["map:<alert>&'test'".into()],
        orphan: false,
    });
    report.canonicalize();

    let artifacts = render_canonical_artifacts(&report).expect("render artifacts");
    let html_bytes = render_audit_html(&artifacts).expect("render html");
    let html = String::from_utf8(html_bytes).expect("valid utf-8 html");

    // Finding row / display text must be escaped
    assert!(
        html.contains("&lt;script&gt;alert(&#39;xss &amp; dangerous &quot;quotes&quot;&#39;)&lt;/script&gt;"),
        "Game-controlled finding message must be HTML-escaped in rendered body"
    );
    assert!(
        html.contains("map:&lt;alert&gt;&amp;&#39;test&#39;"),
        "Game-controlled node id must be HTML-escaped"
    );
}

#[test]
fn test_render_audit_html_renders_status_badges_and_counters() {
    let (mut report, _) = sample_report_pair();
    report.outcome = AuditOutcome::Pass;
    report.findings.clear();
    report.canonicalize();

    let artifacts_pass = render_canonical_artifacts(&report).expect("render pass");
    let html_pass =
        String::from_utf8(render_audit_html(&artifacts_pass).expect("render html")).unwrap();
    assert!(html_pass.contains("PASS"), "Must render PASS badge");

    report.outcome = AuditOutcome::Warning;
    let artifacts_warn = render_canonical_artifacts(&report).expect("render warn");
    let html_warn =
        String::from_utf8(render_audit_html(&artifacts_warn).expect("render html")).unwrap();
    assert!(html_warn.contains("WARNING"), "Must render WARNING badge");

    report.outcome = AuditOutcome::Fail;
    let artifacts_fail = render_canonical_artifacts(&report).expect("render fail");
    let html_fail =
        String::from_utf8(render_audit_html(&artifacts_fail).expect("render html")).unwrap();
    assert!(html_fail.contains("FAIL"), "Must render FAIL badge");

    report.outcome = AuditOutcome::Indeterminate;
    let artifacts_indet = render_canonical_artifacts(&report).expect("render indet");
    let html_indet =
        String::from_utf8(render_audit_html(&artifacts_indet).expect("render html")).unwrap();
    assert!(
        html_indet.contains("INDETERMINATE"),
        "Must render INDETERMINATE badge"
    );

    // Verify summary counter headers exist
    assert!(
        html_pass.contains("Total Assets"),
        "Must contain Total Assets counter"
    );
    assert!(
        html_pass.contains("Resolved"),
        "Must contain Resolved counter"
    );
    assert!(
        html_pass.contains("Missing"),
        "Must contain Missing counter"
    );
    assert!(
        html_pass.contains("Case Mismatch"),
        "Must contain Case Mismatch counter"
    );
    assert!(
        html_pass.contains("Unresolved Dynamic"),
        "Must contain Unresolved Dynamic counter"
    );
    assert!(
        html_pass.contains("Unreadable"),
        "Must contain Unreadable counter"
    );
}

#[test]
fn test_write_audit_artifacts_with_html_and_no_html() {
    let output_dir = temp_dir("write-html-artifacts");
    let (report, _) = sample_report_pair();
    let artifacts = render_canonical_artifacts(&report).expect("render artifacts");

    // Default: write_audit_artifacts emits HTML
    write_audit_artifacts(&output_dir, &artifacts).expect("write artifacts");
    assert!(output_dir.join("asset-audit-report.json").exists());
    assert!(output_dir.join("asset-dependency-graph.json").exists());
    assert!(output_dir.join("asset-audit-report.html").exists());

    // When emit_html is false: write_audit_artifacts_with_options skips HTML
    let output_dir_no_html = temp_dir("write-no-html-artifacts");
    write_audit_artifacts_with_options(&output_dir_no_html, &artifacts, false)
        .expect("write artifacts without html");
    assert!(output_dir_no_html.join("asset-audit-report.json").exists());
    assert!(output_dir_no_html
        .join("asset-dependency-graph.json")
        .exists());
    assert!(
        !output_dir_no_html.join("asset-audit-report.html").exists(),
        "HTML must NOT be emitted when emit_html=false"
    );

    let _ = fs::remove_dir_all(output_dir);
    let _ = fs::remove_dir_all(output_dir_no_html);
}

#[test]
fn test_cli_audit_emits_html_by_default_and_respects_no_html_flag() {
    let working_dir = temp_dir("cli-audit-html");
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/fixtures/vxace_min");

    // Default audit run: emits asset-audit-report.html
    let run1 = std::process::Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .current_dir(&working_dir)
        .arg("audit")
        .arg(&fixture)
        .output()
        .unwrap();

    assert!(run1.status.success(), "audit command must succeed");
    assert!(working_dir.join("asset-audit-report.json").exists());
    assert!(working_dir.join("asset-dependency-graph.json").exists());
    assert!(
        working_dir.join("asset-audit-report.html").exists(),
        "HTML report must be emitted by default"
    );

    // Remove artifacts
    let _ = fs::remove_file(working_dir.join("asset-audit-report.json"));
    let _ = fs::remove_file(working_dir.join("asset-dependency-graph.json"));
    let _ = fs::remove_file(working_dir.join("asset-audit-report.html"));

    // Run with --no-html: omits asset-audit-report.html
    let run2 = std::process::Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .current_dir(&working_dir)
        .arg("audit")
        .arg(&fixture)
        .arg("--no-html")
        .output()
        .unwrap();

    assert!(
        run2.status.success(),
        "audit command with --no-html must succeed"
    );
    assert!(working_dir.join("asset-audit-report.json").exists());
    assert!(working_dir.join("asset-dependency-graph.json").exists());
    assert!(
        !working_dir.join("asset-audit-report.html").exists(),
        "HTML report must NOT be emitted with --no-html"
    );

    let _ = fs::remove_dir_all(working_dir);
}
