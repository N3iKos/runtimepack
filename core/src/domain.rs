use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineId {
    #[serde(rename = "rpgmaker-xp", alias = "rpg-maker-xp")]
    RpgMakerXp,
    #[serde(rename = "rpgmaker-vx", alias = "rpg-maker-vx")]
    RpgMakerVx,
    #[serde(rename = "rpgmaker-vxace", alias = "rpg-maker-vx-ace", alias = "rpg-maker-vxace")]
    RpgMakerVxAce,
    #[serde(rename = "rpgmaker-mv", alias = "rpg-maker-mv")]
    RpgMakerMv,
    #[serde(rename = "rpgmaker-mz", alias = "rpg-maker-mz")]
    RpgMakerMz,
    #[serde(rename = "renpy", alias = "ren-py")]
    RenPy,
    #[serde(rename = "tyrano")]
    Tyrano,
    #[serde(rename = "visual-novel-maker")]
    VisualNovelMaker,
    #[serde(rename = "unknown")]
    Unknown,
}

impl fmt::Display for EngineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineId::RpgMakerXp => write!(f, "RPG Maker XP"),
            EngineId::RpgMakerVx => write!(f, "RPG Maker VX"),
            EngineId::RpgMakerVxAce => write!(f, "RPG Maker VX Ace"),
            EngineId::RpgMakerMv => write!(f, "RPG Maker MV"),
            EngineId::RpgMakerMz => write!(f, "RPG Maker MZ"),
            EngineId::RenPy => write!(f, "Ren'Py"),
            EngineId::Tyrano => write!(f, "TyranoBuilder / TyranoScript"),
            EngineId::VisualNovelMaker => write!(f, "Visual Novel Maker"),
            EngineId::Unknown => write!(f, "Unknown"),
        }
    }
}

impl std::str::FromStr for EngineId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "rpgmaker-xp" | "rpg-maker-xp" | "xp" => Ok(EngineId::RpgMakerXp),
            "rpgmaker-vx" | "rpg-maker-vx" | "vx" => Ok(EngineId::RpgMakerVx),
            "rpgmaker-vxace" | "rpgmaker-vx-ace" | "rpg-maker-vxace" | "rpg-maker-vx-ace" | "vxace" => {
                Ok(EngineId::RpgMakerVxAce)
            }
            "rpgmaker-mv" | "rpg-maker-mv" | "mv" => Ok(EngineId::RpgMakerMv),
            "rpgmaker-mz" | "rpg-maker-mz" | "mz" => Ok(EngineId::RpgMakerMz),
            "renpy" | "ren-py" => Ok(EngineId::RenPy),
            "tyrano" | "tyranobuilder" | "tyranoscript" => Ok(EngineId::Tyrano),
            "visual-novel-maker" | "visualnovelmaker" | "vnm" | "vnmaker" => {
                Ok(EngineId::VisualNovelMaker)
            }
            "unknown" => Ok(EngineId::Unknown),
            other => Err(format!("Unknown engine ID: '{}'", other)),
        }
    }
}

impl EngineId {
    pub fn default_runtime_family(&self) -> Option<RuntimeFamily> {
        match self {
            EngineId::RpgMakerXp | EngineId::RpgMakerVx | EngineId::RpgMakerVxAce => {
                Some(RuntimeFamily::Rgss)
            }
            EngineId::RpgMakerMv | EngineId::RpgMakerMz => Some(RuntimeFamily::WebRpgMaker),
            EngineId::RenPy => Some(RuntimeFamily::RenPy),
            EngineId::Tyrano => Some(RuntimeFamily::TyranoWeb),
            EngineId::VisualNovelMaker => Some(RuntimeFamily::VnMakerWeb),
            EngineId::Unknown => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuntimeFamily {
    #[serde(rename = "rgss")]
    Rgss,
    #[serde(rename = "web", alias = "web-rpg-maker")]
    WebRpgMaker,
    #[serde(rename = "renpy", alias = "ren-py")]
    RenPy,
    #[serde(rename = "tyrano", alias = "tyrano-web")]
    TyranoWeb,
    #[serde(rename = "vnmaker-web")]
    VnMakerWeb,
}

impl fmt::Display for RuntimeFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeFamily::Rgss => write!(f, "RGSS"),
            RuntimeFamily::WebRpgMaker => write!(f, "RPG Maker Web"),
            RuntimeFamily::RenPy => write!(f, "Ren'Py"),
            RuntimeFamily::TyranoWeb => write!(f, "Tyrano Web"),
            RuntimeFamily::VnMakerWeb => write!(f, "VNM Web"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DetectionConfidence {
    None,
    Low,
    Medium,
    High,
    Exact,
    Ambiguous,
}

impl fmt::Display for DetectionConfidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DetectionConfidence::None => write!(f, "NONE"),
            DetectionConfidence::Low => write!(f, "LOW"),
            DetectionConfidence::Medium => write!(f, "MEDIUM"),
            DetectionConfidence::High => write!(f, "HIGH"),
            DetectionConfidence::Exact => write!(f, "EXACT"),
            DetectionConfidence::Ambiguous => write!(f, "AMBIGUOUS"),
        }
    }
}

impl DetectionConfidence {
    pub fn allows_auto_build(&self) -> bool {
        matches!(self, DetectionConfidence::Exact | DetectionConfidence::High)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompatibilityStatus {
    Compatible,
    CompatibleWithShims,
    Degraded,
    UnsupportedDependency,
    Unknown,
    Blocked,
}

impl fmt::Display for CompatibilityStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompatibilityStatus::Compatible => write!(f, "COMPATIBLE"),
            CompatibilityStatus::CompatibleWithShims => write!(f, "COMPATIBLE_WITH_SHIMS"),
            CompatibilityStatus::Degraded => write!(f, "DEGRADED"),
            CompatibilityStatus::UnsupportedDependency => write!(f, "UNSUPPORTED_DEPENDENCY"),
            CompatibilityStatus::Unknown => write!(f, "UNKNOWN"),
            CompatibilityStatus::Blocked => write!(f, "BLOCKED"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ContentMode {
    Plain,
    #[default]
    Packed,
    EncryptedStreaming,
}

impl fmt::Display for ContentMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentMode::Plain => write!(f, "plain"),
            ContentMode::Packed => write!(f, "packed"),
            ContentMode::EncryptedStreaming => write!(f, "encrypted-streaming"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SigningMode {
    #[default]
    Debug,
    ReleaseLocal,
    ReleaseExternalKeystore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CompatibilityPolicy {
    #[default]
    Strict,
    Warn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Abi {
    #[serde(rename = "arm64-v8a")]
    Arm64V8a,
    #[serde(rename = "x86_64")]
    X86_64,
    #[serde(rename = "armeabi-v7a")]
    ArmeabiV7a,
}

impl fmt::Display for Abi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Abi::Arm64V8a => write!(f, "arm64-v8a"),
            Abi::X86_64 => write!(f, "x86_64"),
            Abi::ArmeabiV7a => write!(f, "armeabi-v7a"),
        }
    }
}

#[cfg(test)]
mod canonical_wire_tests {
    use super::{EngineId, RuntimeFamily};

    #[test]
    fn engine_id_serializes_canonical_spelling() {
        let cases = [
            (EngineId::RpgMakerXp, "\"rpgmaker-xp\""),
            (EngineId::RpgMakerVx, "\"rpgmaker-vx\""),
            (EngineId::RpgMakerVxAce, "\"rpgmaker-vxace\""),
            (EngineId::RpgMakerMv, "\"rpgmaker-mv\""),
            (EngineId::RpgMakerMz, "\"rpgmaker-mz\""),
            (EngineId::RenPy, "\"renpy\""),
            (EngineId::Tyrano, "\"tyrano\""),
            (EngineId::VisualNovelMaker, "\"visual-novel-maker\""),
            (EngineId::Unknown, "\"unknown\""),
        ];
        for (value, expected) in cases {
            assert_eq!(serde_json::to_string(&value).unwrap(), expected);
        }
    }

    #[test]
    fn engine_id_accepts_legacy_kebab_alias() {
        let cases = [
            ("\"rpg-maker-xp\"", EngineId::RpgMakerXp),
            ("\"rpg-maker-vx\"", EngineId::RpgMakerVx),
            ("\"rpg-maker-vx-ace\"", EngineId::RpgMakerVxAce),
            ("\"rpg-maker-mv\"", EngineId::RpgMakerMv),
            ("\"rpg-maker-mz\"", EngineId::RpgMakerMz),
            ("\"ren-py\"", EngineId::RenPy),
        ];
        for (wire, expected) in cases {
            let parsed: EngineId = serde_json::from_str(wire).unwrap();
            assert_eq!(parsed, expected, "failed to read alias {wire}");
        }
    }

    #[test]
    fn runtime_family_serializes_canonical_spelling() {
        let cases = [
            (RuntimeFamily::Rgss, "\"rgss\""),
            (RuntimeFamily::WebRpgMaker, "\"web\""),
            (RuntimeFamily::RenPy, "\"renpy\""),
            (RuntimeFamily::TyranoWeb, "\"tyrano\""),
            (RuntimeFamily::VnMakerWeb, "\"vnmaker-web\""),
        ];
        for (value, expected) in cases {
            assert_eq!(serde_json::to_string(&value).unwrap(), expected);
        }
    }

    #[test]
    fn runtime_family_accepts_legacy_kebab_alias() {
        let cases = [
            ("\"web-rpg-maker\"", RuntimeFamily::WebRpgMaker),
            ("\"ren-py\"", RuntimeFamily::RenPy),
            ("\"tyrano-web\"", RuntimeFamily::TyranoWeb),
        ];
        for (wire, expected) in cases {
            let parsed: RuntimeFamily = serde_json::from_str(wire).unwrap();
            assert_eq!(parsed, expected, "failed to read alias {wire}");
        }
    }
    #[test]
    fn engine_id_from_str_still_rejects_unknown_values() {
        use std::str::FromStr;
        assert!(EngineId::from_str("not-an-engine").is_err());
        assert_eq!(EngineId::from_str("renpy").unwrap(), EngineId::RenPy);
        assert_eq!(EngineId::from_str("ren-py").unwrap(), EngineId::RenPy);
    }
}
