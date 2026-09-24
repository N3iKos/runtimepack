use runtimepack_core::asset_audit::{
    AssetAuditOptions, AssetAuditorRegistry, AuditMode, AuditOutcome,
};
use runtimepack_core::detection::EngineDescriptor;
use runtimepack_core::domain::{DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_engine_renpy::RenPyAssetAuditor;
use runtimepack_engine_rgss::RgssAssetAuditor;
use runtimepack_engine_rpgmaker_web::RpgMakerWebAssetAuditor;
use runtimepack_engine_tyrano::TyranoAssetAuditor;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn create_test_registry() -> AssetAuditorRegistry {
    let mut registry = AssetAuditorRegistry::new();
    registry
        .register(Box::new(RgssAssetAuditor))
        .expect("RGSS auditor must register");
    registry
        .register(Box::new(RpgMakerWebAssetAuditor))
        .expect("RPG Maker Web auditor must register");
    registry
        .register(Box::new(RenPyAssetAuditor))
        .expect("Ren'Py auditor must register");
    registry
        .register(Box::new(TyranoAssetAuditor))
        .expect("Tyrano auditor must register");
    registry
}

fn fixture(name: &str) -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap_or(manifest_dir);
    root.join("tests").join("fixtures").join(name)
}

fn engine_desc(engine: EngineId) -> EngineDescriptor {
    EngineDescriptor {
        id: engine,
        confidence: DetectionConfidence::Exact,
        score: 100,
        evidence: Vec::new(),
        conflicts: Vec::new(),
        detected_version: None,
        overridden: false,
    }
}

fn hash_directory(root: &Path) -> BTreeMap<PathBuf, String> {
    fn visit(root: &Path, current: &Path, hashes: &mut BTreeMap<PathBuf, String>) {
        let mut entries = fs::read_dir(current)
            .unwrap()
            .map(|e| e.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, hashes);
            } else {
                let content = fs::read(&path).unwrap();
                let mut hasher = Sha256::new();
                hasher.update(&content);
                let hex_hash = format!("{:x}", hasher.finalize());
                let rel = path.strip_prefix(root).unwrap().to_path_buf();
                hashes.insert(rel, hex_hash);
            }
        }
    }
    let mut hashes = BTreeMap::new();
    visit(root, root, &mut hashes);
    hashes
}

const V1_ENGINES: &[(EngineId, &str)] = &[
    (EngineId::RpgMakerXp, "xp_min"),
    (EngineId::RpgMakerVx, "vx_min"),
    (EngineId::RpgMakerVxAce, "vxace_min"),
    (EngineId::RpgMakerMv, "mv_min"),
    (EngineId::RpgMakerMz, "mz_min"),
    (EngineId::RenPy, "renpy_supported_version_A"),
    (EngineId::Tyrano, "tyrano_min"),
];

#[test]
fn every_v1_engine_resolves_to_a_real_auditor() {
    let registry = create_test_registry();

    for &(engine_id, fixture_name) in V1_ENGINES {
        let source_dir = fixture(fixture_name);
        let source_index = GameSourceIndex::build(&source_dir).unwrap();
        let desc = engine_desc(engine_id);

        let options = AssetAuditOptions {
            mode: AuditMode::Standard,
            allow_network: false,
            ..AssetAuditOptions::default()
        };

        let report = registry.audit(&source_index, &desc, &options);

        assert_eq!(
            report.engine, engine_id,
            "report engine should match for {fixture_name}"
        );
        assert_eq!(
            report.schema_version, 1,
            "schema_version must be 1 for {fixture_name}"
        );

        // Verify that the report was NOT produced by an unavailable placeholder
        let has_unavailable_finding = report
            .findings
            .iter()
            .any(|f| f.code == "CMP-AUD-001" && f.id.ends_with("-unavailable"));
        assert!(
            !has_unavailable_finding,
            "engine {engine_id:?} should have a real auditor, not unavailable placeholder"
        );

        // Verify that it didn't fail with 'no asset auditor is registered'
        let has_unregistered_finding = report
            .findings
            .iter()
            .any(|f| f.message.contains("no asset auditor is registered"));
        assert!(
            !has_unregistered_finding,
            "engine {engine_id:?} must have an auditor registered"
        );
    }
}

#[test]
fn generic_html_remains_out_of_scope_and_unsupported() {
    let registry = create_test_registry();
    let source_dir = fixture("generic_html_negative");
    let source_index = GameSourceIndex::build(&source_dir).unwrap();
    let desc = engine_desc(EngineId::Unknown);

    let options = AssetAuditOptions {
        mode: AuditMode::Standard,
        allow_network: false,
        ..AssetAuditOptions::default()
    };

    let report = registry.audit(&source_index, &desc, &options);

    assert_eq!(report.engine, EngineId::Unknown);
    assert_eq!(report.outcome, AuditOutcome::Indeterminate);
    assert!(!report.completeness.required_scope_complete);

    let finding = report
        .findings
        .iter()
        .find(|f| f.code == "CMP-AUD-001")
        .expect("generic HTML / unknown engine must emit CMP-AUD-001");
    assert!(
        finding
            .message
            .contains("no asset auditor is registered for the selected engine"),
        "unexpected message: {}",
        finding.message
    );
}

#[test]
fn report_generation_is_deterministic_across_all_v1_engines() {
    let registry = create_test_registry();

    for &(engine_id, fixture_name) in V1_ENGINES {
        let source_dir = fixture(fixture_name);
        let source_index = GameSourceIndex::build(&source_dir).unwrap();
        let desc = engine_desc(engine_id);

        for mode in [AuditMode::Standard, AuditMode::Deep] {
            let options = AssetAuditOptions {
                mode,
                allow_network: false,
                ..AssetAuditOptions::default()
            };

            let report_first = registry.audit(&source_index, &desc, &options);
            let report_second = registry.audit(&source_index, &desc, &options);

            let json_first = serde_json::to_string_pretty(&report_first).unwrap();
            let json_second = serde_json::to_string_pretty(&report_second).unwrap();

            assert_eq!(
                json_first, json_second,
                "audit report must be deterministic for {fixture_name} in mode {mode:?}"
            );
        }
    }
}

#[test]
fn deep_coverage_is_superset_of_standard_coverage_across_all_v1_engines() {
    let registry = create_test_registry();

    for &(engine_id, fixture_name) in V1_ENGINES {
        let source_dir = fixture(fixture_name);
        let source_index = GameSourceIndex::build(&source_dir).unwrap();
        let desc = engine_desc(engine_id);

        let std_options = AssetAuditOptions {
            mode: AuditMode::Standard,
            allow_network: false,
            ..AssetAuditOptions::default()
        };
        let deep_options = AssetAuditOptions {
            mode: AuditMode::Deep,
            allow_network: false,
            ..AssetAuditOptions::default()
        };

        let std_report = registry.audit(&source_index, &desc, &std_options);
        let deep_report = registry.audit(&source_index, &desc, &deep_options);

        // Covered reference classes in standard must all be present in deep
        let deep_covered: HashSet<_> = deep_report
            .completeness
            .covered_reference_classes
            .iter()
            .collect();
        for covered_class in &std_report.completeness.covered_reference_classes {
            assert!(
                deep_covered.contains(covered_class),
                "deep coverage in {fixture_name} missing class {covered_class:?} covered by standard"
            );
        }

        // Deep statistics should be greater than or equal to standard statistics
        assert!(
            deep_report.statistics.nodes >= std_report.statistics.nodes,
            "deep node count ({}) must be >= standard node count ({}) for {fixture_name}",
            deep_report.statistics.nodes,
            std_report.statistics.nodes
        );
        assert!(
            deep_report.statistics.references >= std_report.statistics.references,
            "deep reference count ({}) must be >= standard reference count ({}) for {fixture_name}",
            deep_report.statistics.references,
            std_report.statistics.references
        );

        // Deep node IDs must be a superset of standard node IDs
        let std_node_ids: HashSet<_> = std_report.graph.nodes.iter().map(|n| &n.id).collect();
        let deep_node_ids: HashSet<_> = deep_report.graph.nodes.iter().map(|n| &n.id).collect();
        assert!(
            std_node_ids.is_subset(&deep_node_ids),
            "deep graph node IDs must be a superset of standard graph node IDs for {fixture_name}"
        );
    }
}

#[test]
fn source_directory_remains_100_percent_immutable_before_and_after_audit() {
    let registry = create_test_registry();

    let all_test_fixtures = [
        "xp_min",
        "vx_min",
        "vxace_min",
        "mv_min",
        "mz_min",
        "renpy_supported_version_A",
        "tyrano_min",
        "generic_html_negative",
    ];

    for fixture_name in all_test_fixtures {
        let source_dir = fixture(fixture_name);
        let hashes_before = hash_directory(&source_dir);

        let source_index = GameSourceIndex::build(&source_dir).unwrap();
        let desc = engine_desc(match fixture_name {
            "xp_min" => EngineId::RpgMakerXp,
            "vx_min" => EngineId::RpgMakerVx,
            "vxace_min" => EngineId::RpgMakerVxAce,
            "mv_min" => EngineId::RpgMakerMv,
            "mz_min" => EngineId::RpgMakerMz,
            "renpy_supported_version_A" => EngineId::RenPy,
            "tyrano_min" => EngineId::Tyrano,
            _ => EngineId::Unknown,
        });

        // Run both standard and deep audit
        let _ = registry.audit(
            &source_index,
            &desc,
            &AssetAuditOptions {
                mode: AuditMode::Standard,
                allow_network: false,
                ..AssetAuditOptions::default()
            },
        );
        let _ = registry.audit(
            &source_index,
            &desc,
            &AssetAuditOptions {
                mode: AuditMode::Deep,
                allow_network: false,
                ..AssetAuditOptions::default()
            },
        );

        let hashes_after = hash_directory(&source_dir);
        assert_eq!(
            hashes_before, hashes_after,
            "source directory {fixture_name} was mutated during audit!"
        );
    }
}

#[test]
fn cli_binary_executes_real_auditors_for_all_v1_engines() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    for &(engine_id, fixture_name) in V1_ENGINES {
        let working_dir = std::env::temp_dir().join(format!(
            "runtimepack-contract-cli-{fixture_name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&working_dir).unwrap();
        let source = fixture(fixture_name);
        let report_path = working_dir.join("report.json");

        let output = Command::new(env!("CARGO_BIN_EXE_runtimepack"))
            .current_dir(&working_dir)
            .arg("audit")
            .arg(source.to_str().unwrap())
            .arg("--output")
            .arg(report_path.to_str().unwrap())
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "CLI audit failed for {fixture_name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let content = fs::read(&report_path).unwrap();
        let report: serde_json::Value = serde_json::from_slice(&content).unwrap();

        let expected_engine = serde_json::to_value(engine_id).unwrap();
        assert_eq!(report["schema_version"], 1);
        assert_eq!(report["engine"], expected_engine);
        assert_eq!(report["runtime_validation"], "NOT_RUN");

        let findings = report["findings"].as_array().unwrap();
        for finding in findings {
            assert_ne!(
                finding["code"].as_str(),
                Some("CMP-AUD-001"),
                "CLI report for {fixture_name} should not contain CMP-AUD-001"
            );
        }

        fs::remove_dir_all(working_dir).unwrap();
    }
}
