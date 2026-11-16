use runtimepack_core::diagnostics::DiagnosticCode;
use runtimepack_core::ingest::{GameSourceIndex, IndexedFile};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn create_test_game_dir(name: &str) -> PathBuf {
    let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "runtimepack_scoped_test_{}_{}_{}",
        name,
        std::process::id(),
        id
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_scoped_index_lexical_traversal_rejection() {
    let dir = create_test_game_dir("traversal");
    let file_path = dir.join("index.html");
    fs::write(&file_path, "<html></html>").unwrap();

    let index = GameSourceIndex::build(&dir).unwrap();

    let invalid_targets = [
        "../outside",
        "C:\\game",
        "\\\\server\\share",
        "dir\0name",
        "/absolute/unix",
        "\\absolute\\win",
        "nested/../../escape",
        "http://example.com/payload",
    ];

    for target in &invalid_targets {
        let res = index.scoped_to(target);
        assert!(
            res.is_err(),
            "Expected error for target '{}', but got Ok",
            target
        );
        let err = res.unwrap_err();
        assert_eq!(
            err.code(),
            Some(DiagnosticCode::CNT_TRAVERSAL_REJECTED),
            "Expected CNT-103 for '{}', got '{:?}'",
            target,
            err.code()
        );
    }
}

#[test]
fn test_scoped_index_symlink_escape_rejection() {
    let base = create_test_game_dir("symlink_escape");
    let game_root = base.join("game");
    let outside_root = base.join("outside");
    fs::create_dir_all(&game_root).unwrap();
    fs::create_dir_all(&outside_root).unwrap();

    fs::write(outside_root.join("secret.txt"), "secret data").unwrap();
    fs::write(game_root.join("game.txt"), "game data").unwrap();

    let index = GameSourceIndex::build(&game_root).unwrap();

    let link_target = game_root.join("nested_link");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside_root, &link_target).unwrap();
    }
    #[cfg(windows)]
    {
        let _ = std::os::windows::fs::symlink_dir(&outside_root, &link_target);
    }

    let res = index.scoped_to("nested_link");
    assert!(
        res.is_err(),
        "Expected symlink escape rejection, but got Ok"
    );
    let err = res.unwrap_err();
    assert_eq!(
        err.code(),
        Some(DiagnosticCode::ING_SYMLINK_ESCAPE),
        "Expected ING-102, got '{:?}'",
        err.code()
    );
}

#[test]
fn test_scoped_index_dangling_file_symlink_escape_rejection() {
    let base = create_test_game_dir("dangling_file_symlink");
    let game_root = base.join("game");
    let nested_dir = game_root.join("bin").join("www");
    fs::create_dir_all(&nested_dir).unwrap();

    let valid_file = nested_dir.join("index.html");
    fs::write(&valid_file, "<html></html>").unwrap();

    // Create a dangling symlink inside the payload directory pointing to a nonexistent outside file
    let dangling_link = nested_dir.join("broken.js");
    let nonexistent_target = base.join("outside_nonexistent.js");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&nonexistent_target, &dangling_link).unwrap();
    }
    #[cfg(windows)]
    {
        let _ = std::os::windows::fs::symlink_file(&nonexistent_target, &dangling_link);
    }

    let index = GameSourceIndex::build(&game_root).unwrap();

    let res = index.scoped_to("bin/www");
    assert!(
        res.is_err(),
        "Expected dangling symlink to fail closed with ING-102, but got Ok"
    );
    let err = res.unwrap_err();
    assert_eq!(
        err.code(),
        Some(DiagnosticCode::ING_SYMLINK_ESCAPE),
        "Expected ING-102 for dangling symlink, got '{:?}'",
        err.code()
    );
}

#[test]
fn test_scoped_index_component_rerooting() {
    let dir = create_test_game_dir("reroot");
    let nested_dir = dir.join("bin").join("www");
    fs::create_dir_all(&nested_dir).unwrap();

    let index_file = nested_dir.join("index.html");
    let script_file = nested_dir.join("game.js");
    let outer_file = dir.join("nw.exe");

    fs::write(&index_file, "<html></html>").unwrap();
    fs::write(&script_file, "console.log('hi');").unwrap();
    fs::write(&outer_file, "binary").unwrap();

    let index = GameSourceIndex::build(&dir).unwrap();
    let original_hash = index.source_hash.clone();

    let scoped = index.scoped_to("bin/www").unwrap();

    // Invariants
    assert_eq!(
        scoped.root_path, index.root_path,
        "Physical root_path must be preserved"
    );
    assert_eq!(
        scoped.source_hash, original_hash,
        "Original source_hash must be preserved"
    );
    assert_eq!(
        scoped.content_root,
        Some("bin/www".to_string()),
        "content_root carrier must match canonical subpath"
    );
    assert!(
        scoped.scoped_content_hash.is_some(),
        "scoped_content_hash must be computed"
    );
    assert_ne!(
        scoped.scoped_content_hash.as_deref(),
        Some(original_hash.as_str()),
        "scoped hash must differ from full tree hash when tree differs"
    );

    // Logical path re-rooting
    assert!(
        scoped.has_file("index.html"),
        "Remapped index.html must be present"
    );
    assert!(
        scoped.has_file("game.js"),
        "Remapped game.js must be present"
    );
    assert!(
        !scoped.has_file("bin/www/index.html"),
        "Original nested key must not be at scoped root"
    );
    assert!(
        !scoped.has_file("nw.exe"),
        "Outer file must be excluded from scoped index"
    );

    // Physical path resolution
    let indexed_index = scoped
        .get_file("index.html")
        .expect("index.html must exist");
    assert_eq!(indexed_index.logical_path, "index.html");
    assert_eq!(
        indexed_index.real_path(&scoped.root_path),
        index_file.canonicalize().unwrap_or(index_file),
        "real_path must resolve to physical disk path"
    );
}

#[test]
fn test_scoped_index_multilevel_unicode_rerooting() {
    let dir = create_test_game_dir("unicode_reroot");
    let unicode_nested = dir.join("ゲーム").join("Data_MZ").join("js");
    fs::create_dir_all(&unicode_nested).unwrap();

    let script_file = unicode_nested.join("rmmz_core.js");
    fs::write(&script_file, "console.log('mz');").unwrap();

    let index = GameSourceIndex::build(&dir).unwrap();
    let scoped = index.scoped_to("ゲーム/Data_MZ").unwrap();

    assert_eq!(scoped.content_root, Some("ゲーム/Data_MZ".to_string()));
    assert!(scoped.has_file("js/rmmz_core.js"));
    let file = scoped.get_file("js/rmmz_core.js").unwrap();
    assert_eq!(file.logical_path, "js/rmmz_core.js");
    assert_eq!(
        file.real_path(&scoped.root_path),
        script_file.canonicalize().unwrap_or(script_file)
    );
}

#[test]
fn test_scoped_index_case_collision_fails_closed() {
    let dir = create_test_game_dir("case_collision");
    let target_dir = dir.join("sub");
    fs::create_dir_all(&target_dir).unwrap();

    let file_a = target_dir.join("file.txt");
    fs::write(&file_a, "alpha").unwrap();

    let mut index = GameSourceIndex::build(&dir).unwrap();

    // Manually inject a second file with case collision in the subpath
    let conflicting_file =
        IndexedFile::new("sub/FILE.TXT".to_string(), PathBuf::from("sub/FILE.TXT"), 5);
    index.files.insert(
        "sub/file.txt_conflict_collision".to_string(),
        conflicting_file,
    );

    let res = index.scoped_to("sub");
    assert!(res.is_err(), "Expected case collision failure, but got Ok");
    let err = res.unwrap_err();
    assert_eq!(
        err.code(),
        Some(DiagnosticCode::CMP_AUD_CASE_COLLISION),
        "Expected CMP-AUD-005, got '{:?}'",
        err.code()
    );
}

#[test]
fn test_content_root_serde_normalization() {
    use runtimepack_core::build_plan::EngineManifestEntry;
    use runtimepack_core::detection::{DetectionCandidate, EngineDescriptor};
    use runtimepack_core::domain::{DetectionConfidence, EngineId};

    // 1. None should be omitted in serialization
    let candidate = DetectionCandidate {
        engine: EngineId::RpgMakerMz,
        confidence: DetectionConfidence::Exact,
        score: 210,
        evidence: vec![],
        conflicts: vec![],
        content_root: None,
    };
    let json_cand = serde_json::to_string(&candidate).unwrap();
    assert!(
        !json_cand.contains("content_root"),
        "None should be omitted from JSON"
    );

    // 2. Legacy JSON without content_root deserializes as None
    let json_legacy = r#"{"engine":"rpg-maker-mz","confidence":"EXACT","score":210,"evidence":[],"conflicts":[]}"#;
    let decoded: DetectionCandidate = serde_json::from_str(json_legacy).unwrap();
    assert_eq!(decoded.content_root, None);

    // 3. Normalization of empty string / whitespace / dot to None
    let json_empty = r#"{"engine":"rpg-maker-mz","confidence":"EXACT","score":210,"evidence":[],"conflicts":[],"content_root":""}"#;
    let decoded_empty: DetectionCandidate = serde_json::from_str(json_empty).unwrap();
    assert_eq!(decoded_empty.content_root, None);

    let json_dot = r#"{"engine":"rpg-maker-mz","confidence":"EXACT","score":210,"evidence":[],"conflicts":[],"content_root":"."}"#;
    let decoded_dot: DetectionCandidate = serde_json::from_str(json_dot).unwrap();
    assert_eq!(decoded_dot.content_root, None);

    // 4. Valid nested path preserves canonical form
    let json_valid = r#"{"engine":"rpg-maker-mz","confidence":"EXACT","score":210,"evidence":[],"conflicts":[],"content_root":"bin/www"}"#;
    let decoded_valid: DetectionCandidate = serde_json::from_str(json_valid).unwrap();
    assert_eq!(decoded_valid.content_root, Some("bin/www".to_string()));

    // 5. Invalid external content_roots MUST NOT be sanitized/normalized; must be rejected with error!
    let invalid_json_samples = [
        r#"{"engine":"rpg-maker-mz","confidence":"EXACT","score":210,"evidence":[],"conflicts":[],"content_root":"/etc"}"#,
        r#"{"engine":"rpg-maker-mz","confidence":"EXACT","score":210,"evidence":[],"conflicts":[],"content_root":"C:\\Windows"}"#,
        r#"{"engine":"rpg-maker-mz","confidence":"EXACT","score":210,"evidence":[],"conflicts":[],"content_root":"../escape"}"#,
        r#"{"engine":"rpg-maker-mz","confidence":"EXACT","score":210,"evidence":[],"conflicts":[],"content_root":"\\\\server\\share"}"#,
        r#"{"engine":"rpg-maker-mz","confidence":"EXACT","score":210,"evidence":[],"conflicts":[],"content_root":"dir\u0000name"}"#,
        r#"{"engine":"rpg-maker-mz","confidence":"EXACT","score":210,"evidence":[],"conflicts":[],"content_root":"http://evil.com"}"#,
    ];

    for invalid_json in &invalid_json_samples {
        let res: Result<DetectionCandidate, _> = serde_json::from_str(invalid_json);
        assert!(
            res.is_err(),
            "Expected deserialization rejection for invalid content_root in '{}', but got Ok",
            invalid_json
        );
    }

    // 6. EngineDescriptor normalization & rejection
    let desc_json = r#"{"id":"rpg-maker-mz","confidence":"EXACT","score":210,"evidence":[],"conflicts":[],"overridden":false,"content_root":""}"#;
    let desc: EngineDescriptor = serde_json::from_str(desc_json).unwrap();
    assert_eq!(desc.content_root, None);

    let desc_invalid = r#"{"id":"rpg-maker-mz","confidence":"EXACT","score":210,"evidence":[],"conflicts":[],"overridden":false,"content_root":"/etc"}"#;
    let desc_res: Result<EngineDescriptor, _> = serde_json::from_str(desc_invalid);
    assert!(
        desc_res.is_err(),
        "Expected rejection of /etc in EngineDescriptor"
    );

    // 7. EngineManifestEntry normalization & rejection
    let entry_valid_json = r#"{"id":"rpg-maker-mz","confidence":"EXACT","overridden":false,"detected_version":null,"content_root":"bin/www"}"#;
    let entry_valid: EngineManifestEntry = serde_json::from_str(entry_valid_json).unwrap();
    assert_eq!(entry_valid.content_root, Some("bin/www".to_string()));

    let entry_invalid_json = r#"{"id":"rpg-maker-mz","confidence":"EXACT","overridden":false,"detected_version":null,"content_root":"/etc"}"#;
    let entry_res: Result<EngineManifestEntry, _> = serde_json::from_str(entry_invalid_json);
    assert!(
        entry_res.is_err(),
        "Expected rejection of /etc in EngineManifestEntry"
    );
}

#[test]
fn test_detector_trait_probe_all_default() {
    use runtimepack_core::detection::{DetectionCandidate, Detector};
    use runtimepack_core::domain::EngineId;

    struct DummyDetector;
    impl Detector for DummyDetector {
        fn id(&self) -> &'static str {
            "DET-DUMMY"
        }
        fn probe(&self, _source: &GameSourceIndex) -> DetectionCandidate {
            DetectionCandidate::none(EngineId::RenPy)
        }
    }

    let dir = create_test_game_dir("probe_all_dummy");
    fs::write(dir.join("game.txt"), "data").unwrap();
    let index = GameSourceIndex::build(&dir).unwrap();

    let detector = DummyDetector;
    let candidates = detector.probe_all(&index);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].engine, EngineId::RenPy);
}

#[test]
fn test_scoped_to_empty_or_dot_normalizes_to_none_root() {
    let dir = create_test_game_dir("empty_root");
    fs::write(dir.join("index.html"), "<html></html>").unwrap();
    let index = GameSourceIndex::build(&dir).unwrap();

    let scoped_empty = index.scoped_to("").unwrap();
    assert_eq!(scoped_empty.content_root, None);
    assert_eq!(scoped_empty.file_count(), 1);
    assert_eq!(scoped_empty.source_hash, index.source_hash);
    assert_eq!(
        scoped_empty.scoped_content_hash(),
        index.source_hash.as_str()
    );

    let scoped_dot = index.scoped_to(".").unwrap();
    assert_eq!(scoped_dot.content_root, None);
    assert_eq!(scoped_dot.file_count(), 1);
}
