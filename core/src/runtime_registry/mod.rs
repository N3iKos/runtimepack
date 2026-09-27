use crate::domain::{Abi, EngineId, RuntimeFamily};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Compares two dotted version strings in descending order using numeric
/// component comparison ("8.3.7" > "7.5.3"). Non-numeric components compare as
/// 0 so the ordering is always total and deterministic.
fn compare_versions_desc(a: &str, b: &str) -> Ordering {
    let parse = |v: &str| -> Vec<u64> {
        v.split('.')
            .map(|part| part.trim().parse::<u64>().unwrap_or(0))
            .collect()
    };
    let pa = parse(a);
    let pb = parse(b);
    pb.cmp(&pa)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeAndroidConfig {
    pub min_sdk: u32,
    #[serde(default = "default_target_sdk")]
    pub target_sdk: u32,
}

fn default_target_sdk() -> u32 {
    34
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeArtifacts {
    #[serde(default)]
    pub android_module: Option<String>,
    #[serde(default)]
    pub aar_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeLicenses {
    #[serde(default)]
    pub license_id: Option<String>,
    #[serde(default)]
    pub notice_bundle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeManifest {
    pub manifest_version: u32,
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub version: String,
    pub runtime_api: u32,
    pub family: RuntimeFamily,
    pub engines: Vec<EngineId>,
    pub abis: Vec<Abi>,
    pub android: RuntimeAndroidConfig,
    #[serde(default)]
    pub artifacts: Option<RuntimeArtifacts>,
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub licenses: Option<RuntimeLicenses>,
}

#[derive(Debug, Default, Clone)]
pub struct RuntimeRegistry {
    runtimes: HashMap<String, RuntimeManifest>,
}

impl RuntimeRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            runtimes: HashMap::new(),
        };
        registry.register_builtins();
        registry
    }

    pub fn register(&mut self, manifest: RuntimeManifest) {
        self.runtimes.insert(manifest.id.clone(), manifest);
    }

    pub fn get(&self, id: &str) -> Option<&RuntimeManifest> {
        self.runtimes.get(id)
    }

    pub fn all(&self) -> Vec<&RuntimeManifest> {
        let mut list: Vec<&RuntimeManifest> = self.runtimes.values().collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        list
    }

    pub fn find_for_engine(&self, engine: EngineId) -> Vec<&RuntimeManifest> {
        let mut list: Vec<&RuntimeManifest> = self
            .runtimes
            .values()
            .filter(|r| r.engines.contains(&engine))
            .collect();
        // Deterministic order: newest version first, ties broken by id.
        // `HashMap` iteration order is otherwise unspecified, which previously
        // made `runtimes[0]` pick renpy-android-7 or -8 at random.
        list.sort_by(|a, b| {
            compare_versions_desc(&a.version, &b.version).then_with(|| a.id.cmp(&b.id))
        });
        list
    }

    /// Resolve runtimes for an engine deterministically, preferring an explicit
    /// runtime id when the analyzer named one (e.g. Ren'Py's RNP-100-VERSION
    /// finding maps a detected generation to a runtime id).
    ///
    /// The candidate list is already ordered newest-version-first; the preferred
    /// entry, when present, is moved to the front and the remaining order is
    /// preserved. Engine-agnostic: any engine may supply a preferred id.
    pub fn resolve_for_engine(
        &self,
        engine: EngineId,
        preferred_id: Option<&str>,
    ) -> Vec<&RuntimeManifest> {
        let mut list = self.find_for_engine(engine);
        if let Some(preferred) = preferred_id {
            list.sort_by_key(|r| if r.id == preferred { 0 } else { 1 });
        }
        list
    }

    pub fn load_dir(&mut self, dir: &Path) -> std::io::Result<usize> {
        if !dir.exists() || !dir.is_dir() {
            return Ok(0);
        }
        let mut count = 0;
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "toml" {
                        let content = fs::read_to_string(&path)?;
                        if let Ok(manifest) = toml::from_str::<RuntimeManifest>(&content) {
                            self.register(manifest);
                            count += 1;
                        }
                    }
                }
            }
        }
        Ok(count)
    }

    fn register_builtins(&mut self) {
        // RGSS Android Runtime
        self.register(RuntimeManifest {
            manifest_version: 1,
            id: "rgss-android".to_string(),
            name: "RuntimePack RGSS Backend (XP/VX/VX Ace)".to_string(),
            version: "0.1.0".to_string(),
            runtime_api: 1,
            family: RuntimeFamily::Rgss,
            engines: vec![
                EngineId::RpgMakerXp,
                EngineId::RpgMakerVx,
                EngineId::RpgMakerVxAce,
            ],
            abis: vec![Abi::Arm64V8a, Abi::X86_64],
            android: RuntimeAndroidConfig {
                min_sdk: 26,
                target_sdk: 34,
            },
            artifacts: Some(RuntimeArtifacts {
                android_module: Some("runtime-rgss".to_string()),
                aar_path: None,
            }),
            capabilities: vec![
                "fs.vfs.read".to_string(),
                "fs.vfs.seek".to_string(),
                "fs.save.write".to_string(),
                "input.keyboard".to_string(),
                "input.gamepad".to_string(),
                "input.virtual_gamepad".to_string(),
                "graphics.surface".to_string(),
                "audio.standard".to_string(),
                "rgss.win32.user32.keyboard".to_string(),
                "rgss.win32.kernel32.temp".to_string(),
            ],
            licenses: Some(RuntimeLicenses {
                license_id: Some("GPL-2.0-or-later".to_string()),
                notice_bundle: Some("licenses/rgss".to_string()),
            }),
        });

        // Web RPG Maker Runtime (MV / MZ)
        self.register(RuntimeManifest {
            manifest_version: 1,
            id: "web-rpgmaker".to_string(),
            name: "RuntimePack Web Runtime (MV/MZ)".to_string(),
            version: "0.1.0".to_string(),
            runtime_api: 1,
            family: RuntimeFamily::WebRpgMaker,
            engines: vec![EngineId::RpgMakerMv, EngineId::RpgMakerMz],
            abis: vec![Abi::Arm64V8a, Abi::X86_64],
            android: RuntimeAndroidConfig {
                min_sdk: 26,
                target_sdk: 34,
            },
            artifacts: Some(RuntimeArtifacts {
                android_module: Some("runtime-web".to_string()),
                aar_path: None,
            }),
            capabilities: vec![
                "fs.vfs.read".to_string(),
                "fs.save.write".to_string(),
                "input.keyboard".to_string(),
                "input.pointer".to_string(),
                "input.virtual_gamepad".to_string(),
                "web.local_origin".to_string(),
                "web.javascript".to_string(),
                "web.webgl".to_string(),
                "web.webaudio".to_string(),
                "web.node.path".to_string(),
                "web.node.fs_subset".to_string(),
                "web.nwjs_subset".to_string(),
            ],
            licenses: Some(RuntimeLicenses {
                license_id: Some("MIT OR Apache-2.0".to_string()),
                notice_bundle: Some("licenses/web".to_string()),
            }),
        });

        // Ren'Py Android 7.x
        self.register(RuntimeManifest {
            manifest_version: 1,
            id: "renpy-android-7".to_string(),
            name: "RuntimePack Ren'Py 7 Runtime".to_string(),
            version: "7.5.3".to_string(),
            runtime_api: 1,
            family: RuntimeFamily::RenPy,
            engines: vec![EngineId::RenPy],
            abis: vec![Abi::Arm64V8a, Abi::X86_64],
            android: RuntimeAndroidConfig {
                min_sdk: 26,
                target_sdk: 34,
            },
            artifacts: Some(RuntimeArtifacts {
                android_module: Some("runtimes/renpy".to_string()),
                aar_path: None,
            }),
            capabilities: vec![
                "fs.vfs.read".to_string(),
                "fs.save.write".to_string(),
                "input.keyboard".to_string(),
                "input.pointer".to_string(),
                "renpy.runtime.versioned".to_string(),
                "renpy.python.pure".to_string(),
            ],
            licenses: Some(RuntimeLicenses {
                license_id: Some("MIT".to_string()),
                notice_bundle: Some("licenses/renpy".to_string()),
            }),
        });

        // Ren'Py Android 8.x
        self.register(RuntimeManifest {
            manifest_version: 1,
            id: "renpy-android-8".to_string(),
            name: "RuntimePack Ren'Py 8 Runtime".to_string(),
            version: "8.3.7".to_string(),
            runtime_api: 1,
            family: RuntimeFamily::RenPy,
            engines: vec![EngineId::RenPy],
            abis: vec![Abi::Arm64V8a, Abi::X86_64],
            android: RuntimeAndroidConfig {
                min_sdk: 26,
                target_sdk: 34,
            },
            artifacts: Some(RuntimeArtifacts {
                android_module: Some("runtimes/renpy".to_string()),
                aar_path: None,
            }),
            capabilities: vec![
                "fs.vfs.read".to_string(),
                "fs.save.write".to_string(),
                "input.keyboard".to_string(),
                "input.pointer".to_string(),
                "renpy.runtime.versioned".to_string(),
                "renpy.python.pure".to_string(),
            ],
            licenses: Some(RuntimeLicenses {
                license_id: Some("MIT".to_string()),
                notice_bundle: Some("licenses/renpy".to_string()),
            }),
        });

        // Tyrano Web Adapter (Technical adapter; blocked by TYRANO-LEGAL gate)
        self.register(RuntimeManifest {
            manifest_version: 1,
            id: "tyrano-web".to_string(),
            name: "RuntimePack Tyrano Web Adapter (Subject to TYRANO-LEGAL gate)".to_string(),
            version: "0.1.0".to_string(),
            runtime_api: 1,
            family: RuntimeFamily::TyranoWeb,
            engines: vec![EngineId::Tyrano],
            abis: vec![Abi::Arm64V8a, Abi::X86_64],
            android: RuntimeAndroidConfig {
                min_sdk: 26,
                target_sdk: 34,
            },
            artifacts: Some(RuntimeArtifacts {
                android_module: Some("adapter-tyrano".to_string()),
                aar_path: None,
            }),
            capabilities: vec![
                "fs.vfs.read".to_string(),
                "fs.save.write".to_string(),
                "input.keyboard".to_string(),
                "input.pointer".to_string(),
                "web.local_origin".to_string(),
                "web.javascript".to_string(),
            ],
            licenses: Some(RuntimeLicenses {
                license_id: Some("PROPRIETARY-GATED".to_string()),
                notice_bundle: Some("licenses/tyrano".to_string()),
            }),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_for_engine_is_deterministic_and_newest_first() {
        let registry = RuntimeRegistry::new();
        let runtimes = registry.find_for_engine(EngineId::RenPy);
        assert_eq!(runtimes.len(), 2, "expected both Ren'Py runtimes");
        assert_eq!(
            runtimes[0].id, "renpy-android-8",
            "newest Ren'Py runtime must sort first regardless of HashMap order"
        );
        assert_eq!(runtimes[1].id, "renpy-android-7");

        // Repeated calls must be stable.
        for _ in 0..25 {
            let again = registry.find_for_engine(EngineId::RenPy);
            assert_eq!(again[0].id, "renpy-android-8");
        }
    }

    #[test]
    fn resolve_for_engine_prefers_requested_id() {
        let registry = RuntimeRegistry::new();

        let default = registry.resolve_for_engine(EngineId::RenPy, None);
        assert_eq!(default[0].id, "renpy-android-8");

        let preferred = registry.resolve_for_engine(EngineId::RenPy, Some("renpy-android-7"));
        assert_eq!(
            preferred[0].id, "renpy-android-7",
            "explicit preference must win over version ordering"
        );
        assert_eq!(preferred[1].id, "renpy-android-8");
    }

    #[test]
    fn resolve_for_engine_ignores_unknown_preference() {
        let registry = RuntimeRegistry::new();
        let resolved = registry.resolve_for_engine(EngineId::RenPy, Some("does-not-exist"));
        assert_eq!(resolved[0].id, "renpy-android-8");
    }

    #[test]
    fn compare_versions_desc_orders_numerically() {
        assert_eq!(compare_versions_desc("8.3.7", "7.5.3"), Ordering::Less);
        assert_eq!(compare_versions_desc("7.5.3", "8.3.7"), Ordering::Greater);
        assert_eq!(compare_versions_desc("1.2.0", "1.2.0"), Ordering::Equal);
        assert_eq!(compare_versions_desc("10.0.0", "9.9.9"), Ordering::Less);
    }
}
