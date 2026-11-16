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
use runtimepack_core::build_plan::{
    evaluate_audit_policy_with_mode, evaluate_build_policy, evaluate_build_policy_typed,
    AuditBuildPolicy, AuditPolicyDecision,
};
use runtimepack_core::build_policy::{BuildPolicy, ReportStatus};
use runtimepack_core::compatibility::{CompatibilityIssue, CompatibilityReport};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::diagnostics::{Diagnostic, DiagnosticCode, DiagnosticSeverity};
use runtimepack_core::domain::{CompatibilityStatus, DetectionConfidence, EngineId};
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
    assert_eq!(
        CURRENT_AUDIT_RULESET_VERSION, 26,
        "Audit ruleset version must be 26 after web portability policy, dormancy, and live boundary (25 was the web audit provenance and cache correctness)"
    );

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

    // Setting content_root changes key hash
    let key_with_root = key1.clone().with_content_root(Some("bin/www".into())).expect("valid root");
    assert_ne!(key1.to_sha256(), key_with_root.to_sha256());

    let key_with_diff_root = key1.clone().with_content_root(Some("www".into())).expect("valid root");
    assert_ne!(
        key_with_root.to_sha256(),
        key_with_diff_root.to_sha256()
    );

    // Canonicalization of empty/dot content_root to None
    let key_empty_root = key1.clone().with_content_root(Some("".into())).expect("valid empty root");
    assert_eq!(key_empty_root.content_root, None);
    assert_eq!(key1.to_sha256(), key_empty_root.to_sha256());

    let key_dot_root = key1.clone().with_content_root(Some(".".into())).expect("valid dot root");
    assert_eq!(key_dot_root.content_root, None);
    assert_eq!(key1.to_sha256(), key_dot_root.to_sha256());

    // Fail-closed rejection of malformed content roots (must return Err, never silently fall back to None)
    let key_malformed_traversal = key1.clone().with_content_root(Some("../escape".into()));
    assert!(key_malformed_traversal.is_err(), "traversal path in content root must fail closed with Err");

    let key_malformed_abs = key1.clone().with_content_root(Some("/etc".into()));
    assert!(key_malformed_abs.is_err(), "absolute path in content root must fail closed with Err");
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
                source_root: None,
            },
        );
    }
    let source_index = GameSourceIndex {
        root_path: PathBuf::from("mock_game"),
        files,
        total_size_bytes: 1234 * 5,
        source_hash: "mockhash".into(),
        rtp_root: None,
        content_root: None,
        scoped_content_hash: None,
        original_source_hash: None,
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
            source_root: None,
        },
    );
    let source_index = GameSourceIndex {
        root_path: PathBuf::from("mock_game"),
        files,
        total_size_bytes: 500,
        source_hash: "mockhash".into(),
        rtp_root: None,
        content_root: None,
        scoped_content_hash: None,
        original_source_hash: None,
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
    assert!(event_props["category"].is_object(), "category must be defined in events item properties");
    assert!(event_props["audit_id"].is_object(), "audit_id must be defined in events item properties");

    let report_props = &runtime_schema["properties"];
    assert!(report_props["missing_count"].is_object());
    assert!(report_props["decode_failed_count"].is_object());
    assert!(report_props["indeterminate_count"].is_object());
    assert!(report_props["excluded_count"].is_object());
    assert!(report_props["attempted_count"].is_object());
    assert!(report_props["stage_attempt_counts"].is_object());

    // 2. Combined validation summary schema
    let summary_schema_path = workspace_root.join("schemas/combined-validation-summary.schema.json");
    assert!(summary_schema_path.exists());
    let summary_schema_str = fs::read_to_string(&summary_schema_path).unwrap();
    let summary_schema: serde_json::Value = serde_json::from_str(&summary_schema_str).unwrap();

    assert_eq!(summary_schema["title"], "CombinedValidationSummary");
    assert!(summary_schema["properties"]["combined_outcome"].is_object());
    assert!(summary_schema["properties"]["discrepancies"].is_object());
    let runtime_summary_props = &summary_schema["properties"]["runtime_summary"]["properties"];
    assert!(runtime_summary_props["missing_count"].is_object());
    assert!(runtime_summary_props["decode_failed_count"].is_object());
    assert!(runtime_summary_props["indeterminate_count"].is_object());
    assert!(runtime_summary_props["excluded_count"].is_object());
    assert!(runtime_summary_props["attempted_count"].is_object());
    assert!(runtime_summary_props["stage_attempt_counts"].is_object());
}

// =============================================================================
// Integration-gates — audit ruleset 23 cache invalidation
//
// Requirement LM-T1-04, defined in
// `Docs/superpowers/specs/2026-10-22-linux-batch-compatibility-remediation-design.md`
// §4 "Audit ruleset version 22 (requirement LM-T1-04)": bump
// CURRENT_AUDIT_RULESET_VERSION exactly once per coordinated semantic change —
// 21 -> 22 for the T1 `loadjs` runtime-base semantics, 22 -> 23 for the P1
// advisory classification semantics (CMP-109 / CMP-110 projection shared by the
// web analyzer and auditor), 23 -> 24 for the P2 web enhancements, and 24 -> 25
// for the Task 3 web audit provenance and cache correctness, and 25 -> 26
// for Task 3/4 web portability policy, dormancy, and live boundary. Each bump
// must prove that the previous ruleset's
// cache entry is a miss rather than a fake hit, and must never delete caches.
// =============================================================================

#[test]
fn test_stale_ruleset_24_cache_entry_is_miss_for_ruleset_25() {
    let cache_dir = temp_dir("cache-stale-ruleset-24");
    let cache = AuditCache::new(&cache_dir);

    let (report, _) = sample_report_pair();
    let artifacts = render_canonical_artifacts(&report).expect("render artifacts");

    // Authored baseline entry: exactly what a ruleset-24 run produced before the
    // Task 3 web audit provenance and cache correctness landed. No real game cache is generated here.
    let stale_key = AuditCacheKey::new(
        "source-hash-stale-ruleset-24",
        EngineId::RpgMakerMz,
        AuditMode::Standard,
        24,
        "0.1.0",
    );
    cache
        .put(&stale_key, &artifacts)
        .expect("write authored ruleset-24 entry");
    assert!(
        cache.get(&stale_key).is_some(),
        "authored ruleset-24 entry must be readable under its own key"
    );

    let current_key = AuditCacheKey::new(
        "source-hash-stale-ruleset-24",
        EngineId::RpgMakerMz,
        AuditMode::Standard,
        CURRENT_AUDIT_RULESET_VERSION,
        "0.1.0",
    );
    assert_ne!(
        stale_key.to_sha256(),
        current_key.to_sha256(),
        "ruleset version must participate in cache identity"
    );
    assert!(
        cache.get(&current_key).is_none(),
        "a ruleset-24 entry must not satisfy a ruleset-25 request (no fake hit)"
    );

    // The current-ruleset entry is a genuine miss first, then a real hit.
    cache
        .put(&current_key, &artifacts)
        .expect("write ruleset-25 entry");
    let hit = cache
        .get(&current_key)
        .expect("ruleset-25 entry must hit after put");
    assert_eq!(hit.report_sha256, artifacts.report_sha256);
    assert_eq!(hit.graph_sha256, artifacts.graph_sha256);

    // The bump path must not delete the old entry.
    assert!(
        cache.get(&stale_key).is_some(),
        "existing ruleset-24 cache entries must not be deleted by the bump"
    );

    let _ = fs::remove_dir_all(cache_dir);
}

#[test]
fn test_audit_ruleset_version_is_26_after_web_portability_policy_and_dormancy() {
    assert_eq!(
        CURRENT_AUDIT_RULESET_VERSION, 26,
        "Ruleset must be 26 once the web portability policy and dormancy changes land; \
         a later audit semantic change requires another coordinated bump"
    );
}

#[test]
fn test_stale_ruleset_25_cache_entry_is_miss_for_ruleset_26() {
    let cache_dir = temp_dir("cache-stale-ruleset-25");
    let cache = AuditCache::new(&cache_dir);

    let (report, _) = sample_report_pair();
    let artifacts = render_canonical_artifacts(&report).expect("render artifacts");

    // Authored baseline entry: exactly what a ruleset-25 run produced before the
    // Task 3/4 web portability policy and dormancy changes landed, with explicit limits/root/mode.
    let custom_limits = AuditLimits {
        max_files: 1234,
        max_total_bytes: 56_789_012,
        max_single_file_bytes: 345_678,
    };
    let stale_key = AuditCacheKey::new(
        "source-hash-stale-ruleset-25",
        EngineId::RpgMakerMz,
        AuditMode::Deep,
        25,
        "0.1.0",
    )
    .with_limits(custom_limits.clone())
    .with_allow_network(false)
    .with_content_root(Some("bin/www".to_string()))
    .unwrap();

    cache
        .put(&stale_key, &artifacts)
        .expect("write authored ruleset-25 entry");
    assert!(
        cache.get(&stale_key).is_some(),
        "authored ruleset-25 entry must be readable under its own key"
    );

    let current_key = AuditCacheKey::new(
        "source-hash-stale-ruleset-25",
        EngineId::RpgMakerMz,
        AuditMode::Deep,
        CURRENT_AUDIT_RULESET_VERSION,
        "0.1.0",
    )
    .with_limits(custom_limits.clone())
    .with_allow_network(false)
    .with_content_root(Some("bin/www".to_string()))
    .unwrap();

    assert_ne!(
        stale_key.to_sha256(),
        current_key.to_sha256(),
        "ruleset version must participate in cache identity"
    );
    assert!(
        cache.get(&current_key).is_none(),
        "a ruleset-25 entry with identical limits/root/mode must not satisfy a ruleset-26 request (no fake hit)"
    );

    // The current-ruleset entry is a genuine miss first, then a real hit.
    cache
        .put(&current_key, &artifacts)
        .expect("write ruleset-26 entry");
    let hit = cache
        .get(&current_key)
        .expect("ruleset-26 entry must hit after put");
    assert_eq!(hit.report_sha256, artifacts.report_sha256);
    assert_eq!(hit.graph_sha256, artifacts.graph_sha256);

    // Preserving context limits/root/mode: changing any of them still misses ruleset-26 hit
    let diff_mode_key = AuditCacheKey::new(
        "source-hash-stale-ruleset-25",
        EngineId::RpgMakerMz,
        AuditMode::Standard,
        CURRENT_AUDIT_RULESET_VERSION,
        "0.1.0",
    )
    .with_limits(custom_limits)
    .with_allow_network(false)
    .with_content_root(Some("bin/www".to_string()))
    .unwrap();
    assert!(
        cache.get(&diff_mode_key).is_none(),
        "different audit mode must miss"
    );

    // The bump path must not delete the old entry.
    assert!(
        cache.get(&stale_key).is_some(),
        "existing ruleset-25 cache entries must not be deleted by the bump"
    );

    let _ = fs::remove_dir_all(cache_dir);
}

#[test]
fn test_stale_ruleset_22_cache_entry_is_miss_for_ruleset_23() {
    let cache_dir = temp_dir("cache-stale-ruleset-22");
    let cache = AuditCache::new(&cache_dir);

    let (report, _) = sample_report_pair();
    let artifacts = render_canonical_artifacts(&report).expect("render artifacts");

    // Authored baseline entry: exactly what a ruleset-22 run produced before the
    // P1 advisory semantics landed. No real game cache is generated here.
    let stale_key = AuditCacheKey::new(
        "source-hash-stale-ruleset-22",
        EngineId::RpgMakerMz,
        AuditMode::Standard,
        22,
        "0.1.0",
    );
    cache
        .put(&stale_key, &artifacts)
        .expect("write authored ruleset-22 entry");
    assert!(
        cache.get(&stale_key).is_some(),
        "authored ruleset-22 entry must be readable under its own key"
    );

    let current_key = AuditCacheKey::new(
        "source-hash-stale-ruleset-22",
        EngineId::RpgMakerMz,
        AuditMode::Standard,
        CURRENT_AUDIT_RULESET_VERSION,
        "0.1.0",
    );
    assert_ne!(
        stale_key.to_sha256(),
        current_key.to_sha256(),
        "ruleset version must participate in cache identity"
    );
    assert!(
        cache.get(&current_key).is_none(),
        "a ruleset-22 entry must not satisfy a ruleset-23 request (no fake hit)"
    );

    // The current-ruleset entry is a genuine miss first, then a real hit.
    cache
        .put(&current_key, &artifacts)
        .expect("write ruleset-23 entry");
    let hit = cache
        .get(&current_key)
        .expect("ruleset-23 entry must hit after put");
    assert_eq!(hit.report_sha256, artifacts.report_sha256);
    assert_eq!(hit.graph_sha256, artifacts.graph_sha256);

    // The bump path must not delete the old entry.
    assert!(
        cache.get(&stale_key).is_some(),
        "existing ruleset-22 cache entries must not be deleted by the bump"
    );

    let _ = fs::remove_dir_all(cache_dir);
}

#[test]
fn test_stale_ruleset_21_cache_entry_is_miss_for_current_ruleset() {
    let cache_dir = temp_dir("cache-stale-ruleset-21");
    let cache = AuditCache::new(&cache_dir);

    let (report, _) = sample_report_pair();
    let artifacts = render_canonical_artifacts(&report).expect("render artifacts");

    // Authored baseline entry: exactly what a ruleset-21 run produced before the
    // T1 `loadjs` semantic change. No real game cache is generated here.
    let stale_key = AuditCacheKey::new(
        "source-hash-stale-ruleset",
        EngineId::Tyrano,
        AuditMode::Standard,
        21,
        "0.1.0",
    );
    cache
        .put(&stale_key, &artifacts)
        .expect("write authored ruleset-21 entry");
    assert!(
        cache.get(&stale_key).is_some(),
        "authored ruleset-21 entry must be readable under its own key"
    );

    let current_key = AuditCacheKey::new(
        "source-hash-stale-ruleset",
        EngineId::Tyrano,
        AuditMode::Standard,
        CURRENT_AUDIT_RULESET_VERSION,
        "0.1.0",
    );
    assert_ne!(
        stale_key.to_sha256(),
        current_key.to_sha256(),
        "ruleset version must participate in cache identity"
    );
    // Genuine miss before any current-ruleset entry is written (no file exists
    // at the current key path yet). The real cross-ruleset proof is the
    // assert_ne! above plus the key-derived entry path in AuditCache::get.
    assert!(
        cache.get(&current_key).is_none(),
        "a ruleset-21 entry must not satisfy a current-ruleset request (no fake hit)"
    );

    // The current-ruleset entry is a genuine miss first, then a real hit.
    cache
        .put(&current_key, &artifacts)
        .expect("write current ruleset entry");
    let hit = cache
        .get(&current_key)
        .expect("current-ruleset entry must hit after put");
    assert_eq!(hit.report_sha256, artifacts.report_sha256);
    assert_eq!(hit.graph_sha256, artifacts.graph_sha256);

    // The bump path must not delete the old entry.
    assert!(
        cache.get(&stale_key).is_some(),
        "existing ruleset-21 cache entries must not be deleted by the bump"
    );

    let _ = fs::remove_dir_all(cache_dir);
}

#[test]
fn test_ruleset_cache_key_binds_engine_graph_and_provenance() {
    // SPEC §4/§5: an independent engine graph plus provenance must change the
    // cache identity, so one engine/root/version result can never satisfy
    // another request.
    let base = AuditCacheKey::new(
        "source-hash-bind",
        EngineId::RpgMakerMz,
        AuditMode::Standard,
        CURRENT_AUDIT_RULESET_VERSION,
        "0.1.0",
    );

    let other_engine = AuditCacheKey::new(
        "source-hash-bind",
        EngineId::RpgMakerMv,
        AuditMode::Standard,
        CURRENT_AUDIT_RULESET_VERSION,
        "0.1.0",
    );
    assert_ne!(
        base.to_sha256(),
        other_engine.to_sha256(),
        "engine family must change the cache identity"
    );

    let other_mode = AuditCacheKey::new(
        "source-hash-bind",
        EngineId::RpgMakerMz,
        AuditMode::Deep,
        CURRENT_AUDIT_RULESET_VERSION,
        "0.1.0",
    );
    assert_ne!(
        base.to_sha256(),
        other_mode.to_sha256(),
        "audit mode must change the cache identity"
    );

    let with_root = base
        .clone()
        .with_content_root(Some("www".into()))
        .expect("scoped content root");
    assert_ne!(
        base.to_sha256(),
        with_root.to_sha256(),
        "content root provenance must change the cache identity"
    );

    let with_version = base
        .clone()
        .with_detected_version(Some("1.5.0".into()));
    assert_ne!(
        base.to_sha256(),
        with_version.to_sha256(),
        "detected version provenance must change the cache identity"
    );

    let with_limits = base.clone().with_limits(AuditLimits {
        max_files: 4096,
        ..AuditLimits::default()
    });
    assert_ne!(
        base.to_sha256(),
        with_limits.to_sha256(),
        "audit limits must change the cache identity"
    );
}

// =============================================================================
// Integration-gates — P1 shared policy
//
// Three shared-layer contracts are pinned here:
//   1. I01: the raw native codes CMP-102/103/105 block *by code*, in
//      `core/src/build_plan/audit_policy.rs`, independently of the severity an
//      advisory (or a hand-crafted report) carries.
//   2. The new P1 advisory codes CMP-109 / CMP-110 are registered in the
//      canonical diagnostics registry as Warning and are deliberately NOT in
//      that code-promotion list, so no blanket code-based waiver exists.
//   3. I03: an advisory that carries no proof linkage (or a contradictory
//      classification) is rejected, never consumed as a success.
// =============================================================================

fn minimal_policy_report() -> AssetAuditReport {
    AssetAuditReport {
        schema_version: 1,
        engine: EngineId::RpgMakerMz,
        mode: AuditMode::Standard,
        outcome: AuditOutcome::Pass,
        completeness: AuditCompleteness {
            required_scope_complete: true,
            covered_reference_classes: vec!["plugin".into()],
            uncovered_reference_classes: Vec::new(),
        },
        statistics: AuditStatistics {
            nodes: 0,
            references: 0,
            findings: 0,
        },
        graph: DependencyGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        },
        findings: Vec::new(),
        overrides: Vec::new(),
        runtime_validation: RuntimeValidationState::NotRun,
    }
}

fn advisory_finding(
    code: &str,
    severity: FindingSeverity,
    disposition: FindingDisposition,
    requiredness: ReferenceRequiredness,
) -> AuditFinding {
    AuditFinding {
        id: format!("{code}-1"),
        code: code.to_string(),
        severity,
        disposition,
        requiredness,
        message: format!("{code} projected by the P1 classifier"),
        source_node_ids: vec!["plugin:greenworks/lib/greenworks.node".into()],
        orphan: false,
    }
}

#[test]
fn test_raw_native_codes_block_regardless_of_severity_and_policy() {
    // I01: a downgraded raw native finding must still block. Promotion happens in
    // `core/src/build_plan/audit_policy.rs` by code, not by severity, so neither
    // the audit policy nor the accept flags can waive it.
    for (code, severity, disposition) in [
        (
            DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY,
            FindingSeverity::Warning,
            FindingDisposition::Warning,
        ),
        (
            DiagnosticCode::CMP_NODE_NATIVE_MODULE,
            FindingSeverity::Info,
            FindingDisposition::Informational,
        ),
        (
            DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION,
            FindingSeverity::Warning,
            FindingDisposition::Warning,
        ),
    ] {
        let mut report = minimal_policy_report();
        report.findings = vec![advisory_finding(
            code,
            severity,
            disposition,
            ReferenceRequiredness::Optional,
        )];
        report.recompute_outcome();
        assert_ne!(
            report.outcome,
            AuditOutcome::Fail,
            "{code}: the fixture must be a downgraded advisory, not an already-blocking finding"
        );

        for policy in [AuditBuildPolicy::Strict, AuditBuildPolicy::Informational] {
            for accept_fail in [false, true] {
                for accept_unknown in [false, true] {
                    let decision = evaluate_audit_policy_with_mode(
                        &report,
                        policy,
                        accept_fail,
                        accept_unknown,
                        &[],
                    );
                    assert!(
                        matches!(decision, AuditPolicyDecision::Block { .. }),
                        "{code} at {severity:?}/{disposition:?} must stay a hard blocker \
                         (policy {policy:?}, accept_fail {accept_fail}, accept_unknown {accept_unknown})"
                    );
                }
            }
        }
    }
}

#[test]
fn test_p1_advisory_codes_are_registered_and_not_code_promoted() {
    use runtimepack_core::diagnostics::{
        audit_diagnostic_spec, DiagnosticSeverity, RemediationCategory,
    };

    let not_required =
        audit_diagnostic_spec(DiagnosticCode::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET)
            .expect("CMP-109 must be registered in the canonical diagnostics registry");
    assert_eq!(not_required.code, "CMP-109");
    assert_eq!(not_required.name, "DEPENDENCY_NOT_REQUIRED_ON_TARGET");
    assert_eq!(not_required.severity, DiagnosticSeverity::Warning);
    assert_eq!(not_required.remediation, RemediationCategory::Dependency);

    let degraded = audit_diagnostic_spec(DiagnosticCode::CMP_WEB_EXTERNAL_URL_FEATURE_DEGRADED)
        .expect("CMP-110 must be registered in the canonical diagnostics registry");
    assert_eq!(degraded.code, "CMP-110");
    assert_eq!(degraded.name, "WEB_EXTERNAL_URL_FEATURE_DEGRADED");
    assert_eq!(degraded.severity, DiagnosticSeverity::Warning);
    assert_eq!(degraded.remediation, RemediationCategory::UnsupportedFeature);

    // A proved advisory is Warning/Warning and is NOT promoted by code: strict
    // policy allows it without any accept flag (I02), which is the opposite of
    // the raw native codes above.
    for code in [
        DiagnosticCode::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET,
        DiagnosticCode::CMP_WEB_EXTERNAL_URL_FEATURE_DEGRADED,
    ] {
        let mut report = minimal_policy_report();
        report.findings = vec![advisory_finding(
            code,
            FindingSeverity::Warning,
            FindingDisposition::Warning,
            ReferenceRequiredness::Required,
        )];
        report.recompute_outcome();
        assert_eq!(report.outcome, AuditOutcome::Warning);
        let decision =
            evaluate_audit_policy_with_mode(&report, AuditBuildPolicy::Strict, false, false, &[]);
        assert!(
            decision.is_allowed(),
            "{code} is an advisory and must not be code-promoted into a hard blocker"
        );
    }
}

#[test]
fn test_advisory_without_proof_linkage_is_rejected() {
    // I03: the projection is only accepted when it carries its proof linkage.
    let proved = serde_json::json!({
        "id": "CMP-109-1",
        "code": DiagnosticCode::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET,
        "severity": "WARNING",
        "disposition": "WARNING",
        "requiredness": "OPTIONAL",
        "message": "native payload proven not required on the Android target",
        "source_node_ids": ["plugin:greenworks/lib/greenworks.node"],
        "orphan": false
    });
    let finding: AuditFinding =
        serde_json::from_value(proved.clone()).expect("a proved advisory shape must parse");
    assert_eq!(finding.code, "CMP-109");

    // Absent proof linkage: no advisory may be reconstructed from a report that
    // omits the source nodes the proof points at.
    let mut missing_linkage = proved.clone();
    missing_linkage
        .as_object_mut()
        .expect("object")
        .remove("source_node_ids");
    assert!(
        serde_json::from_value::<AuditFinding>(missing_linkage).is_err(),
        "an advisory without proof linkage must be rejected, never treated as a success"
    );

    // Contradictory classification: a tampered severity/disposition pair is
    // rejected by the shared finding contract.
    let mut contradictory = proved.clone();
    contradictory["severity"] = "ERROR".into();
    assert!(
        serde_json::from_value::<AuditFinding>(contradictory).is_err(),
        "an advisory claiming ERROR severity with a WARNING disposition is contradictory"
    );

    // A report that carries the rejected shape can never be rebuilt as a passing
    // report: the deserialization failure is the fail-closed path.
    let mut report = minimal_policy_report();
    report.findings = vec![finding];
    report.recompute_outcome();
    assert_ne!(report.outcome, AuditOutcome::Pass);
}

/// Evaluates the real `finding` definition of the canonical report schema against
/// one serialized finding: required fields present, no unknown fields, and
/// exactly one `oneOf` branch satisfied.
fn finding_matches_schema(schema: &serde_json::Value, finding: &serde_json::Value) -> bool {
    let definition = &schema["definitions"]["finding"];
    let required = definition["required"].as_array().expect("required array");
    for field in required {
        let name = field.as_str().expect("required field name");
        if finding.get(name).is_none() {
            return false;
        }
    }
    let properties = definition["properties"]
        .as_object()
        .expect("finding properties");
    for key in finding.as_object().expect("finding object").keys() {
        if !properties.contains_key(key) {
            return false;
        }
    }
    let mut matched = 0usize;
    for branch in definition["oneOf"].as_array().expect("oneOf branches") {
        let constraints = branch["properties"].as_object().expect("branch properties");
        let mut ok = true;
        for (key, constraint) in constraints {
            let actual = &finding[key];
            if let Some(expected) = constraint.get("const") {
                if actual != expected {
                    ok = false;
                    break;
                }
            }
            if let Some(allowed) = constraint.get("enum") {
                let allowed = allowed.as_array().expect("enum values");
                if !allowed.iter().any(|value| value == actual) {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            matched += 1;
        }
    }
    matched == 1
}

#[test]
fn test_audit_report_schema_admits_p1_advisory_findings() {
    // The canonical schema constrains a finding by shape (severity, disposition,
    // requiredness, orphan) and leaves `code` free-form, so the P1 advisory codes
    // validate without a per-code enum. This pins the emitted shapes against the
    // real schema rather than an invented one.
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let schema_path = workspace_root.join("schemas/asset-audit-report.schema.json");
    let schema: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&schema_path).expect("read report schema"))
            .expect("parse report schema");

    assert!(
        schema["definitions"]["finding"]["properties"]["code"]
            .get("enum")
            .is_none(),
        "finding.code must stay free-form: advisory codes are governed by the \
         diagnostics registry, not by a schema enum"
    );

    // Auditor projection of a proved dormant native payload (Class A).
    let class_a = serde_json::to_value(advisory_finding(
        DiagnosticCode::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET,
        FindingSeverity::Warning,
        FindingDisposition::Warning,
        ReferenceRequiredness::Optional,
    ))
    .expect("serialize CMP-109 finding");
    assert!(
        finding_matches_schema(&schema, &class_a),
        "a generated CMP-109 Warning/Optional finding must validate against the real schema"
    );

    // Auditor projection of a degraded external-URL feature (Class C).
    let class_c = serde_json::to_value(advisory_finding(
        DiagnosticCode::CMP_WEB_EXTERNAL_URL_FEATURE_DEGRADED,
        FindingSeverity::Warning,
        FindingDisposition::Warning,
        ReferenceRequiredness::Required,
    ))
    .expect("serialize CMP-110 finding");
    assert!(
        finding_matches_schema(&schema, &class_c),
        "a generated CMP-110 Warning finding must validate against the real schema"
    );

    // A tampered advisory claiming ERROR severity with a WARNING disposition is
    // not an admitted shape.
    let mut tampered = class_a.clone();
    tampered["severity"] = "ERROR".into();
    assert!(
        !finding_matches_schema(&schema, &tampered),
        "the schema must not admit a contradictory ERROR/WARNING finding"
    );
}

// =============================================================================
// Task 4 — single build-gate entry point
//
// `evaluate_build_policy` owns compatibility translation exactly once, adds the
// caller's non-compatibility facts, delegates the existing audit policy
// semantics and deduplicates `(code, message)` in the final blocking
// diagnostics. The audit-only wrappers keep their previous behavior.
// =============================================================================

fn build_policy_compatibility(status: CompatibilityStatus) -> CompatibilityReport {
    let descriptor = EngineDescriptor {
        id: EngineId::RpgMakerMz,
        confidence: DetectionConfidence::Exact,
        score: 200,
        evidence: Vec::new(),
        conflicts: Vec::new(),
        detected_version: None,
        overridden: false,
        content_root: None,
    };
    let mut report = CompatibilityReport::new(&descriptor);
    report.status = status;
    report
}

fn compatibility_issue(
    code: &str,
    severity: DiagnosticSeverity,
    message: &str,
) -> CompatibilityIssue {
    CompatibilityIssue {
        code: code.to_string(),
        severity,
        message: message.to_string(),
        source_path: None,
        remediation: None,
    }
}

fn hard_code_finding(code: &str, message: &str) -> AuditFinding {
    AuditFinding {
        id: format!("{code}-policy"),
        code: code.to_string(),
        severity: FindingSeverity::Warning,
        disposition: FindingDisposition::Warning,
        requiredness: ReferenceRequiredness::Required,
        message: message.to_string(),
        source_node_ids: Vec::new(),
        orphan: false,
    }
}

#[test]
fn test_build_policy_scanner_only_errors_block_under_all_flags_and_policies() {
    let mut compatibility = build_policy_compatibility(CompatibilityStatus::Compatible);
    compatibility.add_issue(compatibility_issue(
        DiagnosticCode::CMP_UNSUPPORTED_WIN32,
        DiagnosticSeverity::Error,
        "scanner-only error without any audit finding",
    ));
    let audit = minimal_policy_report();

    for policy in [AuditBuildPolicy::Strict, AuditBuildPolicy::Informational] {
        for accept_fail in [false, true] {
            for accept_unknown in [false, true] {
                let decision = evaluate_build_policy(
                    &compatibility,
                    &audit,
                    policy,
                    accept_fail,
                    accept_unknown,
                    &[],
                );
                match decision {
                    AuditPolicyDecision::Block { diagnostics } => {
                        assert_eq!(diagnostics.len(), 1, "one translated scanner blocker expected");
                        assert_eq!(diagnostics[0].code.0, DiagnosticCode::CMP_UNSUPPORTED_WIN32);
                        assert_eq!(
                            diagnostics[0].message,
                            "scanner-only error without any audit finding"
                        );
                        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
                    }
                    other => panic!(
                        "scanner-only Error must block (policy {policy:?}, accept_fail {accept_fail}, accept_unknown {accept_unknown}): {other:?}"
                    ),
                }
            }
        }
    }
}

#[test]
fn test_build_policy_downgraded_raw_native_codes_block_by_code() {
    for code in [
        DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY,
        DiagnosticCode::CMP_NODE_NATIVE_MODULE,
        DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION,
    ] {
        let mut compatibility = build_policy_compatibility(CompatibilityStatus::Compatible);
        compatibility.add_issue(compatibility_issue(
            code,
            DiagnosticSeverity::Warning,
            "downgraded native",
        ));
        let audit = minimal_policy_report();
        for policy in [AuditBuildPolicy::Strict, AuditBuildPolicy::Informational] {
            for accept_fail in [false, true] {
                for accept_unknown in [false, true] {
                    let decision = evaluate_build_policy(
                        &compatibility,
                        &audit,
                        policy,
                        accept_fail,
                        accept_unknown,
                        &[],
                    );
                    assert!(
                        matches!(decision, AuditPolicyDecision::Block { .. }),
                        "{code} at Warning severity must still block (policy {policy:?}, accept_fail {accept_fail}, accept_unknown {accept_unknown})"
                    );
                }
            }
        }
    }
}

#[test]
fn test_build_policy_unsupported_dependency_fallback_blocks_without_issues() {
    let compatibility = build_policy_compatibility(CompatibilityStatus::UnsupportedDependency);
    assert!(compatibility.issues.is_empty());
    let audit = minimal_policy_report();

    for policy in [AuditBuildPolicy::Strict, AuditBuildPolicy::Informational] {
        for accept_fail in [false, true] {
            for accept_unknown in [false, true] {
                let decision = evaluate_build_policy(
                    &compatibility,
                    &audit,
                    policy,
                    accept_fail,
                    accept_unknown,
                    &[],
                );
                match decision {
                    AuditPolicyDecision::Block { diagnostics } => {
                        assert_eq!(diagnostics.len(), 1);
                        assert_eq!(diagnostics[0].code.0, DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY);
                        assert_eq!(
                            diagnostics[0].message,
                            "game contains an unsupported native dependency"
                        );
                    }
                    other => panic!("UnsupportedDependency fallback must block: {other:?}"),
                }
            }
        }
    }
}

#[test]
fn test_build_policy_caller_runtime_blocker_alone_blocks() {
    let compatibility = build_policy_compatibility(CompatibilityStatus::Compatible);
    let audit = minimal_policy_report();
    let runtime = Diagnostic::error(
        DiagnosticCode::RT_UNRESOLVED,
        "no compatible runtime package found for test-engine",
    );

    for policy in [AuditBuildPolicy::Strict, AuditBuildPolicy::Informational] {
        for accept_fail in [false, true] {
            for accept_unknown in [false, true] {
                let decision = evaluate_build_policy(
                    &compatibility,
                    &audit,
                    policy,
                    accept_fail,
                    accept_unknown,
                    std::slice::from_ref(&runtime),
                );
                match decision {
                    AuditPolicyDecision::Block { diagnostics } => {
                        assert_eq!(
                            diagnostics,
                            vec![runtime.clone()],
                            "caller fact must be preserved verbatim"
                        );
                    }
                    other => panic!("caller RT_UNRESOLVED must block: {other:?}"),
                }
            }
        }
    }
}

#[test]
fn test_build_policy_audit_only_hard_codes_block() {
    for code in [
        DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY,
        DiagnosticCode::CMP_AUD_UNSAFE_PATH,
        "SIG-101",
        "CNT-101",
    ] {
        let compatibility = build_policy_compatibility(CompatibilityStatus::Compatible);
        let mut audit = minimal_policy_report();
        audit.findings = vec![hard_code_finding(code, "audit-only hard fact")];
        audit.recompute_outcome();
        assert_ne!(
            audit.outcome,
            AuditOutcome::Fail,
            "{code}: fixture must not rely on outcome"
        );

        for policy in [AuditBuildPolicy::Strict, AuditBuildPolicy::Informational] {
            let decision = evaluate_build_policy(&compatibility, &audit, policy, true, true, &[]);
            match decision {
                AuditPolicyDecision::Block { diagnostics } => {
                    assert_eq!(diagnostics.len(), 1, "{code}: exactly one audit blocker expected");
                    assert_eq!(diagnostics[0].code.0, code);
                }
                other => panic!("audit-only {code} must block: {other:?}"),
            }
        }
    }
}

#[test]
fn test_build_policy_deduplicates_same_fact_but_keeps_distinct_reasons() {
    let mut same = build_policy_compatibility(CompatibilityStatus::Compatible);
    same.add_issue(compatibility_issue(
        DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY,
        DiagnosticSeverity::Warning,
        "native payload lib/x.so",
    ));
    let mut audit = minimal_policy_report();
    audit.findings = vec![hard_code_finding(
        DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY,
        "native payload lib/x.so",
    )];
    audit.recompute_outcome();
    let decision =
        evaluate_build_policy(&same, &audit, AuditBuildPolicy::Strict, false, false, &[]);
    match decision {
        AuditPolicyDecision::Block { diagnostics } => {
            assert_eq!(
                diagnostics.len(),
                1,
                "the same fact on both surfaces must appear once"
            );
            assert_eq!(diagnostics[0].message, "native payload lib/x.so");
        }
        other => panic!("duplicate native fact must block: {other:?}"),
    }

    let mut distinct = build_policy_compatibility(CompatibilityStatus::Compatible);
    distinct.add_issue(compatibility_issue(
        DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY,
        DiagnosticSeverity::Warning,
        "compatibility reason",
    ));
    let mut audit = minimal_policy_report();
    audit.findings = vec![hard_code_finding(
        DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY,
        "audit reason",
    )];
    audit.recompute_outcome();
    let decision =
        evaluate_build_policy(&distinct, &audit, AuditBuildPolicy::Strict, false, false, &[]);
    match decision {
        AuditPolicyDecision::Block { diagnostics } => {
            assert_eq!(diagnostics.len(), 2, "distinct reasons must not be collapsed");
            assert_eq!(diagnostics[0].message, "compatibility reason");
            assert_eq!(diagnostics[1].message, "audit reason");
        }
        other => panic!("distinct native reasons must block: {other:?}"),
    }
}

#[test]
fn test_build_policy_asset_outcomes_preserve_prior_override_matching() {
    let compatibility = build_policy_compatibility(CompatibilityStatus::Compatible);
    for outcome in [
        AuditOutcome::Pass,
        AuditOutcome::Warning,
        AuditOutcome::Fail,
        AuditOutcome::Indeterminate,
    ] {
        let audit = {
            let mut report = minimal_policy_report();
            report.outcome = outcome;
            match outcome {
                AuditOutcome::Fail => {
                    report.findings =
                        vec![hard_code_finding("CMP-AUD-002", "missing required reference")]
                }
                AuditOutcome::Indeterminate => {
                    report.findings =
                        vec![hard_code_finding("CMP-AUD-006", "dynamic reference unresolved")]
                }
                AuditOutcome::Warning => {
                    report.findings =
                        vec![hard_code_finding("CMP-AUD-003", "optional reference missing")]
                }
                AuditOutcome::Pass => {}
            }
            report
        };
        let pristine = audit.clone();
        for policy in [AuditBuildPolicy::Strict, AuditBuildPolicy::Informational] {
            for accept_fail in [false, true] {
                for accept_unknown in [false, true] {
                    let expected = evaluate_audit_policy_with_mode(
                        &audit,
                        policy,
                        accept_fail,
                        accept_unknown,
                        &[],
                    );
                    let actual = evaluate_build_policy(
                        &compatibility,
                        &audit,
                        policy,
                        accept_fail,
                        accept_unknown,
                        &[],
                    );
                    assert_eq!(
                        actual, expected,
                        "build policy must preserve audit-only semantics for {outcome:?} (policy {policy:?}, accept_fail {accept_fail}, accept_unknown {accept_unknown})"
                    );
                }
            }
        }
        assert_eq!(audit.outcome, pristine.outcome, "input report must never be mutated");
        assert_eq!(
            serde_json::to_value(&audit.findings).unwrap(),
            serde_json::to_value(&pristine.findings).unwrap(),
            "input findings must never be mutated"
        );
        assert_eq!(
            serde_json::to_value(&audit.overrides).unwrap(),
            serde_json::to_value(&pristine.overrides).unwrap(),
            "input overrides must never be mutated"
        );
    }
}

#[test]
fn test_build_policy_case_mismatch_only_fail_is_characterized_not_rewritten() {
    let compatibility = build_policy_compatibility(CompatibilityStatus::Compatible);
    let mut audit = minimal_policy_report();
    audit.outcome = AuditOutcome::Fail;
    audit.findings = vec![AuditFinding {
        id: "case-mismatch".into(),
        code: DiagnosticCode::CMP_AUD_CASE_MISMATCH.to_string(),
        severity: FindingSeverity::Warning,
        disposition: FindingDisposition::Warning,
        requiredness: ReferenceRequiredness::Required,
        message: "assets/MAP001.png differs only by case".into(),
        source_node_ids: Vec::new(),
        orphan: false,
    }];
    for policy in [AuditBuildPolicy::Strict, AuditBuildPolicy::Informational] {
        for accept_fail in [false, true] {
            for accept_unknown in [false, true] {
                let expected =
                    evaluate_audit_policy_with_mode(&audit, policy, accept_fail, accept_unknown, &[]);
                let actual = evaluate_build_policy(
                    &compatibility,
                    &audit,
                    policy,
                    accept_fail,
                    accept_unknown,
                    &[],
                );
                assert_eq!(
                    actual, expected,
                    "case-mismatch-only FAIL must keep the prior behavior (policy {policy:?}, accept_fail {accept_fail}, accept_unknown {accept_unknown})"
                );
            }
        }
    }
    assert!(
        evaluate_build_policy(&compatibility, &audit, AuditBuildPolicy::Strict, false, false, &[])
            .is_allowed(),
        "case-mismatch-only FAIL is a characterized allowance and must not be tightened here"
    );
}

#[test]
fn test_build_policy_supported_compatibility_and_audit_pass_allow() {
    let mut compatibility = build_policy_compatibility(CompatibilityStatus::CompatibleWithShims);
    compatibility.add_issue(compatibility_issue(
        DiagnosticCode::CMP_DEPENDENCY_NOT_REQUIRED_ON_TARGET,
        DiagnosticSeverity::Warning,
        "advisory only",
    ));
    let audit = minimal_policy_report();
    for policy in [AuditBuildPolicy::Strict, AuditBuildPolicy::Informational] {
        let decision = evaluate_build_policy(&compatibility, &audit, policy, false, false, &[]);
        assert_eq!(
            decision,
            AuditPolicyDecision::Allow { overrides: Vec::new() },
            "supported compatibility with a passing audit must allow"
        );
    }
}

#[test]
fn test_build_policy_flags_never_waive_native_path_signature_toolchain_gate() {
    let mut compatibility = build_policy_compatibility(CompatibilityStatus::Compatible);
    compatibility.add_issue(compatibility_issue(
        DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY,
        DiagnosticSeverity::Warning,
        "native dll",
    ));
    let mut audit = minimal_policy_report();
    audit.findings = vec![hard_code_finding(
        DiagnosticCode::CMP_AUD_UNSAFE_PATH,
        "unsafe path escape",
    )];
    audit.recompute_outcome();
    let toolchain = Diagnostic::error(
        DiagnosticCode::SIG_TOOLCHAIN_MISSING,
        "signing toolchain missing",
    );

    for policy in [AuditBuildPolicy::Strict, AuditBuildPolicy::Informational] {
        let decision = evaluate_build_policy(
            &compatibility,
            &audit,
            policy,
            true,
            true,
            std::slice::from_ref(&toolchain),
        );
        match decision {
            AuditPolicyDecision::Block { diagnostics } => {
                let codes: Vec<&str> = diagnostics.iter().map(|d| d.code.0.as_str()).collect();
                assert_eq!(codes, vec!["CMP-102", "SIG-104", "CMP-AUD-009"]);
            }
            other => panic!("override flags must not waive hard gates: {other:?}"),
        }
    }
}

// =============================================================================
// Tahap 3 Task 3 — `--allow-unknown` forwards only `Unknown`
//
// Producers encode the typed class in the proof-context carrier they append to a
// retained hard finding. `build_policy::decide` is the single policy point; the
// adapter must keep every legacy hard gate verbatim and must never turn a
// non-waivable code into a forwardable finding.
// =============================================================================

fn proof_carrier(state: &str, code: &str, path: &str) -> String {
    // Mirrors the real producer shape: the caller's own note (which carries the
    // untrusted logical path) comes before the carrier, and the carrier is last.
    format!(
        "hard finding for '{path}'. (activation {state}) [proof: state=\"{state}\"; original=\"{code}\"; file=\"{path}\"]"
    )
}

fn unknown_carrier_issue(path: &str) -> CompatibilityIssue {
    CompatibilityIssue {
        code: DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE.to_string(),
        severity: DiagnosticSeverity::Error,
        message: proof_carrier(
            "unknown",
            DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE,
            path,
        ),
        source_path: Some(path.to_string()),
        remediation: None,
    }
}

fn unknown_carrier_finding(path: &str) -> AuditFinding {
    AuditFinding {
        id: format!("{path}-unknown"),
        code: DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE.to_string(),
        severity: FindingSeverity::Error,
        disposition: FindingDisposition::Blocking,
        requiredness: ReferenceRequiredness::Required,
        message: proof_carrier(
            "unknown",
            DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE,
            path,
        ),
        source_node_ids: vec![path.to_string()],
        orphan: false,
    }
}

#[test]
fn test_allow_unknown_forwards_unknown_only_and_build_proceeds() {
    let mut compatibility = build_policy_compatibility(CompatibilityStatus::Compatible);
    compatibility.add_issue(unknown_carrier_issue("js/plugins/Guarded.js"));
    let mut audit = minimal_policy_report();
    audit.findings = vec![unknown_carrier_finding("js/plugins/Guarded.js")];
    audit.recompute_outcome();

    // Without the flag the unknown blocks, under both audit policies.
    for policy in [AuditBuildPolicy::Strict, AuditBuildPolicy::Informational] {
        match evaluate_build_policy(&compatibility, &audit, policy, false, false, &[]) {
            AuditPolicyDecision::Block { diagnostics } => assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code.0 == DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE),
                "the unknown must block by its own code: {diagnostics:?}"
            ),
            other => panic!("Unknown must block without --allow-unknown ({policy:?}): {other:?}"),
        }
    }

    let decision = evaluate_build_policy_typed(
        &compatibility,
        &audit,
        AuditBuildPolicy::Strict,
        BuildPolicy::Experimental,
        false,
        false,
        &[],
    );
    assert!(decision.is_allowed(), "the forwarded unknown must not block");
    assert!(decision.is_experimental());
    assert_eq!(decision.status, ReportStatus::Experimental);
    assert_eq!(
        decision.forwarded.len(),
        1,
        "one fact reported by both surfaces must be forwarded once: {:?}",
        decision.forwarded
    );
    assert_eq!(
        decision.forwarded[0].code,
        DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE
    );
    assert_eq!(
        decision.forwarded[0].source_path.as_deref(),
        Some("js/plugins/Guarded.js"),
        "the forwarded unknown must carry its site"
    );
    assert_eq!(
        decision.forwarded_codes(),
        vec![DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE.to_string()]
    );
}

#[test]
fn test_allow_unknown_cannot_bypass_raw_native_codes() {
    for code in [
        DiagnosticCode::CMP_NATIVE_DLL_DEPENDENCY,
        DiagnosticCode::CMP_NODE_NATIVE_MODULE,
        DiagnosticCode::CMP_PYTHON_NATIVE_EXTENSION,
    ] {
        // Even a crafted carrier claiming `unknown` must not make the raw native
        // code forwardable.
        let mut compatibility = build_policy_compatibility(CompatibilityStatus::Compatible);
        compatibility.add_issue(CompatibilityIssue {
            code: code.to_string(),
            severity: DiagnosticSeverity::Error,
            message: proof_carrier("unknown", code, "js/plugins/addon.node"),
            source_path: Some("js/plugins/addon.node".to_string()),
            remediation: None,
        });
        let mut audit = minimal_policy_report();
        audit.findings = vec![AuditFinding {
            id: format!("{code}-crafted"),
            code: code.to_string(),
            severity: FindingSeverity::Error,
            disposition: FindingDisposition::Blocking,
            requiredness: ReferenceRequiredness::Required,
            message: proof_carrier("unknown", code, "js/plugins/addon.node"),
            source_node_ids: vec!["js/plugins/addon.node".to_string()],
            orphan: false,
        }];
        audit.recompute_outcome();

        for policy in [AuditBuildPolicy::Strict, AuditBuildPolicy::Informational] {
            let decision = evaluate_build_policy_typed(
                &compatibility,
                &audit,
                policy,
                BuildPolicy::Experimental,
                true,
                true,
                &[],
            );
            assert!(!decision.is_allowed(), "{code} must never be forwarded");
            assert!(
                decision.forwarded.is_empty(),
                "{code} must not appear as a forwarded unknown: {:?}",
                decision.forwarded
            );
            match decision.decision {
                AuditPolicyDecision::Block { diagnostics } => assert!(
                    diagnostics.iter().any(|d| d.code.0 == code),
                    "{code} must stay a legacy hard blocker: {diagnostics:?}"
                ),
                other => panic!("{code} must block: {other:?}"),
            }
        }
    }
}

#[test]
fn test_allow_unknown_cannot_bypass_security_prefixes_or_runtime_gate() {
    let compatibility = build_policy_compatibility(CompatibilityStatus::Compatible);
    let mut audit = minimal_policy_report();
    audit.findings = vec![AuditFinding {
        id: "sec-crafted".into(),
        code: "SEC-101".into(),
        severity: FindingSeverity::Warning,
        disposition: FindingDisposition::Warning,
        requiredness: ReferenceRequiredness::Required,
        message: proof_carrier("unknown", "SEC-101", "assets/../escape"),
        source_node_ids: vec!["assets/../escape".to_string()],
        orphan: false,
    }];
    audit.recompute_outcome();
    let runtime = Diagnostic::error(
        DiagnosticCode::RT_UNRESOLVED,
        "no compatible runtime package found for test-engine",
    );

    let decision = evaluate_build_policy_typed(
        &compatibility,
        &audit,
        AuditBuildPolicy::Informational,
        BuildPolicy::Experimental,
        true,
        true,
        std::slice::from_ref(&runtime),
    );
    assert!(!decision.is_allowed(), "security and runtime gates stay closed");
    match decision.decision {
        AuditPolicyDecision::Block { diagnostics } => {
            let codes: Vec<&str> = diagnostics.iter().map(|d| d.code.0.as_str()).collect();
            assert!(codes.contains(&"SEC-101"), "security prefix must block: {codes:?}");
            assert!(codes.contains(&"RT-101"), "runtime gate must block: {codes:?}");
        }
        other => panic!("crafted unknown must not waive security: {other:?}"),
    }
}

#[test]
fn test_allow_unknown_never_clears_blocked_class_or_legacy_diagnostic() {
    // A `Blocked` class (carrier state `required`) blocks under both policies.
    let mut compatibility = build_policy_compatibility(CompatibilityStatus::Compatible);
    compatibility.add_issue(CompatibilityIssue {
        code: DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE.to_string(),
        severity: DiagnosticSeverity::Error,
        message: proof_carrier(
            "required",
            DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE,
            "js/plugins/Unguarded.js",
        ),
        source_path: Some("js/plugins/Unguarded.js".to_string()),
        remediation: None,
    });
    let mut audit = minimal_policy_report();
    audit.findings = vec![unknown_carrier_finding("js/plugins/Guarded.js")];
    audit.recompute_outcome();
    let legacy = Diagnostic::error("CMP-101", "legacy finding without a class");

    for policy in [BuildPolicy::Default, BuildPolicy::Experimental] {
        let decision = evaluate_build_policy_typed(
            &compatibility,
            &audit,
            AuditBuildPolicy::Informational,
            policy,
            false,
            false,
            std::slice::from_ref(&legacy),
        );
        assert!(!decision.is_allowed(), "Blocked and legacy facts always block");
        match decision.decision {
            AuditPolicyDecision::Block { diagnostics } => {
                let codes: Vec<&str> = diagnostics.iter().map(|d| d.code.0.as_str()).collect();
                assert!(
                    codes.contains(&DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE),
                    "the Blocked finding must block: {codes:?}"
                );
                assert!(codes.contains(&"CMP-101"), "legacy fact must block: {codes:?}");
            }
            other => panic!("Blocked/legacy must block under {policy:?}: {other:?}"),
        }
    }
}

#[test]
fn test_default_policy_reports_unknown_never_compatible() {
    let mut compatibility = build_policy_compatibility(CompatibilityStatus::Compatible);
    compatibility.add_issue(unknown_carrier_issue("js/plugins/Guarded.js"));
    let audit = minimal_policy_report();

    let decision = evaluate_build_policy_typed(
        &compatibility,
        &audit,
        AuditBuildPolicy::Informational,
        BuildPolicy::Default,
        false,
        false,
        &[],
    );
    assert!(!decision.is_allowed());
    assert_ne!(
        decision.status,
        ReportStatus::Compatible,
        "an unproven build must never be reported Compatible"
    );
    assert!(decision.forwarded.is_empty(), "Default policy forwards nothing");
}

// =============================================================================
// Tahap 3 debt closure — review L2 negative coverage
//
// Three negative contracts are pinned against the real adapter, never against an
// invented one:
//   1. A crafted proof carrier cannot waive `CMP-AUD-009` path safety: the
//      hard-code filter runs before the carrier is read, so the raw code blocks
//      under both audit policies, with both accept flags, and under `Default` and
//      `Experimental` alike.
//   2. Caller hard blockers (`BLD-101`, `SIG-104`) stay blocked under
//      `Experimental` even when a `CMP-104` unknown is genuinely forwarded.
//   3. An `UnsupportedDependency` report carrying one typed `Unknown` blocks under
//      `Default` and is allowed under `Experimental`.
//   4. An `UnsupportedDependency` report carrying one typed `Advisory` stays
//      blocked: a non-gating class may not suppress the synthetic `CMP-102`
//      fallback. Pinned in the core adapter unit tests
//      (`unsupported_dependency_fallback_survives_a_non_gating_typed_finding`)
//      rather than here, because the suppression rule lives in that module.
// =============================================================================

#[test]
fn test_cmp_aud_009_crafted_unknown_cannot_waive_path_safety() {
    // The finding is downgraded to Warning/Warning so the report itself is a
    // warning, never an already-failing one: the block must come from the raw
    // code, not from the audit outcome.
    let mut audit = minimal_policy_report();
    audit.findings = vec![AuditFinding {
        id: "cmp-aud-009-crafted-unknown".into(),
        code: DiagnosticCode::CMP_AUD_UNSAFE_PATH.to_string(),
        severity: FindingSeverity::Warning,
        disposition: FindingDisposition::Warning,
        requiredness: ReferenceRequiredness::Optional,
        message: proof_carrier(
            "unknown",
            DiagnosticCode::CMP_AUD_UNSAFE_PATH,
            "assets/../escape.png",
        ),
        source_node_ids: vec!["assets/../escape.png".to_string()],
        orphan: false,
    }];
    audit.recompute_outcome();
    assert_eq!(
        audit.outcome,
        AuditOutcome::Warning,
        "the fixture must be a downgraded advisory report, not a FAIL"
    );

    let compatibility = build_policy_compatibility(CompatibilityStatus::Compatible);

    for policy in [AuditBuildPolicy::Strict, AuditBuildPolicy::Informational] {
        for accept_fail in [false, true] {
            for accept_unknown in [false, true] {
                // Audit-only gate: the raw path-safety code blocks by code.
                match evaluate_audit_policy_with_mode(
                    &audit,
                    policy,
                    accept_fail,
                    accept_unknown,
                    &[],
                ) {
                    AuditPolicyDecision::Block { diagnostics } => {
                        assert_eq!(
                            diagnostics.len(),
                            1,
                            "exactly one path-safety blocker expected (policy {policy:?}, \
                             accept_fail {accept_fail}, accept_unknown {accept_unknown})"
                        );
                        assert_eq!(
                            diagnostics[0].code.0,
                            DiagnosticCode::CMP_AUD_UNSAFE_PATH
                        );
                    }
                    other => panic!(
                        "CMP-AUD-009 must block on the audit gate (policy {policy:?}, \
                         accept_fail {accept_fail}, accept_unknown {accept_unknown}): {other:?}"
                    ),
                }

                // Legacy build gate (Default policy) keeps the same block.
                match evaluate_build_policy(
                    &compatibility,
                    &audit,
                    policy,
                    accept_fail,
                    accept_unknown,
                    &[],
                ) {
                    AuditPolicyDecision::Block { diagnostics } => {
                        assert_eq!(diagnostics.len(), 1);
                        assert_eq!(
                            diagnostics[0].code.0,
                            DiagnosticCode::CMP_AUD_UNSAFE_PATH
                        );
                    }
                    other => panic!(
                        "CMP-AUD-009 must block on the build gate (policy {policy:?}, \
                         accept_fail {accept_fail}, accept_unknown {accept_unknown}): {other:?}"
                    ),
                }

                // Typed gates: the carrier is never read for a non-waivable code,
                // so neither policy can forward or clear it.
                for build_policy in [BuildPolicy::Default, BuildPolicy::Experimental] {
                    let decision = evaluate_build_policy_typed(
                        &compatibility,
                        &audit,
                        policy,
                        build_policy,
                        accept_fail,
                        accept_unknown,
                        &[],
                    );
                    assert!(
                        !decision.is_allowed(),
                        "CMP-AUD-009 crafted unknown must not be waivable \
                         (policy {policy:?}, build_policy {build_policy:?}, \
                         accept_fail {accept_fail}, accept_unknown {accept_unknown})"
                    );
                    assert!(
                        decision.forwarded.is_empty(),
                        "CMP-AUD-009 must never be forwarded as an unknown: {:?}",
                        decision.forwarded
                    );
                    match decision.decision {
                        AuditPolicyDecision::Block { diagnostics } => {
                            assert_eq!(diagnostics.len(), 1);
                            assert_eq!(
                                diagnostics[0].code.0,
                                DiagnosticCode::CMP_AUD_UNSAFE_PATH
                            );
                        }
                        other => panic!(
                            "CMP-AUD-009 must block (policy {policy:?}, \
                             build_policy {build_policy:?}): {other:?}"
                        ),
                    }
                }
            }
        }
    }
}

#[test]
fn test_experimental_forwarded_unknown_never_waives_caller_hard_blockers() {
    let mut compatibility = build_policy_compatibility(CompatibilityStatus::Compatible);
    compatibility.add_issue(unknown_carrier_issue("js/plugins/Guarded.js"));
    let mut audit = minimal_policy_report();
    audit.findings = vec![unknown_carrier_finding("js/plugins/Guarded.js")];
    audit.recompute_outcome();

    let toolchain = Diagnostic::error(
        DiagnosticCode::BLD_TOOLCHAIN_MISSING,
        "toolchain missing for test-engine",
    );
    let signing = Diagnostic::error(
        DiagnosticCode::SIG_TOOLCHAIN_MISSING,
        "signing toolchain missing",
    );
    let caller_facts = [toolchain, signing];

    for policy in [AuditBuildPolicy::Strict, AuditBuildPolicy::Informational] {
        for accept_fail in [false, true] {
            for accept_unknown in [false, true] {
                let decision = evaluate_build_policy_typed(
                    &compatibility,
                    &audit,
                    policy,
                    BuildPolicy::Experimental,
                    accept_fail,
                    accept_unknown,
                    &caller_facts,
                );
                assert!(
                    !decision.is_allowed(),
                    "caller hard blockers must survive Experimental forwarding \
                     (policy {policy:?}, accept_fail {accept_fail}, \
                     accept_unknown {accept_unknown})"
                );
                assert!(
                    decision.is_experimental(),
                    "the experimental policy must be recorded even when it blocks"
                );
                assert_eq!(
                    decision.forwarded_codes(),
                    vec![DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE.to_string()],
                    "the CMP-104 unknown must still be forwarded exactly once: {:?}",
                    decision.forwarded
                );
                match decision.decision {
                    AuditPolicyDecision::Block { diagnostics } => {
                        let codes: Vec<&str> =
                            diagnostics.iter().map(|d| d.code.0.as_str()).collect();
                        assert_eq!(
                            codes,
                            vec!["BLD-101", "SIG-104"],
                            "only the caller hard blockers may block once CMP-104 is forwarded"
                        );
                        assert!(
                            !codes.contains(&DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE),
                            "the forwarded unknown must not also appear as a blocker: {codes:?}"
                        );
                    }
                    other => panic!(
                        "caller hard blockers must block (policy {policy:?}): {other:?}"
                    ),
                }
            }
        }
    }
}

#[test]
fn test_unsupported_dependency_typed_unknown_blocks_default_allows_experimental() {
    // Policy matrix: Default must block a typed Required Unknown, Experimental must
    // forward it. The synthetic CMP-102 fallback's suppression rule is pinned by
    // the core adapter unit tests, not here.
    let mut compatibility = build_policy_compatibility(CompatibilityStatus::UnsupportedDependency);
    compatibility.add_issue(unknown_carrier_issue("js/plugins/Guarded.js"));
    let audit = minimal_policy_report();

    for policy in [AuditBuildPolicy::Strict, AuditBuildPolicy::Informational] {
        let default_decision = evaluate_build_policy_typed(
            &compatibility,
            &audit,
            policy,
            BuildPolicy::Default,
            false,
            false,
            &[],
        );
        assert!(
            !default_decision.is_allowed(),
            "a typed Unknown must still block under Default (policy {policy:?})"
        );
        assert!(
            default_decision.forwarded.is_empty(),
            "Default policy forwards nothing"
        );
        match default_decision.decision {
            AuditPolicyDecision::Block { diagnostics } => {
                let codes: Vec<&str> = diagnostics.iter().map(|d| d.code.0.as_str()).collect();
                assert!(
                    codes.contains(&DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE),
                    "the typed issue must be among the Default blockers: {codes:?}"
                );
            }
            other => panic!("typed Unknown must block under Default (policy {policy:?}): {other:?}"),
        }

        let experimental = evaluate_build_policy_typed(
            &compatibility,
            &audit,
            policy,
            BuildPolicy::Experimental,
            false,
            false,
            &[],
        );
        assert!(
            experimental.is_allowed(),
            "the typed Unknown must be forwardable under Experimental (policy {policy:?})"
        );
        assert_eq!(experimental.status, ReportStatus::Experimental);
        assert_eq!(
            experimental.forwarded_codes(),
            vec![DiagnosticCode::CMP_NODE_UNSUPPORTED_REQUIRE.to_string()]
        );
    }
}
