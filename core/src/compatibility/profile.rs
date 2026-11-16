use crate::diagnostics::{DiagnosticCode, DiagnosticError};
use crate::domain::{CompatibilityStatus, EngineId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetAnchor {
    Payload,
    Source,
}

impl Default for TargetAnchor {
    fn default() -> Self {
        Self::Payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityPatch {
    #[serde(default)]
    pub target_anchor: TargetAnchor,
    pub logical_path: String,
    pub expected_file_size: u64,
    pub expected_file_sha256: String,
    pub range: ByteRange,
    pub expected_fragment_sha256: String,
    pub replacement: String,
    pub result_file_sha256: String,
}

impl CompatibilityPatch {
    pub fn resolved_path(&self, content_root: Option<&str>) -> String {
        match self.target_anchor {
            TargetAnchor::Payload => {
                if let Some(root) = content_root {
                    let trimmed = root.trim_matches('/');
                    if trimmed.is_empty() {
                        self.logical_path.clone()
                    } else {
                        format!("{}/{}", trimmed, self.logical_path.trim_start_matches('/'))
                    }
                } else {
                    self.logical_path.clone()
                }
            }
            TargetAnchor::Source => self.logical_path.clone(),
        }
    }

    pub fn validate(&self) -> Result<(), DiagnosticError> {
        let p = &self.logical_path;
        if p.is_empty()
            || p.starts_with('/')
            || p.starts_with('\\')
            || p.contains("..")
            || (p.len() >= 2 && p.as_bytes()[1] == b':')
        {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CNT_TRAVERSAL_REJECTED,
                format!("Invalid patch logical_path '{}'", p),
            ));
        }

        if self.range.start > self.range.end {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CMP_PROFILE_INVALID,
                format!(
                    "Invalid byte range {}..{} in patch for '{}'",
                    self.range.start, self.range.end, p
                ),
            ));
        }

        if self.range.end > self.expected_file_size as usize {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CMP_PROFILE_INVALID,
                format!(
                    "Byte range end {} exceeds expected file size {} in patch for '{}'",
                    self.range.end, self.expected_file_size, p
                ),
            ));
        }

        let is_valid_sha = |s: &str| s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit());
        if !is_valid_sha(&self.expected_file_sha256)
            || !is_valid_sha(&self.expected_fragment_sha256)
            || !is_valid_sha(&self.result_file_sha256)
        {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CMP_PROFILE_INVALID,
                format!("Invalid SHA-256 hash format in patch for '{}'", p),
            ));
        }

        Ok(())
    }

    pub fn apply_to_bytes(&self, original_bytes: &[u8]) -> Result<Vec<u8>, DiagnosticError> {
        if original_bytes.len() as u64 != self.expected_file_size {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CMP_PROFILE_INVALID,
                format!(
                    "Pre-patch file size mismatch for '{}': expected {} bytes, found {} bytes",
                    self.logical_path,
                    self.expected_file_size,
                    original_bytes.len()
                ),
            ));
        }

        let pre_sha = format!("{:x}", Sha256::digest(original_bytes));
        if !pre_sha.eq_ignore_ascii_case(&self.expected_file_sha256) {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CMP_PROFILE_INVALID,
                format!(
                    "Pre-patch file SHA-256 mismatch for '{}': expected {}, found {}",
                    self.logical_path, self.expected_file_sha256, pre_sha
                ),
            ));
        }

        let fragment = &original_bytes[self.range.start..self.range.end];
        let frag_sha = format!("{:x}", Sha256::digest(fragment));
        if !frag_sha.eq_ignore_ascii_case(&self.expected_fragment_sha256) {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CMP_PROFILE_INVALID,
                format!(
                    "Pre-patch fragment SHA-256 mismatch for '{}': expected {}, found {}",
                    self.logical_path, self.expected_fragment_sha256, frag_sha
                ),
            ));
        }

        // Apply replacement bytes
        let rep_bytes = self.replacement.as_bytes();
        let mut result = Vec::with_capacity(
            original_bytes.len() - (self.range.end - self.range.start) + rep_bytes.len(),
        );
        result.extend_from_slice(&original_bytes[..self.range.start]);
        result.extend_from_slice(rep_bytes);
        result.extend_from_slice(&original_bytes[self.range.end..]);

        let post_sha = format!("{:x}", Sha256::digest(&result));
        if !post_sha.eq_ignore_ascii_case(&self.result_file_sha256) {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CMP_PROFILE_INVALID,
                format!(
                    "Post-patch result SHA-256 mismatch for '{}': expected {}, found {}",
                    self.logical_path, self.result_file_sha256, post_sha
                ),
            ));
        }

        Ok(result)
    }
}

fn deserialize_engine_id<'de, D>(deserializer: D) -> Result<Option<EngineId>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        Some(s) => s.parse::<EngineId>().map(Some).map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityProfile {
    pub schema_version: u32,
    pub profile_id: String,
    #[serde(default, deserialize_with = "deserialize_engine_id", skip_serializing_if = "Option::is_none")]
    pub engine_id: Option<EngineId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    pub status: CompatibilityStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub patches: Vec<CompatibilityPatch>,
}

impl Default for CompatibilityProfile {
    fn default() -> Self {
        Self {
            schema_version: 1,
            profile_id: String::new(),
            engine_id: None,
            capability_id: None,
            capabilities: Vec::new(),
            status: CompatibilityStatus::Compatible,
            diagnostic_code: None,
            description: None,
            patches: Vec::new(),
        }
    }
}

impl CompatibilityProfile {
    pub fn collected_capabilities(&self) -> Vec<String> {
        let mut caps = Vec::new();
        if let Some(ref cap) = self.capability_id {
            let trimmed = cap.trim().to_string();
            if !trimmed.is_empty() && !caps.contains(&trimmed) {
                caps.push(trimmed);
            }
        }
        for cap in &self.capabilities {
            let trimmed = cap.trim().to_string();
            if !trimmed.is_empty() && !caps.contains(&trimmed) {
                caps.push(trimmed);
            }
        }
        caps.sort();
        caps.dedup();
        caps
    }
    pub fn parse_from_json(json_str: &str) -> Result<Self, DiagnosticError> {
        let profile: Self = serde_json::from_str(json_str).map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::CMP_PROFILE_INVALID,
                format!("Failed to parse compatibility profile JSON: {}", e),
            )
        })?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn load_from_file(path: &Path) -> Result<Self, DiagnosticError> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::CMP_PROFILE_INVALID,
                format!("Failed to read compatibility profile at '{}': {}", path.display(), e),
            )
        })?;
        Self::parse_from_json(&content)
    }

    pub fn validate(&self) -> Result<(), DiagnosticError> {
        if self.schema_version != 1 {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CMP_PROFILE_INVALID,
                format!("Unsupported compatibility profile schema_version: {}", self.schema_version),
            ));
        }

        if self.profile_id.trim().is_empty() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CMP_PROFILE_INVALID,
                "compatibility profile profile_id must not be empty",
            ));
        }

        if self.patches.is_empty() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CMP_PROFILE_INVALID,
                "compatibility profile must contain at least one patch",
            ));
        }

        // Validate each patch
        for patch in &self.patches {
            patch.validate()?;
        }

        // Reject duplicate patch targets within same anchor
        let mut seen_targets = std::collections::HashSet::new();
        for patch in &self.patches {
            let normalized = patch.logical_path.replace('\\', "/").to_ascii_lowercase();
            let key = (patch.target_anchor, normalized);
            if !seen_targets.insert(key) {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::CMP_PROFILE_INVALID,
                    format!(
                        "Duplicate patch target detected for '{:?}:{}'",
                        patch.target_anchor, patch.logical_path
                    ),
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_profile_parsing_and_validation() {
        let json = r#"{
            "schema_version": 1,
            "profile_id": "test-profile-1",
            "engine_id": "rpgmaker-mz",
            "capability_id": "CAP-WEB-028",
            "status": "DEGRADED",
            "diagnostic_code": "CMP-107",
            "description": "Test transformation profile",
            "patches": [
                {
                    "target_anchor": "payload",
                    "logical_path": "js/plugins/test.js",
                    "expected_file_size": 10,
                    "expected_file_sha256": "4b227777d4dd1fc61c6f884f48641d02b4d121d3fd328cb08b5531fcacdabf8a",
                    "range": { "start": 0, "end": 4 },
                    "expected_fragment_sha256": "3e23e8160039594a33894f6564e1b1348bbd7a0088d42c4acb73eeaed59c009d",
                    "replacement": "TEST",
                    "result_file_sha256": "9b6c16198f244fa1216d1ba32c44fc8ffdd78f9f090ce9cff2940ca58a74be88"
                }
            ]
        }"#;

        let profile = CompatibilityProfile::parse_from_json(json).unwrap();
        assert_eq!(profile.profile_id, "test-profile-1");
        assert_eq!(profile.patches.len(), 1);
        assert_eq!(profile.patches[0].resolved_path(Some("bin/www")), "bin/www/js/plugins/test.js");
        assert_eq!(profile.patches[0].resolved_path(None), "js/plugins/test.js");
        assert_eq!(profile.collected_capabilities(), vec!["CAP-WEB-028"]);
    }

    #[test]
    fn test_profile_collected_capabilities_dedup_and_union() {
        let json = r#"{
            "schema_version": 1,
            "profile_id": "neutral-profile",
            "capability_id": "CAP-WEB-030",
            "capabilities": ["CAP-WEB-028", "CAP-WEB-030"],
            "status": "DEGRADED",
            "patches": [
                {
                    "target_anchor": "payload",
                    "logical_path": "js/plugins/test.js",
                    "expected_file_size": 10,
                    "expected_file_sha256": "4b227777d4dd1fc61c6f884f48641d02b4d121d3fd328cb08b5531fcacdabf8a",
                    "range": { "start": 0, "end": 4 },
                    "expected_fragment_sha256": "3e23e8160039594a33894f6564e1b1348bbd7a0088d42c4acb73eeaed59c009d",
                    "replacement": "TEST",
                    "result_file_sha256": "9b6c16198f244fa1216d1ba32c44fc8ffdd78f9f090ce9cff2940ca58a74be88"
                }
            ]
        }"#;
        let profile = CompatibilityProfile::parse_from_json(json).unwrap();
        assert_eq!(profile.collected_capabilities(), vec!["CAP-WEB-028", "CAP-WEB-030"]);
    }

    #[test]
    fn test_profile_rejects_unknown_fields() {
        let json = r#"{
            "schema_version": 1,
            "profile_id": "test-profile",
            "status": "DEGRADED",
            "unknown_field": 123,
            "patches": []
        }"#;
        assert!(CompatibilityProfile::parse_from_json(json).is_err());
    }

    #[test]
    fn test_profile_rejects_path_traversal() {
        let json = r#"{
            "schema_version": 1,
            "profile_id": "test-traversal",
            "status": "DEGRADED",
            "patches": [
                {
                    "logical_path": "../evil.js",
                    "expected_file_size": 10,
                    "expected_file_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
                    "range": { "start": 0, "end": 2 },
                    "expected_fragment_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
                    "replacement": "safe",
                    "result_file_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
                }
            ]
        }"#;
        assert!(CompatibilityProfile::parse_from_json(json).is_err());
    }

    #[test]
    fn test_patch_apply_mismatch_fails_closed() {
        let patch = CompatibilityPatch {
            target_anchor: TargetAnchor::Payload,
            logical_path: "test.js".to_string(),
            expected_file_size: 5,
            expected_file_sha256: "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".to_string(), // "hello"
            range: ByteRange { start: 0, end: 5 },
            expected_fragment_sha256: "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".to_string(),
            replacement: "world".to_string(),
            result_file_sha256: "486ea46224d1bb4fb680f34f7c9ad96a8f24ec88be73ea8e5a6c65260e9cb8a7".to_string(), // "world"
        };

        // Size mismatch
        assert!(patch.apply_to_bytes(b"hello world").is_err());
        // Hash mismatch
        assert!(patch.apply_to_bytes(b"wrong").is_err());
        // Success
        let res = patch.apply_to_bytes(b"hello").unwrap();
        assert_eq!(res, b"world");
    }
}
