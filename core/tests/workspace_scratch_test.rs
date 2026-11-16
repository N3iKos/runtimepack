use runtimepack_core::build_plan::scratch::{CleanupStatus, WorkspaceScratch};
use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticError};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn create_test_output_dir(name: &str) -> PathBuf {
    let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "runtimepack_scratch_test_{}_{}_{}",
        name,
        std::process::id(),
        id
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_scratch_creation_generates_isolated_directory_under_output() {
    let output_dir = create_test_output_dir("creation");
    let scratch = WorkspaceScratch::create(&output_dir).expect("scratch should be created");

    assert!(scratch.scratch_root().exists());
    assert!(scratch.scratch_root().is_dir());
    let canonical_output_dir = output_dir
        .canonicalize()
        .expect("canonical output_dir must succeed");
    assert!(
        scratch.scratch_root().starts_with(&canonical_output_dir),
        "scratch_root ({}) must start with canonical output_dir ({})",
        scratch.scratch_root().display(),
        canonical_output_dir.display()
    );
    assert!(scratch.run_id().len() >= 15);

    let file_name = scratch
        .scratch_root()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        file_name.starts_with(".work_"),
        "scratch directory name must start with .work_"
    );

    let _ = fs::remove_dir_all(&output_dir);
}

#[test]
fn test_scratch_safe_deletion_unlinks_only_owned_entries() {
    let output_dir = create_test_output_dir("safe_delete");
    let outside_sentinel = output_dir.join("outside_deliverable.apk");
    fs::write(&outside_sentinel, b"precious_user_apk").unwrap();

    let mut scratch = WorkspaceScratch::create(&output_dir).unwrap();

    // Register owned entries
    let staging_dir = scratch.register_owned_dir("staging").unwrap();
    let pack_file = scratch.register_owned_file("game.pack").unwrap();
    let project_dir = scratch.register_owned_dir("android-project").unwrap();

    // Populate owned entries with subtrees and files
    fs::write(staging_dir.join("transformed.js"), b"console.log(1);").unwrap();
    fs::write(&pack_file, b"GPACK_CONTENT_BYTES_12345").unwrap();

    fs::create_dir_all(project_dir.join("app/build/outputs")).unwrap();
    fs::write(
        project_dir.join("app/build/outputs/app-debug.apk"),
        b"BUILD_APK",
    )
    .unwrap();

    // Execute safe cleanup
    let summary = scratch.safe_cleanup().expect("cleanup should succeed");

    assert_eq!(summary.status, CleanupStatus::Cleaned);
    assert!(
        summary.estimated_bytes_removed > 0,
        "should record bytes removed"
    );
    assert!(
        !scratch.scratch_root().exists(),
        "scratch root should be removed"
    );
    assert!(
        outside_sentinel.exists(),
        "outside deliverable must remain intact"
    );
    assert_eq!(
        fs::read(&outside_sentinel).unwrap(),
        b"precious_user_apk",
        "outside deliverable content must not be modified"
    );

    let _ = fs::remove_dir_all(&output_dir);
}

#[test]
fn test_scratch_safe_deletion_refuses_unexpected_junctions_and_reparse_points() {
    let output_dir = create_test_output_dir("reparse_refusal");
    let outside_secret_dir = output_dir.join("secret_user_dir");
    fs::create_dir_all(&outside_secret_dir).unwrap();
    let secret_file = outside_secret_dir.join("secret.txt");
    fs::write(&secret_file, b"TOP_SECRET").unwrap();

    let mut scratch = WorkspaceScratch::create(&output_dir).unwrap();
    let _ = scratch.register_owned_dir("staging").unwrap();

    // Inject an unexpected symlink pointing outside
    #[cfg(unix)]
    {
        let link_target = &outside_secret_dir;
        let symlink_path = scratch.scratch_root().join("unexpected_link");
        std::os::unix::fs::symlink(link_target, &symlink_path).unwrap();
    }

    #[cfg(windows)]
    {
        let symlink_path = scratch.scratch_root().join("unexpected_link");
        let _ = std::os::windows::fs::symlink_dir(&outside_secret_dir, &symlink_path);
    }

    let unexpected_link = scratch.scratch_root().join("unexpected_link");
    if unexpected_link.exists() || unexpected_link.is_symlink() {
        let result = scratch.safe_cleanup();
        assert!(
            result.is_err(),
            "should refuse cleanup when unexpected symlink/reparse point exists"
        );
        if let Err(DiagnosticError::Detailed { code, .. }) = result {
            assert_eq!(code.0, DiagnosticCode::BLD_CLEANUP_PARTIAL);
        }
        assert!(
            scratch.scratch_root().exists(),
            "scratch root must be preserved when cleanup is refused"
        );
        assert!(
            secret_file.exists(),
            "outside secret file must never be deleted"
        );
    }

    let _ = fs::remove_dir_all(&output_dir);
}

#[test]
fn test_scratch_safe_deletion_rejects_disjoint_violation() {
    let output_dir = create_test_output_dir("disjoint_test");
    let protected_source = output_dir.join("source_game");
    fs::create_dir_all(&protected_source).unwrap();

    let result = WorkspaceScratch::create_with_protected_roots(
        &protected_source,
        &[protected_source.clone()],
    );
    assert!(result.is_err(), "should reject output inside protected source");

    let _ = fs::remove_dir_all(&output_dir);
}

#[test]
fn test_scratch_registration_rejects_escaping_rel_name() {
    let output_dir = create_test_output_dir("escape_test");
    let mut scratch = WorkspaceScratch::create(&output_dir).unwrap();

    assert!(scratch.register_owned_dir("../escaped").is_err());
    assert!(scratch.register_owned_dir("/absolute/path").is_err());
    assert!(scratch.register_owned_file("nested/../../escaped").is_err());
    assert!(scratch.register_owned_file("").is_err());

    // Valid names succeed
    assert!(scratch.register_owned_dir("staging").is_ok());
    assert!(scratch.register_owned_file("game.pack").is_ok());

    let _ = fs::remove_dir_all(&output_dir);
}

#[test]
fn test_scratch_registration_rejects_foreign_preexisting_targets() {
    let output_dir = create_test_output_dir("preexisting_rejection");
    let mut scratch = WorkspaceScratch::create(&output_dir).unwrap();

    // Plant foreign directory before registration
    let foreign_dir = scratch.scratch_root().join("foreign_staging");
    fs::create_dir_all(&foreign_dir).unwrap();

    let res = scratch.register_owned_dir("foreign_staging");
    assert!(
        res.is_err(),
        "register_owned_dir must reject pre-existing foreign directory"
    );

    // Plant foreign file before registration
    let foreign_file = scratch.scratch_root().join("foreign_pack.pack");
    fs::write(&foreign_file, b"malicious_preexisting_payload").unwrap();

    let res = scratch.register_owned_file("foreign_pack.pack");
    assert!(
        res.is_err(),
        "register_owned_file must reject pre-existing foreign file"
    );

    // Plant dangling symlink before registration
    #[cfg(unix)]
    {
        let dangling_link = scratch.scratch_root().join("dangling_link");
        std::os::unix::fs::symlink("/nonexistent_target_12345", &dangling_link).unwrap();
        let res = scratch.register_owned_file("dangling_link");
        assert!(
            res.is_err(),
            "register_owned_file must reject pre-existing dangling symlink"
        );
    }

    let _ = fs::remove_dir_all(&output_dir);
}

#[test]
fn test_scratch_registration_idempotency_and_type_validation() {
    let output_dir = create_test_output_dir("idempotency_type");
    let mut scratch = WorkspaceScratch::create(&output_dir).unwrap();

    // 1. Initial registration creates directory
    let dir_path = scratch.register_owned_dir("staging").unwrap();
    assert!(dir_path.is_dir());

    // 2. Idempotent re-registration of exact same directory succeeds
    let dir_path2 = scratch.register_owned_dir("staging").unwrap();
    assert_eq!(dir_path, dir_path2);

    // 3. Registering directory as file fails with type mismatch
    assert!(
        scratch.register_owned_file("staging").is_err(),
        "register_owned_file must reject already registered directory"
    );

    // 4. Initial registration creates file
    let file_path = scratch.register_owned_file("game.pack").unwrap();
    assert!(file_path.is_file());

    // 5. Idempotent re-registration of exact same file succeeds
    let file_path2 = scratch.register_owned_file("game.pack").unwrap();
    assert_eq!(file_path, file_path2);

    // 6. Registering file as directory fails with type mismatch
    assert!(
        scratch.register_owned_dir("game.pack").is_err(),
        "register_owned_dir must reject already registered file"
    );

    let _ = fs::remove_dir_all(&output_dir);
}

#[test]
fn test_scratch_safe_deletion_refuses_when_root_identity_replaced() {
    let output_dir = create_test_output_dir("identity_replaced");
    let scratch = WorkspaceScratch::create(&output_dir).unwrap();

    let root_path = scratch.scratch_root().to_path_buf();
    assert!(root_path.exists());

    // Adversarial replacement: create another distinct directory first (guaranteeing different inode/identity),
    // then swap it into root_path.
    let replacement_dir = output_dir.join("replacement_dir");
    fs::create_dir(&replacement_dir).unwrap();
    fs::remove_dir(&root_path).unwrap();
    fs::rename(&replacement_dir, &root_path).unwrap();

    // Cleanup must detect the identity mismatch and refuse deletion
    let res = scratch.safe_cleanup();
    assert!(
        res.is_err(),
        "safe_cleanup must refuse deletion when scratch root identity has been replaced"
    );
    if let Err(DiagnosticError::Detailed { code, .. }) = res {
        assert_eq!(code.0, DiagnosticCode::BLD_CLEANUP_PARTIAL);
    }

    let _ = fs::remove_dir_all(&output_dir);
}

#[test]
fn test_scratch_refuses_unregistered_top_level_entries() {
    let output_dir = create_test_output_dir("unregistered_refusal");
    let mut scratch = WorkspaceScratch::create(&output_dir).unwrap();

    let _ = scratch.register_owned_dir("staging").unwrap();

    // Plant an unowned/unregistered top-level directory inside scratch_root
    fs::create_dir_all(scratch.scratch_root().join("foreign_directory")).unwrap();

    let result = scratch.safe_cleanup();
    assert!(
        result.is_err(),
        "cleanup must refuse deletion if an unregistered top-level directory is present"
    );
    if let Err(DiagnosticError::Detailed { code, .. }) = result {
        assert_eq!(code.0, DiagnosticCode::BLD_CLEANUP_PARTIAL);
    }
    assert!(
        scratch.scratch_root().exists(),
        "whole tree must remain intact on refusal"
    );

    let _ = fs::remove_dir_all(&output_dir);
}

#[test]
fn test_two_concurrent_workspaces_are_isolated() {
    let output_dir = create_test_output_dir("concurrent");
    let scratch1 = WorkspaceScratch::create(&output_dir).unwrap();
    let scratch2 = WorkspaceScratch::create(&output_dir).unwrap();

    assert_ne!(scratch1.run_id(), scratch2.run_id());
    assert_ne!(scratch1.scratch_root(), scratch2.scratch_root());
    assert!(scratch1.scratch_root().exists());
    assert!(scratch2.scratch_root().exists());

    let summary1 = scratch1.safe_cleanup().unwrap();
    assert_eq!(summary1.status, CleanupStatus::Cleaned);
    assert!(!scratch1.scratch_root().exists());
    assert!(
        scratch2.scratch_root().exists(),
        "scratch2 must remain untouched when scratch1 cleans up"
    );

    let summary2 = scratch2.safe_cleanup().unwrap();
    assert_eq!(summary2.status, CleanupStatus::Cleaned);
    assert!(!scratch2.scratch_root().exists());

    let _ = fs::remove_dir_all(&output_dir);
}

#[test]
fn test_scratch_safe_deletion_handles_empty_registered_entries() {
    let output_dir = create_test_output_dir("empty_registered");
    let mut scratch = WorkspaceScratch::create(&output_dir).unwrap();

    let _ = scratch.register_owned_dir("staging").unwrap();
    let _ = scratch.register_owned_file("game.pack").unwrap();

    let summary = scratch.safe_cleanup().expect("cleanup should succeed on empty entries");
    assert_eq!(summary.status, CleanupStatus::Cleaned);
    assert!(!scratch.scratch_root().exists());

    let _ = fs::remove_dir_all(&output_dir);
}

#[test]
fn test_scratch_journal_captures_in_memory_metadata() {
    let output_dir = create_test_output_dir("journal_meta");
    let mut scratch = WorkspaceScratch::create(&output_dir).unwrap();

    let _ = scratch.register_owned_dir("staging").unwrap();
    let _ = scratch.register_owned_file("game.pack").unwrap();

    let journal = scratch.journal();
    assert_eq!(journal.run_id, scratch.run_id());
    assert_eq!(&journal.scratch_root, scratch.scratch_root());
    assert_eq!(journal.creator_pid, std::process::id());
    assert_eq!(journal.owned_directories, vec!["staging".to_string()]);
    assert_eq!(journal.owned_files, vec!["game.pack".to_string()]);
    assert!(!journal.created_at.is_empty());

    let _ = scratch.safe_cleanup();
    let _ = fs::remove_dir_all(&output_dir);
}

#[test]
fn test_scratch_no_drop_cleanup_on_crash() {
    let output_dir = create_test_output_dir("no_drop_cleanup");
    let scratch_path;
    {
        let mut scratch = WorkspaceScratch::create(&output_dir).unwrap();
        scratch_path = scratch.scratch_root().to_path_buf();
        let _ = scratch.register_owned_dir("staging").unwrap();
        fs::write(scratch_path.join("staging/file.txt"), b"DATA").unwrap();
        // Scratch goes out of scope here WITHOUT safe_cleanup()
    }

    assert!(
        scratch_path.exists(),
        "scratch directory must NOT be deleted on Drop"
    );
    assert!(
        scratch_path.join("staging/file.txt").exists(),
        "staged files must NOT be deleted on Drop"
    );

    let _ = fs::remove_dir_all(&output_dir);
}

#[test]
fn test_scratch_safe_deletion_refuses_ntfs_directory_junction() {
    // Allocate under target directory if available to test real NTFS junctions on DrvFs/Windows
    let base_dir = PathBuf::from("target/test_junction_workspace");
    let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let output_dir = base_dir.join(format!("run_{}_{}", std::process::id(), id));
    let _ = fs::remove_dir_all(&output_dir);
    if fs::create_dir_all(&output_dir).is_err() {
        return; // Skip if target directory is not writable in this environment
    }

    let outside_secret = output_dir.join("outside_secret");
    fs::create_dir_all(&outside_secret).unwrap();
    let secret_file = outside_secret.join("secret.txt");
    fs::write(&secret_file, b"NTFS_SECRET_DATA").unwrap();

    let mut scratch = WorkspaceScratch::create(&output_dir).unwrap();
    let _ = scratch.register_owned_dir("staging").unwrap();

    let junction_path = scratch.scratch_root().join("ntfs_junction");
    let canon = output_dir.canonicalize().unwrap();
    let canon_str = canon.to_string_lossy();

    let created = if canon_str.starts_with("/mnt/") {
        let drive = canon_str.chars().nth(5).unwrap_or('d');
        let rest = &canon_str[6..].replace('/', "\\");
        let win_junction = format!("{}:{}\\{}\\ntfs_junction", drive, rest, format!(".work_{}", scratch.run_id()));
        let win_target = format!("{}:{}\\{}", drive, rest, "outside_secret");
        std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J", &win_junction, &win_target])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else if cfg!(windows) {
        std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&junction_path)
            .arg(&outside_secret)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        false
    };

    if created || junction_path.symlink_metadata().is_ok() {
        let res = scratch.safe_cleanup();
        assert!(
            res.is_err(),
            "safe_cleanup must refuse deletion when NTFS junction is present"
        );
        assert!(
            secret_file.exists(),
            "outside secret file must remain intact"
        );
    }

    // Cleanup junction link via cmd rmdir or fs::remove_dir
    let _ = fs::remove_dir(&junction_path);
    let _ = fs::remove_dir_all(&output_dir);
}
