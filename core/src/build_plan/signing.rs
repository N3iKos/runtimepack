use crate::build_plan::SigningConfig;
use crate::diagnostics::{DiagnosticCode, DiagnosticError};
use crate::domain::SigningMode;
use std::path::Path;

pub struct SigningManager;

impl SigningManager {
    pub fn validate_signing_config(config: &SigningConfig) -> Result<(), DiagnosticError> {
        match config.mode {
            SigningMode::Debug => {
                // Debug signing uses standard debug keystore
                Ok(())
            }
            SigningMode::ReleaseLocal => {
                // Local release mode requires valid keystore path if provided
                if let Some(ref path_str) = config.keystore_path {
                    let path = Path::new(path_str);
                    if !path.exists() {
                        return Err(DiagnosticError::from_code(
                            DiagnosticCode::SIG_KEYSTORE_NOT_FOUND,
                            format!("Release keystore file not found: {}", path.display()),
                        ));
                    }
                }
                Ok(())
            }
            SigningMode::ReleaseExternalKeystore => {
                let path_str = config.keystore_path.as_deref().ok_or_else(|| {
                    DiagnosticError::from_code(
                        DiagnosticCode::SIG_KEYSTORE_NOT_FOUND,
                        "Keystore path is required for release-external-keystore mode",
                    )
                })?;

                let path = Path::new(path_str);
                if !path.exists() {
                    return Err(DiagnosticError::from_code(
                        DiagnosticCode::SIG_KEYSTORE_NOT_FOUND,
                        format!("External release keystore not found: {}", path.display()),
                    ));
                }

                if config.key_alias.is_none() {
                    return Err(DiagnosticError::from_code(
                        "SIG-102",
                        "Key alias is required for release-external-keystore mode",
                    ));
                }

                Ok(())
            }
        }
    }

    pub fn generate_gradle_signing_block(config: &SigningConfig) -> String {
        match config.mode {
            SigningMode::Debug => r#"    signingConfigs {
        getByName("debug") {
            // Standard Android debug key
        }
    }
"#
            .to_string(),
            SigningMode::ReleaseLocal | SigningMode::ReleaseExternalKeystore => {
                let ks_path = config
                    .keystore_path
                    .as_deref()
                    .unwrap_or("release.keystore")
                    .replace('\\', "/");
                let alias = config.key_alias.as_deref().unwrap_or("key0");
                format!(
                    r#"    signingConfigs {{
        create("release") {{
            storeFile = file("{}")
            storePassword = System.getenv("KEYSTORE_PASSWORD") ?: "android"
            keyAlias = "{}"
            keyPassword = System.getenv("KEY_PASSWORD") ?: "android"
        }}
    }}
"#,
                    ks_path, alias
                )
            }
        }
    }
}
