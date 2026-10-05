use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EngineId {
    RpgMakerXp,
    RpgMakerVx,
    RpgMakerVxAce,
    RpgMakerMv,
    RpgMakerMz,
    RenPy,
    Tyrano,
    VisualNovelMaker,
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
#[serde(rename_all = "kebab-case")]
pub enum RuntimeFamily {
    Rgss,
    WebRpgMaker,
    RenPy,
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
