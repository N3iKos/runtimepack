//! Single source of truth for the v1 detector set.
//!
//! Both `runtimepack build` (`main.rs`) and `runtimepack audit`
//! (`audit_command.rs`) must use [`default_detector_registry`] so the two
//! commands can never disagree about which engines are detectable.
//!
//! This lives in the CLI crate, not in `runtimepack_core`, because the detector
//! implementations live in the `runtimepack-engine-*` crates, and every one of
//! those depends on `runtimepack_core`. Putting the constructors in `core`
//! would create a dependency cycle.

use runtimepack_core::detection::DetectorRegistry;
use runtimepack_engine_renpy::RenPyDetector;
use runtimepack_engine_rgss::{RpgMakerVxAceDetector, RpgMakerVxDetector, RpgMakerXpDetector};
use runtimepack_engine_rpgmaker_web::{RpgMakerMvDetector, RpgMakerMzDetector};
use runtimepack_engine_tyrano::TyranoDetector;
use runtimepack_engine_vnmaker::VnMakerDetector;

/// The complete v1 detector set, in deterministic registration order.
///
/// Order is load-bearing: arbitration and conflict reporting see candidates in
/// this sequence. Keep it identical to the historical CLI order — XP, VX,
/// VX Ace, MV, MZ, Ren'Py, Tyrano, VN Maker.
pub fn default_detector_registry() -> DetectorRegistry {
    let mut registry = DetectorRegistry::new();
    registry.register(Box::new(RpgMakerXpDetector));
    registry.register(Box::new(RpgMakerVxDetector));
    registry.register(Box::new(RpgMakerVxAceDetector));
    registry.register(Box::new(RpgMakerMvDetector));
    registry.register(Box::new(RpgMakerMzDetector));
    registry.register(Box::new(RenPyDetector));
    registry.register(Box::new(TyranoDetector));
    registry.register(Box::new(VnMakerDetector));
    registry
}

#[cfg(test)]
mod default_registry_tests {
    use super::default_detector_registry;

    #[test]
    fn registry_contains_every_supported_engine_once() {
        let registry = default_detector_registry();
        assert_eq!(
            registry.len(),
            8,
            "expected exactly the eight v1 detectors"
        );
    }

    #[test]
    fn registry_is_deterministic_across_calls() {
        let a: Vec<String> = default_detector_registry()
            .names()
            .into_iter()
            .map(str::to_string)
            .collect();
        let b: Vec<String> = default_detector_registry()
            .names()
            .into_iter()
            .map(str::to_string)
            .collect();
        assert_eq!(a, b, "registration order must be stable");
    }

    #[test]
    fn registry_registration_order_is_pinned() {
        let names: Vec<String> = default_detector_registry()
            .names()
            .into_iter()
            .map(str::to_string)
            .collect();
        assert_eq!(
            names,
            vec![
                "DET-RGSS-XP",
                "DET-RGSS-VX",
                "DET-RGSS-VXA",
                "DET-WEB-MV",
                "DET-WEB-MZ",
                "DET-RNP",
                "DET-TYR",
                "DET-VNM",
            ],
            "aggregation semantics depend on registration order"
        );
    }
}
