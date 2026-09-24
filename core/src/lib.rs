pub mod asset_audit;
pub mod build_plan;
pub mod compatibility;
pub mod detection;
pub mod diagnostics;
pub mod domain;
pub mod ingest;
pub mod provenance;
pub mod runtime_registry;

pub use domain::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::DiagnosticCode;
    use crate::ingest::GameSourceIndex;
    use crate::provenance::ToolchainLock;
    use crate::runtime_registry::RuntimeRegistry;
    use std::path::Path;

    #[test]
    fn test_nonexistent_directory_error() {
        let res = GameSourceIndex::build(Path::new("nonexistent_directory_12345"));
        assert!(res.is_err());
        if let Err(crate::diagnostics::DiagnosticError::Detailed { code, .. }) = res {
            assert_eq!(code.0, DiagnosticCode::ING_PATH_NOT_FOUND);
        } else {
            panic!("Expected DiagnosticError::Detailed");
        }
    }

    #[test]
    fn test_runtime_registry_builtins() {
        let reg = RuntimeRegistry::new();
        assert!(reg.get("rgss-android").is_some());
        assert!(reg.get("web-rpgmaker").is_some());
        assert!(reg.get("renpy-android-7").is_some());
        assert!(reg.get("renpy-android-8").is_some());
        assert!(reg.get("tyrano-web").is_some());

        let vxace_runtimes = reg.find_for_engine(EngineId::RpgMakerVxAce);
        assert_eq!(vxace_runtimes.len(), 1);
        assert_eq!(vxace_runtimes[0].id, "rgss-android");
    }

    #[test]
    fn test_toolchain_lock_defaults() {
        let lock = ToolchainLock::default();
        assert_eq!(lock.gradle_version, "8.7");
        assert_eq!(lock.agp_version, "8.3.0");
        assert_eq!(lock.android_compile_sdk, 34);
        assert_eq!(lock.android_min_sdk, 26);
    }
}
