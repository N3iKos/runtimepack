use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolchainLock {
    pub lock_version: u32,
    pub gradle_version: String,
    pub agp_version: String,
    pub jdk_min_version: String,
    pub android_compile_sdk: u32,
    pub android_min_sdk: u32,
    pub android_target_sdk: u32,
    pub build_tools_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ndk_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmake_version: Option<String>,
}

impl Default for ToolchainLock {
    fn default() -> Self {
        Self {
            lock_version: 1,
            gradle_version: "8.7".to_string(),
            agp_version: "8.3.0".to_string(),
            jdk_min_version: "17".to_string(),
            android_compile_sdk: 34,
            android_min_sdk: 26,
            android_target_sdk: 34,
            build_tools_version: "34.0.0".to_string(),
            ndk_version: Some("26.1.10909125".to_string()),
            cmake_version: Some("3.22.1".to_string()),
        }
    }
}

impl ToolchainLock {
    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json_str(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceClass {
    /// Class A: Official product/vendor documentation
    ClassA,
    /// Class B: Official/upstream open-source repository
    ClassB,
    /// Class C: JoiPlay public repository/documentation
    ClassC,
    /// Class D: Community/unofficial material
    ClassD,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub id: String,
    pub class: EvidenceClass,
    pub title: String,
    pub url: String,
    pub claims: Vec<String>,
}

pub struct EvidenceRegistry;

impl EvidenceRegistry {
    pub fn get_canonical_evidence() -> Vec<EvidenceItem> {
        vec![
            EvidenceItem {
                id: "S01".to_string(),
                class: EvidenceClass::ClassA,
                title: "JoiPlay official product page".to_string(),
                url: "https://www.joiplay.cyou/index.html".to_string(),
                claims: vec![
                    "JoiPlay is a game interpreter and launcher".to_string(),
                    "Supported families: XP, VX, VX Ace, MV, MZ, Ren'Py, Tyrano".to_string(),
                    "Does not emulate Windows".to_string(),
                ],
            },
            EvidenceItem {
                id: "S02".to_string(),
                class: EvidenceClass::ClassA,
                title: "JoiPlay RPGPacker".to_string(),
                url: "https://joiplay.cyou/rpgpacker/".to_string(),
                claims: vec![
                    "Build-time model for XP/VX/VX Ace to Android".to_string(),
                    "Generates Gradle project and builds APK/AAB".to_string(),
                ],
            },
            EvidenceItem {
                id: "S06".to_string(),
                class: EvidenceClass::ClassB,
                title: "mkxp-z open source player".to_string(),
                url: "https://github.com/mkxp-z/mkxp-z".to_string(),
                claims: vec![
                    "Open-source player for XP/VX/VX Ace".to_string(),
                    "GPL-2.0+ licensing with caveats".to_string(),
                ],
            },
            EvidenceItem {
                id: "S10".to_string(),
                class: EvidenceClass::ClassB,
                title: "TyranoScript official repository".to_string(),
                url: "https://github.com/ShikemokuMK/tyranoscript".to_string(),
                claims: vec![
                    "HTML/JavaScript visual novel engine".to_string(),
                    "Redistribution restrictions apply; creates TYRANO-LEGAL gate".to_string(),
                ],
            },
            EvidenceItem {
                id: "S13".to_string(),
                class: EvidenceClass::ClassA,
                title: "RPG Maker MV official Android conversion docs".to_string(),
                url: "https://rpgmakerofficial.com/product/MV_Help/page/01_11_05.html".to_string(),
                claims: vec![
                    "Uses HTML5/WebGL/Canvas/WebAudio/JS".to_string(),
                    "Android packaging via WebView hybrid application".to_string(),
                ],
            },
            EvidenceItem {
                id: "S14".to_string(),
                class: EvidenceClass::ClassA,
                title: "RPG Maker MZ official output docs".to_string(),
                url: "https://rpgmakerofficial.com/product/MZ_help-en/01_11_03.html".to_string(),
                claims: vec![
                    "MZ web browser deployment".to_string(),
                    "Android-capable modern browsers/WebGL".to_string(),
                ],
            },
            EvidenceItem {
                id: "S15".to_string(),
                class: EvidenceClass::ClassA,
                title: "Android app-specific storage".to_string(),
                url: "https://developer.android.com/training/data-storage/app-specific".to_string(),
                claims: vec![
                    "App-specific internal storage isolated".to_string(),
                    "No system storage permissions needed".to_string(),
                ],
            },
        ]
    }
}
