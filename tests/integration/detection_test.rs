use runtimepack_core::detection::DetectorRegistry;
use runtimepack_core::domain::{DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_engine_renpy::RenPyDetector;
use runtimepack_engine_rgss::{RpgMakerVxAceDetector, RpgMakerVxDetector, RpgMakerXpDetector};
use runtimepack_engine_rpgmaker_web::{RpgMakerMvDetector, RpgMakerMzDetector};
use runtimepack_engine_tyrano::TyranoDetector;
use std::path::{Path, PathBuf};

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
