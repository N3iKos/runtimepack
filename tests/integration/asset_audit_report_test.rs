use runtimepack_core::asset_audit::{
    audit_external_network_resources, audit_network_reference, classify_possible_orphans,
    render_audit_html, render_canonical_artifacts, sanitize_id, sanitize_path_str,
    write_audit_artifacts, write_audit_artifacts_with_options, AssetAuditOptions, AssetAuditReport,
    AssetKind, AssetNode, AuditCache, AuditCacheKey, AuditCompleteness, AuditCoverage,
    AuditFinding, AuditLimits, AuditMode, AuditOutcome, AuditOverride, AuditStatistics,
    DependencyGraph, EvidenceStage, EvidenceTrail, FindingDisposition, FindingSeverity,
    NetworkAuditResult, ReferenceEdge, ReferenceRequiredness, ReferenceResolution,
    RuntimeValidationState, CURRENT_AUDIT_RULESET_VERSION,
};
use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::domain::EngineId;
use runtimepack_core::ingest::{GameSourceIndex, IndexedFile};
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
    let canonical_json: serde_json::Value = serde_json::from_slice(&artifacts.report_json).unwrap();
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
        message: "XSS test: <script>alert('xss & dangerous \"quotes\"')</script> and </SCRIPT> and </sCrIpT> and </style>".into(),
        source_node_ids: vec!["map:<alert>&'test'".into()],
        orphan: false,
    });
    report.canonicalize();

    let artifacts = render_canonical_artifacts(&report).expect("render artifacts");
    let html_bytes = render_audit_html(&artifacts).expect("render html");
    let html = String::from_utf8(html_bytes).expect("valid utf-8 html");

    // Finding row / display text must be escaped
    assert!(
        html.contains(
            "&lt;script&gt;alert(&#39;xss &amp; dangerous &quot;quotes&quot;&#39;)&lt;/script&gt;"
        ),
        "Game-controlled finding message must be HTML-escaped in rendered body"
    );
    assert!(
        html.contains("&lt;/SCRIPT&gt;"),
        "Mixed-case closing script tag must be HTML-escaped in table body"
    );
    assert!(
        html.contains("map:&lt;alert&gt;&amp;&#39;test&#39;"),
        "Game-controlled node id must be HTML-escaped"
    );

    // Verify script breakout in #audit-data script tag
    let script_start = html
        .find(r#"<script type="application/json" id="audit-data">"#)
        .unwrap();
    let json_start = html[script_start..].find('>').unwrap() + script_start + 1;
    let json_end = html[json_start..].find("</script>").unwrap() + json_start;
    let embedded_json = &html[json_start..json_end];

    // Inside the script tag body, there must be NO unescaped </SCRIPT>, </script>, </sCrIpT>, etc.
    assert!(
        !embedded_json.to_lowercase().contains("</script"),
        "Embedded JSON must not contain closing script tags: {embedded_json}"
    );
    assert!(
        !embedded_json.contains("</"),
        "Embedded JSON must universally escape </ to <\\/: {embedded_json}"
    );

    // Verify it still parses as identical valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(embedded_json.trim()).expect("must be valid JSON");
    let expected: serde_json::Value = serde_json::from_slice(&artifacts.report_json).unwrap();
    assert_eq!(parsed, expected);
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

    // Verify exact counter values rendered in sample report 1
    let (report_1, _) = sample_report_pair();
    let artifacts_1 = render_canonical_artifacts(&report_1).expect("render report 1");
    let html_1 =
        String::from_utf8(render_audit_html(&artifacts_1).expect("render html 1")).unwrap();

    assert!(
        html_1.contains(r#"<div class="stat-value" id="stat-total-assets">3</div>"#),
        "Total Assets count must match exact number 3"
    );
    assert!(
        html_1.contains(r#"<div class="stat-value" id="stat-resolved">2</div>"#),
        "Resolved count must match exact number 2"
    );
    assert!(
        html_1.contains(r#"<div class="stat-value" id="stat-missing">2</div>"#),
        "Missing count must match exact number 2"
    );
    assert!(
        html_1.contains(r#"<div class="stat-value" id="stat-case-mismatch">0</div>"#),
        "Case mismatch count must match exact number 0"
    );
    assert!(
        html_1.contains(r#"<div class="stat-value" id="stat-unresolved-dynamic">0</div>"#),
        "Unresolved dynamic count must match exact number 0"
    );
    assert!(
        html_1.contains(r#"<div class="stat-value" id="stat-unreadable">0</div>"#),
        "Unreadable count must match exact number 0"
    );

    // Assert that data-stages contains SCREAMING_SNAKE_CASE like REFERENCE_FOUND, FILE_READABLE
    assert!(
        html_1.contains("FILE_READABLE"),
        "data-stages must use SCREAMING_SNAKE_CASE matching <option> values"
    );
    assert!(
        !html_1.contains("FILEREADABLE") && !html_1.contains("REFERENCEFOUND"),
        "data-stages must not contain unformatted pascal-case strings"
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

#[test]
fn test_orphan_analysis_unreferenced_assets_become_info_possibly_unused() {
    let mut files = std::collections::HashMap::new();
    for path in [
        "data/System.json",
        "graphics/titles/Title.png",
        "audio/bgm/Title.ogg",
        "graphics/pictures/unused_pic.png",
        "audio/se/unused_sound.wav",
    ] {
        files.insert(
            path.to_ascii_lowercase(),
            IndexedFile {
                logical_path: path.to_string(),
                relative_path: PathBuf::from(path),
                size_bytes: 1234,
            },
        );
    }
    let source_index = GameSourceIndex {
        root_path: PathBuf::from("mock_game"),
        files,
        total_size_bytes: 1234 * 5,
        source_hash: "mockhash".into(),
    };

    let mut graph = DependencyGraph::default();
    let node_root = AssetNode {
        id: "data:data/System.json".into(),
        kind: AssetKind::DatabaseEntity,
        logical_path: Some("data/System.json".into()),
    };
    let node_img = AssetNode {
        id: "image:graphics/titles/Title.png".into(),
        kind: AssetKind::Image,
        logical_path: Some("graphics/titles/Title.png".into()),
    };
    let node_audio = AssetNode {
        id: "audio:audio/bgm/Title.ogg".into(),
        kind: AssetKind::Audio,
        logical_path: Some("audio/bgm/Title.ogg".into()),
    };
    graph.nodes.push(node_root);
    graph.nodes.push(node_img);
    graph.nodes.push(node_audio);

    graph.edges.push(ReferenceEdge {
        source: "data:data/System.json".into(),
        target: "image:graphics/titles/Title.png".into(),
        relation: "title_graphic".into(),
        requiredness: ReferenceRequiredness::Required,
        evidence: EvidenceTrail::through(EvidenceStage::FileReadable),
        resolution: ReferenceResolution::Resolved,
    });
    graph.edges.push(ReferenceEdge {
        source: "data:data/System.json".into(),
        target: "audio:audio/bgm/Title.ogg".into(),
        relation: "title_bgm".into(),
        requiredness: ReferenceRequiredness::Required,
        evidence: EvidenceTrail::through(EvidenceStage::FileReadable),
        resolution: ReferenceResolution::Resolved,
    });

    let mut coverage = AuditCoverage::new();
    coverage.add_covered("database");
    coverage.add_covered("image");
    coverage.add_covered("audio");

    let orphan_findings = classify_possible_orphans(&source_index, &graph, &coverage);

    assert_eq!(
        orphan_findings.len(),
        2,
        "Exactly 2 unused assets should be classified as candidate orphans"
    );

    for finding in &orphan_findings {
        assert_eq!(
            finding.code,
            DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED,
            "Finding code must be CMP-AUD-013"
        );
        assert_eq!(
            finding.severity,
            FindingSeverity::Info,
            "Severity must strictly be Info"
        );
        assert_eq!(
            finding.disposition,
            FindingDisposition::Informational,
            "Disposition must strictly be Informational"
        );
        assert!(finding.orphan, "orphan flag must strictly be true");
        assert!(
            finding.message.contains("possibly unused")
                || finding.message.contains("no incoming reference edge"),
            "Message must clarify candidate orphan status: {}",
            finding.message
        );
    }

    // Verify non-escalating outcome
    let mut report = AssetAuditReport {
        schema_version: 1,
        engine: EngineId::RpgMakerMv,
        mode: AuditMode::Deep,
        outcome: AuditOutcome::Pass,
        completeness: AuditCompleteness {
            required_scope_complete: true,
            covered_reference_classes: vec!["database".into(), "image".into(), "audio".into()],
            uncovered_reference_classes: Vec::new(),
        },
        statistics: AuditStatistics::default(),
        graph,
        findings: orphan_findings,
        overrides: Vec::new(),
        runtime_validation: RuntimeValidationState::NotRun,
    };

    report.recompute_outcome();
    assert_eq!(
        report.outcome,
        AuditOutcome::Pass,
        "Orphan findings must never escalate outcome to Warning or Fail"
    );
}

#[test]
fn test_orphan_analysis_dynamic_coverage_keeps_orphan_informational_with_clarification() {
    let mut files = std::collections::HashMap::new();
    files.insert(
        "graphics/pictures/extra.png".to_string(),
        IndexedFile {
            logical_path: "graphics/pictures/extra.png".to_string(),
            relative_path: PathBuf::from("graphics/pictures/extra.png"),
            size_bytes: 500,
        },
    );
    let source_index = GameSourceIndex {
        root_path: PathBuf::from("mock_game"),
        files,
        total_size_bytes: 500,
        source_hash: "mockhash".into(),
    };

    let graph = DependencyGraph::default();

    let mut coverage = AuditCoverage::new();
    coverage.add_covered("image");
    coverage.add_dynamic("script_eval", ReferenceRequiredness::Required);

    assert!(
        coverage.has_unsupported_or_dynamic(),
        "Coverage must indicate dynamic references exist"
    );

    let orphan_findings = classify_possible_orphans(&source_index, &graph, &coverage);

    assert_eq!(orphan_findings.len(), 1);
    let finding = &orphan_findings[0];
    assert_eq!(finding.code, DiagnosticCode::CMP_AUD_POSSIBLY_UNUSED);
    assert_eq!(finding.severity, FindingSeverity::Info);
    assert_eq!(finding.disposition, FindingDisposition::Informational);
    assert!(finding.orphan);
    assert!(
        finding.message.to_lowercase().contains("dynamic"),
        "Message must clarify that dynamic references exist in the project: {}",
        finding.message
    );
}

#[test]
fn test_bounded_network_audit_with_mock_http_server() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local mock server");
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).expect("set nonblocking");

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    let server_handle = std::thread::spawn(move || {
        while r.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    let mut buf = [0u8; 1024];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]);

                    if req.contains("/assets/valid.png") {
                        let resp = "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 4\r\nConnection: close\r\n\r\nTEST";
                        let _ = stream.write_all(resp.as_bytes());
                    } else if req.contains("/assets/redirect.png") {
                        let resp = format!(
                            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{}/assets/valid.png\r\nConnection: close\r\n\r\n",
                            port
                        );
                        let _ = stream.write_all(resp.as_bytes());
                    } else if req.contains("/assets/missing.png") {
                        let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                        let _ = stream.write_all(resp.as_bytes());
                    } else {
                        let resp = "HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n";
                        let _ = stream.write_all(resp.as_bytes());
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    let valid_url = format!("http://127.0.0.1:{port}/assets/valid.png");
    let redirect_url = format!("http://127.0.0.1:{port}/assets/redirect.png");
    let missing_url = format!("http://127.0.0.1:{port}/assets/missing.png");

    // Case 1: allow_network = false -> UnverifiedOffline
    let offline_options = AssetAuditOptions {
        allow_network: false,
        ..Default::default()
    };

    let offline_res = audit_network_reference(&valid_url, &offline_options);
    assert!(
        matches!(offline_res, NetworkAuditResult::UnverifiedOffline { .. }),
        "Offline audit must produce UnverifiedOffline"
    );
    let offline_finding = offline_res.to_finding(Some("source_node_1")).unwrap();
    assert_eq!(
        offline_finding.code,
        DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED
    );
    assert_eq!(
        offline_finding.disposition,
        FindingDisposition::Indeterminate
    );

    // Case 2: allow_network = true -> Verified for 200 OK
    let online_options = AssetAuditOptions {
        allow_network: true,
        ..Default::default()
    };

    let online_res = audit_network_reference(&valid_url, &online_options);
    assert!(
        matches!(
            online_res,
            NetworkAuditResult::Verified {
                status_code: 200,
                ..
            }
        ),
        "Online audit for valid URL must return Verified: {:?}",
        online_res
    );
    assert!(
        online_res.to_finding(None).is_none(),
        "Verified network resource must produce no error finding"
    );

    // Case 3: allow_network = true -> Follows redirect (302 -> 200)
    let redirect_res = audit_network_reference(&redirect_url, &online_options);
    assert!(
        matches!(
            redirect_res,
            NetworkAuditResult::Verified {
                status_code: 200,
                ..
            }
        ),
        "Online audit following redirect must return Verified: {:?}",
        redirect_res
    );

    // Case 4: allow_network = true -> NotFound on 404
    let missing_res = audit_network_reference(&missing_url, &online_options);
    assert!(
        matches!(
            missing_res,
            NetworkAuditResult::NotFound {
                status_code: Some(404),
                ..
            }
        ),
        "Online audit for missing URL must return NotFound: {:?}",
        missing_res
    );
    let missing_finding = missing_res.to_finding(Some("source_node_2")).unwrap();
    assert_eq!(
        missing_finding.code,
        DiagnosticCode::CMP_AUD_REQUIRED_REFERENCE_MISSING
    );
    assert_eq!(missing_finding.disposition, FindingDisposition::Blocking);

    // Case 5: Invalid scheme (e.g. ftp://)
    let invalid_res = audit_network_reference("ftp://example.com/asset.png", &online_options);
    assert!(
        matches!(invalid_res, NetworkAuditResult::InvalidUrl { .. }),
        "Invalid scheme must return InvalidUrl"
    );

    running.store(false, Ordering::SeqCst);
    let _ = server_handle.join();
}

#[test]
fn test_regression_network_audit_https_remains_indeterminate_without_fail() {
    let online_options = AssetAuditOptions {
        allow_network: true,
        ..Default::default()
    };
    let https_url = "https://example.com/asset.png";
    let res = audit_network_reference(https_url, &online_options);

    // Finding 2: When allow_network is true but TLS transport is unavailable,
    // do NOT return NotFound (which causes blocking FAIL). Return UnverifiedOffline / Indeterminate.
    assert!(
        matches!(res, NetworkAuditResult::UnverifiedOffline { .. }),
        "HTTPS without TLS must yield UnverifiedOffline, not NotFound: {:?}",
        res
    );

    let finding = res.to_finding(Some("source_node")).expect("finding");
    assert_eq!(
        finding.code,
        DiagnosticCode::CMP_AUD_EXTERNAL_RESOURCE_UNVERIFIED
    );
    assert_eq!(
        finding.disposition,
        FindingDisposition::Indeterminate,
        "Disposition must be Indeterminate, not Blocking"
    );

    let mut report = AssetAuditReport {
        schema_version: 1,
        engine: EngineId::RpgMakerMv,
        mode: AuditMode::Standard,
        outcome: AuditOutcome::Pass,
        completeness: AuditCompleteness {
            required_scope_complete: true,
            covered_reference_classes: vec!["network".into()],
            uncovered_reference_classes: Vec::new(),
        },
        statistics: AuditStatistics::default(),
        graph: DependencyGraph::default(),
        findings: vec![finding],
        overrides: Vec::new(),
        runtime_validation: RuntimeValidationState::NotRun,
    };
    report.recompute_outcome();
    assert_eq!(
        report.outcome,
        AuditOutcome::Indeterminate,
        "HTTPS unverified finding must cause Indeterminate outcome, not Fail"
    );
}

#[test]
fn test_regression_network_audit_http_304_not_treated_as_redirect() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local mock server");
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).expect("set nonblocking");

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    let server_handle = std::thread::spawn(move || {
        while r.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    let mut buf = [0u8; 1024];
                    let _ = stream.read(&mut buf);
                    let resp = "HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n";
                    let _ = stream.write_all(resp.as_bytes());
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    let url_304 = format!("http://127.0.0.1:{port}/assets/cached.png");
    let online_options = AssetAuditOptions {
        allow_network: true,
        ..Default::default()
    };

    let res = audit_network_reference(&url_304, &online_options);
    // Finding 3: HTTP 304 is NOT treated as a redirect (no Location header needed)
    assert!(
        matches!(
            res,
            NetworkAuditResult::NotFound {
                status_code: Some(304),
                ..
            }
        ),
        "HTTP 304 must not be treated as a redirect: {:?}",
        res
    );

    running.store(false, Ordering::SeqCst);
    let _ = server_handle.join();
}

#[test]
fn test_regression_network_audit_url_with_query_and_no_path() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local mock server");
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).expect("set nonblocking");

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    let server_handle = std::thread::spawn(move || {
        while r.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    let mut buf = [0u8; 1024];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]);
                    if req.contains("GET /?token=xyz") {
                        let resp = "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nConnection: close\r\n\r\nOK";
                        let _ = stream.write_all(resp.as_bytes());
                    } else {
                        let resp = "HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n";
                        let _ = stream.write_all(resp.as_bytes());
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    let url_query_only = format!("http://127.0.0.1:{port}?token=xyz");
    let online_options = AssetAuditOptions {
        allow_network: true,
        ..Default::default()
    };

    // Finding 4: Handle query strings on URLs without paths (e.g. http://example.com?q=1)
    let res = audit_network_reference(&url_query_only, &online_options);
    assert!(
        matches!(
            res,
            NetworkAuditResult::Verified {
                status_code: 200,
                ..
            }
        ),
        "URL with query and no path must parse and verify correctly: {:?}",
        res
    );

    running.store(false, Ordering::SeqCst);
    let _ = server_handle.join();
}

#[test]
fn test_regression_audit_external_network_resources_does_not_mangle_raw_http_urls() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local mock server");
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).expect("set nonblocking");

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    let server_handle = std::thread::spawn(move || {
        while r.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    let mut buf = [0u8; 1024];
                    let _ = stream.read(&mut buf);
                    let resp =
                        "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nConnection: close\r\n\r\nOK";
                    let _ = stream.write_all(resp.as_bytes());
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    let raw_url = format!("http://127.0.0.1:{port}/assets/raw_url.png");

    let mut graph = DependencyGraph::default();
    // Raw URL in node and edge target WITHOUT "external:" prefix
    graph.nodes.push(AssetNode {
        id: raw_url.clone(),
        kind: AssetKind::ExternalResource,
        logical_path: None,
    });
    graph.edges.push(ReferenceEdge {
        source: "script:main".into(),
        target: raw_url.clone(),
        relation: "downloads".into(),
        requiredness: ReferenceRequiredness::Required,
        evidence: EvidenceTrail::through(EvidenceStage::ReferenceFound),
        resolution: ReferenceResolution::External,
    });

    let mut report = AssetAuditReport {
        schema_version: 1,
        engine: EngineId::RpgMakerMv,
        mode: AuditMode::Standard,
        outcome: AuditOutcome::Pass,
        completeness: AuditCompleteness {
            required_scope_complete: true,
            covered_reference_classes: vec!["network".into()],
            uncovered_reference_classes: Vec::new(),
        },
        statistics: AuditStatistics::default(),
        graph,
        findings: Vec::new(),
        overrides: Vec::new(),
        runtime_validation: RuntimeValidationState::NotRun,
    };

    let online_options = AssetAuditOptions {
        allow_network: true,
        ..Default::default()
    };

    // Finding 1: Ensure strip_prefix("external:") is used so raw http:// is not mangled to //...
    audit_external_network_resources(&mut report, &online_options);

    assert!(
        report.findings.is_empty(),
        "Raw URL must be requested as http://... and verified, not mangled: {:?}",
        report.findings
    );
    assert_eq!(
        report.graph.edges[0].evidence,
        EvidenceTrail::through(EvidenceStage::IntegrityVerified),
        "Edge evidence must be upgraded to IntegrityVerified on 200 OK"
    );

    running.store(false, Ordering::SeqCst);
    let _ = server_handle.join();
}

#[test]
fn test_regression_sanitize_preserves_external_urls_without_truncation() {
    let url_http = "http://example.com/sound/music.ogg";
    let url_https = "https://cdn.example.com/data/bgm.ogg";
    let url_external = "external:https://cdn.example.com/data/image.png";

    assert_eq!(
        sanitize_path_str(url_http),
        url_http,
        "HTTP URL must not be truncated at 'sound'"
    );
    assert_eq!(
        sanitize_path_str(url_https),
        url_https,
        "HTTPS URL must not be truncated at 'data'"
    );
    assert_eq!(
        sanitize_path_str(url_external),
        url_external,
        "External URL with prefix must be preserved intact"
    );

    assert_eq!(
        sanitize_id(url_external),
        url_external,
        "sanitize_id must preserve external URL with prefix"
    );
    assert_eq!(
        sanitize_id(url_http),
        url_http,
        "sanitize_id must preserve raw HTTP URL without splitting on ':'"
    );
}

#[test]
fn test_regression_build_preserves_clean_audit_cache_without_ephemeral_overrides() {
    let working_dir = temp_dir("build-clean-audit-cache");
    let dist_dir = temp_dir("build-clean-dist");
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/fixtures/vxace_min");

    // Run build with --accept-audit-fail and --no-compile
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_runtimepack"))
        .current_dir(&working_dir)
        .arg("build")
        .arg(&fixture)
        .arg("--output")
        .arg(&dist_dir)
        .arg("--flavor")
        .arg("beta")
        .arg("--no-compile")
        .arg("--accept-audit-fail")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&run.stdout);
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        run.status.success(),
        "Build must succeed with override:\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
    );

    // 1. Output directory report MUST contain the build override record
    let build_report_path = dist_dir.join("asset-audit-report.json");
    assert!(build_report_path.exists());
    let build_report_bytes = fs::read(&build_report_path).unwrap();
    let build_report: AssetAuditReport = serde_json::from_slice(&build_report_bytes).unwrap();
    assert_eq!(
        build_report.overrides.len(),
        1,
        "Build output report must record the accepted override"
    );
    assert_eq!(build_report.overrides[0].flag, "--accept-audit-fail");

    // 2. The audit cache MUST remain pristine (zero overrides)
    let cache_dir = working_dir.join(".runtimepack/cache/audit");
    assert!(cache_dir.exists(), "Cache directory must exist");
    let mut checked_cached_reports = 0;
    for entry in fs::read_dir(&cache_dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            let cache_bytes = fs::read(&path).unwrap();
            let entry_val: serde_json::Value = serde_json::from_slice(&cache_bytes).unwrap();
            let report_str = entry_val["report_json_utf8"].as_str().unwrap();
            let cached_report: AssetAuditReport = serde_json::from_str(report_str).unwrap();
            assert!(
                cached_report.overrides.is_empty(),
                "Cached report in audit cache must remain clean with 0 overrides, found: {:?}",
                cached_report.overrides
            );
            checked_cached_reports += 1;
        }
    }
    assert!(
        checked_cached_reports > 0,
        "At least one cached report must be validated"
    );

    let _ = fs::remove_dir_all(working_dir);
    let _ = fs::remove_dir_all(dist_dir);
}

#[test]
fn test_runtime_validation_report_and_combined_summary_schemas() {
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();

    // 1. Runtime validation report schema
    let runtime_schema_path = workspace_root.join("schemas/runtime-validation-report.schema.json");
    assert!(runtime_schema_path.exists());
    let runtime_schema_str = fs::read_to_string(&runtime_schema_path).unwrap();
    let runtime_schema: serde_json::Value = serde_json::from_str(&runtime_schema_str).unwrap();

    assert_eq!(runtime_schema["title"], "RuntimeValidationReport");
    let event_props = &runtime_schema["properties"]["events"]["items"]["properties"];
    assert!(
        event_props["is_complete"].is_object(),
        "is_complete must be defined in events item properties"
    );
    assert_eq!(event_props["is_complete"]["type"], "boolean");

    // 2. Combined validation summary schema
    let summary_schema_path = workspace_root.join("schemas/combined-validation-summary.schema.json");
    assert!(summary_schema_path.exists());
    let summary_schema_str = fs::read_to_string(&summary_schema_path).unwrap();
    let summary_schema: serde_json::Value = serde_json::from_str(&summary_schema_str).unwrap();

    assert_eq!(summary_schema["title"], "CombinedValidationSummary");
    assert!(summary_schema["properties"]["combined_outcome"].is_object());
    assert!(summary_schema["properties"]["discrepancies"].is_object());
}
