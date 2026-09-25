use runtimepack_core::asset_audit::{
    render_canonical_artifacts, write_audit_artifacts, AssetAuditReport, AssetKind, AssetNode,
    AuditCache, AuditCacheKey, AuditCompleteness, AuditFinding, AuditLimits, AuditMode,
    AuditOutcome, AuditOverride, AuditStatistics, DependencyGraph, EvidenceStage, EvidenceTrail,
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
        message: r"Missing file at C:\Users\SecretUser\Projects\Game\Data\System.json".into(),
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
