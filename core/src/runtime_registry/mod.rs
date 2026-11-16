use crate::diagnostics::{DiagnosticCode, DiagnosticError};
use crate::domain::{Abi, EngineId, RuntimeFamily};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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
    #[serde(default)]
    pub native_libs_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeLicenses {
    #[serde(default)]
    pub license_id: Option<String>,
    #[serde(default)]
    pub notice_bundle: Option<String>,
    /// Notice files that must exist inside the runtime directory (paths relative
    /// to it) before an artifact may be reported as packaged. Empty means the
    /// runtime ships no mandatory notice bundle. Engine-agnostic: this is data,
    /// not a per-engine branch in `core/build_plan`.
    #[serde(default)]
    pub required_notice_files: Vec<String>,
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

/// A runtime manifest file that exists but cannot be read or parsed.
///
/// Manifest loading fails closed: a broken manifest must never be skipped
/// silently, because the registry would then silently fall back to the builtin
/// entry and report a runtime configuration the operator did not ask for.
#[derive(Debug)]
pub struct RuntimeManifestError {
    pub path: PathBuf,
    pub reason: String,
}

impl std::fmt::Display for RuntimeManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid runtime manifest '{}': {}",
            self.path.display(),
            self.reason
        )
    }
}

impl std::error::Error for RuntimeManifestError {}

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

    /// Builds a registry from the builtin entries and then loads `dir` on top.
    ///
    /// Shipped manifests override the builtin entry with the same id, so a
    /// malformed manifest must surface as an error instead of leaving the
    /// builtin quietly in place.
    pub fn load_dir_with_builtins(dir: &Path) -> Result<RuntimeRegistry, RuntimeManifestError> {
        let mut registry = RuntimeRegistry::new();
        registry.load_dir(dir)?;
        Ok(registry)
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

    /// Loads every `*.toml` manifest in `dir`.
    ///
    /// - absent directory or a non-directory path: `Ok(0)` (builtin runtimes are
    ///   still usable, and callers may point at a directory that ships nothing);
    /// - unreadable or unparsable `*.toml`: `Err` naming the offending file;
    /// - other files: skipped;
    /// - success: `Ok(count)`.
    pub fn load_dir(&mut self, dir: &Path) -> Result<usize, RuntimeManifestError> {
        if !dir.exists() || !dir.is_dir() {
            return Ok(0);
        }
        let mut count = 0;
        let entries = fs::read_dir(dir).map_err(|err| RuntimeManifestError {
            path: dir.to_path_buf(),
            reason: err.to_string(),
        })?;
        for entry in entries {
            let entry = entry.map_err(|err| RuntimeManifestError {
                path: dir.to_path_buf(),
                reason: err.to_string(),
            })?;
            let path = entry.path();
            if !path.is_file() || path.extension().map(|ext| ext != "toml").unwrap_or(true) {
                continue;
            }
            let content = fs::read_to_string(&path).map_err(|err| RuntimeManifestError {
                path: path.clone(),
                reason: err.to_string(),
            })?;
            let manifest =
                toml::from_str::<RuntimeManifest>(&content).map_err(|err| RuntimeManifestError {
                    path: path.clone(),
                    reason: err.to_string(),
                })?;
            self.register(manifest);
            count += 1;
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
            abis: vec![Abi::Arm64V8a],
            android: RuntimeAndroidConfig {
                min_sdk: 26,
                target_sdk: 34,
            },
            artifacts: Some(RuntimeArtifacts {
                android_module: Some("runtime-rgss".to_string()),
                aar_path: None,
                native_libs_dir: Some("runtimes/rgss/jni".to_string()),
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
                required_notice_files: Vec::new(),
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
            abis: vec![Abi::Arm64V8a],
            android: RuntimeAndroidConfig {
                min_sdk: 26,
                target_sdk: 34,
            },
            artifacts: Some(RuntimeArtifacts {
                android_module: Some("runtime-web".to_string()),
                aar_path: None,
                native_libs_dir: None,
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
                required_notice_files: Vec::new(),
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
            abis: vec![Abi::Arm64V8a],
            android: RuntimeAndroidConfig {
                min_sdk: 26,
                target_sdk: 34,
            },
            artifacts: Some(RuntimeArtifacts {
                android_module: Some("runtimes/renpy-7".to_string()),
                aar_path: None,
                native_libs_dir: Some("runtimes/renpy-7/jni".to_string()),
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
                // Runtime-relative notice bundle: `runtimes/renpy-7/licenses`,
                // matching this runtime's own manifest.json. Not the 8.x value.
                notice_bundle: Some("licenses".to_string()),
                required_notice_files: vec![
                    "licenses/LICENSE.txt".to_string(),
                    "licenses/NOTICE.txt".to_string(),
                    "licenses/components.json".to_string(),
                ],
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
            abis: vec![Abi::Arm64V8a],
            android: RuntimeAndroidConfig {
                min_sdk: 26,
                target_sdk: 34,
            },
            artifacts: Some(RuntimeArtifacts {
                android_module: Some("runtimes/renpy".to_string()),
                aar_path: None,
                native_libs_dir: Some("runtimes/renpy/jni".to_string()),
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
                required_notice_files: vec![
                    "licenses/LICENSE.txt".to_string(),
                    "licenses/NOTICE.txt".to_string(),
                    "licenses/components.json".to_string(),
                ],
            }),
        });

        // Tyrano Web Adapter
        self.register(RuntimeManifest {
            manifest_version: 1,
            id: "tyrano-web".to_string(),
            name: "RuntimePack Tyrano Web Adapter".to_string(),
            version: "0.1.0".to_string(),
            runtime_api: 1,
            family: RuntimeFamily::TyranoWeb,
            engines: vec![EngineId::Tyrano],
            abis: vec![Abi::Arm64V8a],
            android: RuntimeAndroidConfig {
                min_sdk: 26,
                target_sdk: 34,
            },
            artifacts: Some(RuntimeArtifacts {
                android_module: Some("adapter-tyrano".to_string()),
                aar_path: None,
                native_libs_dir: None,
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
                license_id: Some("MIT".to_string()),
                notice_bundle: Some("licenses/tyrano".to_string()),
                required_notice_files: Vec::new(),
            }),
        });

        self.register(RuntimeManifest {
            manifest_version: 1,
            id: "vnmaker-web".to_string(),
            name: "RuntimePack Visual Novel Maker Web Adapter".to_string(),
            version: "0.1.0".to_string(),
            runtime_api: 1,
            family: RuntimeFamily::VnMakerWeb,
            engines: vec![EngineId::VisualNovelMaker],
            abis: vec![Abi::Arm64V8a],
            android: RuntimeAndroidConfig {
                min_sdk: 26,
                target_sdk: 34,
            },
            artifacts: Some(RuntimeArtifacts {
                android_module: Some("adapter-vnmaker".to_string()),
                aar_path: None,
                native_libs_dir: None,
            }),
            capabilities: vec![
                "fs.vfs.read".to_string(),
                "fs.save.write".to_string(),
                "input.keyboard".to_string(),
                "input.pointer".to_string(),
                "web.local_origin".to_string(),
                "web.javascript".to_string(),
                "web.webgl".to_string(),
                "web.webaudio".to_string(),
            ],
            licenses: Some(RuntimeLicenses {
                license_id: Some("MIT OR Apache-2.0".to_string()),
                notice_bundle: Some("licenses/vnmaker".to_string()),
                required_notice_files: Vec::new(),
            }),
        });
    }
}

/// Computes the ABIs a runtime can actually support on this host.
///
/// `declared` is the registry/manifest declaration. `jni_dir` is the resolved
/// native library directory (`None` for pure web runtimes, which ship no `.so`).
///
/// - No native dir: the declaration is returned unchanged (nothing to verify).
/// - Native dir present: the result is the intersection of `declared` and the
///   ABI subdirectories that actually exist, so an APK never advertises an ABI
///   whose libraries are absent.
/// - Empty intersection (or an absent declared directory): fails closed with
///   `BLD-109`, naming both sides, instead of emitting a broken APK.
pub fn effective_abis(
    declared: &[Abi],
    jni_dir: Option<&Path>,
) -> Result<Vec<Abi>, DiagnosticError> {
    effective_abis_report(declared, jni_dir).map(|(effective, _dropped)| effective)
}

/// Like [`effective_abis`], but also returns the declared ABIs that were dropped
/// because no libraries exist for them, so the caller can report the narrowing
/// instead of silently shipping a narrower APK than the declaration implies.
pub fn effective_abis_report(
    declared: &[Abi],
    jni_dir: Option<&Path>,
) -> Result<(Vec<Abi>, Vec<Abi>), DiagnosticError> {
    let jni_dir = match jni_dir {
        Some(dir) => dir,
        None => return Ok((declared.to_vec(), Vec::new())),
    };

    if !jni_dir.is_dir() {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_TEMPLATE_MISSING,
            format!(
                "Native runtime library directory '{}' does not exist in the resource root",
                jni_dir.display()
            ),
        ));
    }

    let mut present: Vec<Abi> = Vec::new();
    let entries = fs::read_dir(jni_dir).map_err(DiagnosticError::Io)?;
    for entry in entries {
        let entry = entry.map_err(DiagnosticError::Io)?;
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(abi) = abi_from_dir_name(&name) {
            present.push(abi);
        }
    }

    // No declaration (custom/unknown runtime id): report what is actually
    // shipped rather than inventing or rejecting a set.
    if declared.is_empty() {
        return Ok((present, Vec::new()));
    }

    let effective: Vec<Abi> = declared
        .iter()
        .copied()
        .filter(|abi| present.contains(abi))
        .collect();
    let dropped: Vec<Abi> = declared
        .iter()
        .copied()
        .filter(|abi| !present.contains(abi))
        .collect();

    if effective.is_empty() {
        let declared_list = declared
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let present_list = if present.is_empty() {
            "<none>".to_string()
        } else {
            present
                .iter()
                .map(|a| a.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_TEMPLATE_MISSING,
            format!(
                "No native libraries for any declared ABI in '{}'. Declared ABIs: [{}]; ABI directories present: [{}]. Add the missing ABI libraries or narrow the runtime's declared ABIs.",
                jni_dir.display(),
                declared_list,
                present_list
            ),
        ));
    }

    Ok((effective, dropped))
}

fn abi_from_dir_name(name: &str) -> Option<Abi> {
    match name {
        "arm64-v8a" => Some(Abi::Arm64V8a),
        "x86_64" => Some(Abi::X86_64),
        "armeabi-v7a" => Some(Abi::ArmeabiV7a),
        _ => None,
    }
}

#[cfg(test)]
mod abi_availability_tests {
    use super::*;
    use crate::domain::Abi;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rtp_abi_{}_{}_{}",
            prefix,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn effective_abis_returns_declared_when_runtime_has_no_native_dir() {
        let declared = vec![Abi::Arm64V8a];
        let res = effective_abis(&declared, None).expect("no native dir is not an error");
        assert_eq!(res, declared);
    }

    #[test]
    fn effective_abis_intersects_declared_with_present_dirs() {
        let jni = temp_dir("jni_arm_only");
        fs::create_dir_all(jni.join("arm64-v8a")).unwrap();
        let res = effective_abis(&[Abi::Arm64V8a, Abi::X86_64], Some(&jni))
            .expect("arm64 present must satisfy the declaration");
        assert_eq!(res, vec![Abi::Arm64V8a]);
    }

    #[test]
    fn effective_abis_keeps_both_when_both_dirs_present() {
        let jni = temp_dir("jni_both");
        fs::create_dir_all(jni.join("arm64-v8a")).unwrap();
        fs::create_dir_all(jni.join("x86_64")).unwrap();
        let res = effective_abis(&[Abi::Arm64V8a, Abi::X86_64], Some(&jni)).unwrap();
        assert_eq!(res, vec![Abi::Arm64V8a, Abi::X86_64]);
    }

    #[test]
    fn effective_abis_fails_when_no_declared_abi_dir_present() {
        let jni = temp_dir("jni_mismatch");
        fs::create_dir_all(jni.join("x86_64")).unwrap();
        let err = effective_abis(&[Abi::Arm64V8a], Some(&jni))
            .expect_err("declared arm64 with no arm64 dir must fail closed");
        assert_eq!(err.code(), Some(DiagnosticCode::BLD_TEMPLATE_MISSING));
        let text = err.to_string();
        assert!(
            text.contains("arm64-v8a") && text.contains("x86_64"),
            "diagnostic must list declared and present ABIs, got: {text}"
        );
    }

    #[test]
    fn effective_abis_returns_present_set_when_nothing_declared() {
        let jni = temp_dir("jni_undeclared");
        fs::create_dir_all(jni.join("arm64-v8a")).unwrap();
        let res = effective_abis(&[], Some(&jni)).expect("undeclared runtime must not fail");
        assert_eq!(res, vec![Abi::Arm64V8a]);
    }

    #[test]
    fn effective_abis_report_names_the_dropped_abis() {
        let jni = temp_dir("jni_report");
        fs::create_dir_all(jni.join("arm64-v8a")).unwrap();
        let (effective, dropped) =
            effective_abis_report(&[Abi::Arm64V8a, Abi::X86_64], Some(&jni)).unwrap();
        assert_eq!(effective, vec![Abi::Arm64V8a]);
        assert_eq!(
            dropped,
            vec![Abi::X86_64],
            "narrowing must be reportable, not silent"
        );
    }

    #[test]
    fn effective_abis_fails_when_declared_native_dir_is_absent() {
        let missing = temp_dir("jni_missing").join("does_not_exist");
        let err = effective_abis(&[Abi::Arm64V8a], Some(&missing))
            .expect_err("absent native dir must fail closed");
        assert_eq!(err.code(), Some(DiagnosticCode::BLD_TEMPLATE_MISSING));
        assert!(
            err.to_string().contains("does_not_exist"),
            "diagnostic must name the missing directory, got: {err}"
        );
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

#[cfg(test)]
mod manifest_loading_tests {
    use super::{effective_abis, RuntimeManifest, RuntimeRegistry};
    use crate::domain::Abi;
    use crate::resource_root::{
        is_contained_in_root, resolve_runtime_module_dir, safe_runtime_relative_path,
    };
    use std::collections::HashSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rp-manifest-test-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// `runtimes/manifests`, next to the crate's resource root.
    fn shipped_manifest_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("runtimes")
            .join("manifests")
    }

    /// Parses every shipped `*.toml` manifest, sorted by path for determinism.
    fn shipped_manifests(dir: &Path) -> Vec<(PathBuf, RuntimeManifest)> {
        let mut paths: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().map(|ext| ext == "toml").unwrap_or(false))
            .collect();
        paths.sort();
        paths
            .into_iter()
            .map(|path| {
                let content = fs::read_to_string(&path).unwrap();
                let manifest: RuntimeManifest = toml::from_str(&content)
                    .unwrap_or_else(|err| panic!("{}: {err}", path.display()));
                (path, manifest)
            })
            .collect()
    }

    #[test]
    fn malformed_manifest_is_an_error_not_a_silent_skip() {
        let dir = scratch("malformed");
        fs::write(dir.join("bad.toml"), "this is not toml {{{").unwrap();
        let mut registry = RuntimeRegistry::new();
        let err = registry.load_dir(&dir).unwrap_err();
        assert!(
            err.path.ends_with("bad.toml"),
            "error must name the offending file: {err}"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_enum_value_is_an_error() {
        let dir = scratch("unknown-enum");
        fs::write(
            dir.join("bad-family.toml"),
            r#"
manifest_version = 1
id = "x"
version = "1.0.0"
runtime_api = 1
family = "not-a-family"
engines = ["renpy"]
abis = ["arm64-v8a"]
capabilities = []
[android]
min_sdk = 26
"#,
        )
        .unwrap();
        let mut registry = RuntimeRegistry::new();
        let err = registry.load_dir(&dir).unwrap_err();
        assert!(
            err.reason.contains("not-a-family") || err.reason.contains("unknown variant"),
            "error must explain the bad value: {}",
            err.reason
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_directory_is_not_an_error() {
        let dir = std::env::temp_dir().join("rp-manifest-test-does-not-exist-xyz");
        let _ = fs::remove_dir_all(&dir);
        let mut registry = RuntimeRegistry::new();
        assert_eq!(registry.load_dir(&dir).unwrap(), 0);
    }

    #[test]
    fn all_shipped_manifests_parse() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("runtimes")
            .join("manifests");
        let mut registry = RuntimeRegistry::new();
        let count = registry.load_dir(&manifest_dir).unwrap();
        assert_eq!(count, 6, "all six shipped manifests must parse");
        let ids: Vec<&str> = registry.all().iter().map(|m| m.id.as_str()).collect();
        for expected in [
            "renpy-android-7",
            "renpy-android-8",
            "rgss-android",
            "web-rpgmaker",
            "vnmaker-web",
            "tyrano-web",
        ] {
            assert!(ids.contains(&expected), "missing {expected} in {ids:?}");
        }
    }

    /// Exact notice contract for the shipped Ren'Py runtimes.
    const RENPY_REQUIRED_NOTICE_FILES: [&str; 3] = [
        "licenses/LICENSE.txt",
        "licenses/NOTICE.txt",
        "licenses/components.json",
    ];

    /// Test-only enforcement of the notice contract a future TOML loader will
    /// have to apply. This is deliberately *not* production wiring: nothing in
    /// `resource_root` calls it, and the shipped loader is still inert, so no
    /// gate is fail-open today. It mirrors what such a loader would need:
    ///
    /// 1. every declared path resolves safely inside the runtime module (no
    ///    absolute path, no `..`, no backslash, no NUL);
    /// 2. no duplicate entries (a set comparison alone would mask them);
    /// 3. the declared list equals the builtin contract exactly;
    /// 4. every declared path is a real regular file inside the module dir.
    fn validate_notice_contract(
        resource_root: &Path,
        builtins: &RuntimeRegistry,
        manifest: &RuntimeManifest,
    ) -> Result<(), String> {
        let Some(builtin) = builtins.get(&manifest.id) else {
            return Ok(());
        };
        let expected = builtin
            .licenses
            .as_ref()
            .map(|licenses| licenses.required_notice_files.clone())
            .unwrap_or_default();
        if expected.is_empty() {
            return Ok(());
        }

        let declared = manifest
            .licenses
            .as_ref()
            .map(|licenses| licenses.required_notice_files.clone())
            .unwrap_or_default();

        let module_rel = builtin
            .artifacts
            .as_ref()
            .and_then(|artifacts| artifacts.android_module.clone())
            .ok_or_else(|| {
                format!(
                    "builtin '{}' requires notice files but declares no android_module",
                    manifest.id
                )
            })?;
        let module_dir = resolve_runtime_module_dir(resource_root, &manifest.id).ok_or_else(|| {
            format!(
                "runtime module '{module_rel}' for '{}' was not found under '{}'",
                manifest.id,
                resource_root.display()
            )
        })?;
        if !is_contained_in_root(&module_dir, resource_root) {
            return Err(format!(
                "runtime module '{}' escapes the resource root",
                module_dir.display()
            ));
        }

        // Safety before set comparison, so an unsafe entry is reported as unsafe
        // instead of being hidden behind a mismatch diagnostic.
        for rel in &declared {
            safe_runtime_relative_path(&module_dir, rel)
                .map_err(|err| format!("required notice path '{rel}' is unsafe: {err}"))?;
        }

        let mut seen: HashSet<&str> = HashSet::new();
        for rel in &declared {
            if !seen.insert(rel.as_str()) {
                return Err(format!("duplicate required notice path '{rel}'"));
            }
        }

        let mut declared_sorted = declared.clone();
        declared_sorted.sort();
        let mut expected_sorted = expected.clone();
        expected_sorted.sort();
        if declared_sorted != expected_sorted {
            return Err(format!(
                "required_notice_files {declared:?} do not match the builtin contract {expected:?}"
            ));
        }

        for rel in &declared {
            let path = safe_runtime_relative_path(&module_dir, rel)
                .map_err(|err| format!("required notice path '{rel}' is unsafe: {err}"))?;
            if !path.is_file() {
                return Err(format!(
                    "required notice '{rel}' is not a regular file inside '{}'",
                    module_dir.display()
                ));
            }
            if !is_contained_in_root(&path, &module_dir) {
                return Err(format!(
                    "required notice '{rel}' resolves outside its runtime module"
                ));
            }
        }
        Ok(())
    }

    /// Characterization + contract. The shipped data already satisfied the
    /// previous `!declared.is_empty()` assertion, so this test was green before
    /// and after the strengthening; what changed is the assertion level: exact
    /// builtin equality, safe containment, and real-file existence.
    #[test]
    fn shipped_manifests_declare_required_notice_files_where_builtin_does() {
        let manifest_dir = shipped_manifest_dir();
        let resource_root = manifest_dir
            .parent()
            .and_then(|runtimes| runtimes.parent())
            .expect("runtimes/manifests must sit under the resource root")
            .to_path_buf();
        let builtins = RuntimeRegistry::new();
        let manifests = shipped_manifests(&manifest_dir);

        let mut checked: Vec<String> = Vec::new();
        for (path, manifest) in &manifests {
            let builtin_requires = builtins
                .get(&manifest.id)
                .and_then(|builtin| builtin.licenses.as_ref())
                .map(|licenses| !licenses.required_notice_files.is_empty())
                .unwrap_or(false);
            if !builtin_requires {
                continue;
            }
            checked.push(manifest.id.clone());
            if let Err(problem) = validate_notice_contract(&resource_root, &builtins, manifest) {
                panic!("{}: {problem}", path.display());
            }
        }
        assert_eq!(
            checked,
            vec!["renpy-android-7".to_string(), "renpy-android-8".to_string()],
            "both Ren'Py runtimes must stay in scope of the notice contract"
        );

        let mut pinned = RENPY_REQUIRED_NOTICE_FILES.to_vec();
        pinned.sort();
        for id in ["renpy-android-7", "renpy-android-8"] {
            let (_, manifest) = manifests
                .iter()
                .find(|(_, manifest)| manifest.id == id)
                .unwrap_or_else(|| panic!("{id} manifest must be shipped"));
            let mut declared = manifest
                .licenses
                .as_ref()
                .map(|licenses| licenses.required_notice_files.clone())
                .unwrap_or_default();
            declared.sort();
            assert_eq!(declared, pinned, "{id}: exact required notice paths");

            let builtin = builtins.get(id).expect("builtin entry");
            let builtin_required = builtin
                .licenses
                .as_ref()
                .map(|licenses| licenses.required_notice_files.clone())
                .unwrap_or_default();
            assert_eq!(
                builtin_required,
                RENPY_REQUIRED_NOTICE_FILES.to_vec(),
                "{id}: the builtin contract itself must not drift"
            );
        }
    }

    fn notice_fixture_root(name: &str) -> PathBuf {
        let root = scratch(&format!("notice-{name}"));
        for rel in RENPY_REQUIRED_NOTICE_FILES {
            let path = root.join("runtimes/renpy-7").join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, format!("fixture:{rel}")).unwrap();
        }
        root
    }

    fn notice_fixture_manifest(required: Option<&[&str]>) -> RuntimeManifest {
        let required_line = match required {
            Some(values) => {
                let list = values
                    .iter()
                    .map(|value| format!("\"{value}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("\nrequired_notice_files = [{list}]")
            }
            None => String::new(),
        };
        let text = format!(
            r#"manifest_version = 1
id = "renpy-android-7"
version = "7.5.3"
runtime_api = 1
family = "renpy"
engines = ["renpy"]
abis = ["arm64-v8a"]
capabilities = []

[android]
min_sdk = 26

[artifacts]
android_module = "runtimes/renpy-7"
native_libs_dir = "runtimes/renpy-7/jni"

[licenses]
license_id = "MIT"
notice_bundle = "licenses"{required_line}
"#
        );
        toml::from_str(&text).unwrap()
    }

    /// Negative fixtures. Every case below is accepted by the previous
    /// `!declared.is_empty()` gate (asserted as characterization) and rejected by
    /// the exact contract. No production loader is involved or weakened.
    #[test]
    fn notice_contract_rejects_partial_typo_duplicate_missing_and_unsafe_declarations() {
        let builtins = RuntimeRegistry::new();
        let cases: Vec<(&str, Option<Vec<&str>>, &str)> = vec![
            (
                "omitted-one",
                Some(vec!["licenses/LICENSE.txt", "licenses/NOTICE.txt"]),
                "do not match",
            ),
            (
                "typo",
                Some(vec![
                    "licenses/LICENSE.txt",
                    "licenses/NOTICE.txt",
                    "licenses/components.jsonn",
                ]),
                "do not match",
            ),
            (
                "duplicate",
                Some(vec![
                    "licenses/LICENSE.txt",
                    "licenses/LICENSE.txt",
                    "licenses/NOTICE.txt",
                    "licenses/components.json",
                ]),
                "duplicate",
            ),
            (
                "unsafe-relpath",
                Some(vec![
                    "../licenses/LICENSE.txt",
                    "licenses/NOTICE.txt",
                    "licenses/components.json",
                ]),
                "unsafe",
            ),
            (
                "missing-file",
                Some(vec![
                    "licenses/LICENSE.txt",
                    "licenses/NOTICE.txt",
                    "licenses/components.json",
                ]),
                "not a regular file",
            ),
            ("omitted-array", None, "do not match"),
        ];

        for (label, declared, expected) in cases {
            let root = notice_fixture_root(label);
            if label == "missing-file" {
                fs::remove_file(root.join("runtimes/renpy-7/licenses/NOTICE.txt")).unwrap();
            }
            let manifest = notice_fixture_manifest(declared.as_deref());

            // Characterization of the previous gate: a non-empty array passed.
            if let Some(values) = &declared {
                assert!(
                    !values.is_empty(),
                    "{label}: the previous non-empty gate accepted this fixture"
                );
            }

            let err = validate_notice_contract(&root, &builtins, &manifest)
                .expect_err("the exact notice contract must reject this fixture");
            assert!(
                err.contains(expected),
                "{label}: expected '{expected}' in diagnostic, got: {err}"
            );
        }
    }

    /// Contract for a shipped ABI declaration:
    ///
    /// * A runtime with `native_libs_dir` must back every declared ABI with its
    ///   **own** ABI directory containing at least one non-empty `.so` regular
    ///   file, and that directory must be contained in the resource root. An
    ///   authored empty ABI directory therefore never counts as support.
    /// * A runtime without `native_libs_dir` ships no native artifact at all
    ///   (pure Java/Kotlin + WebView), so its APK is ABI-agnostic: absence of
    ///   `.so` in *another* runtime cannot narrow the declaration. The only
    ///   requirement is that each value is a valid `Abi` and
    ///   `effective_abis(declared, None)` preserves the declaration unchanged.
    ///
    /// `effective_abis` production semantics are not modified here.
    fn validate_abi_contract(
        resource_root: &Path,
        manifest: &RuntimeManifest,
    ) -> Result<(), String> {
        if manifest.abis.is_empty() {
            return Err("declares no ABI at all".to_string());
        }
        let Some(rel) = manifest
            .artifacts
            .as_ref()
            .and_then(|artifacts| artifacts.native_libs_dir.as_deref())
        else {
            let effective = effective_abis(&manifest.abis, None)
                .map_err(|err| format!("effective_abis rejected the declaration: {err}"))?;
            if effective != manifest.abis {
                return Err(format!(
                    "effective_abis(declared, None) changed {:?} into {:?}",
                    manifest.abis, effective
                ));
            }
            return Ok(());
        };

        let dir = resource_root.join(rel);
        if !dir.is_dir() {
            return Err(format!("native_libs_dir '{rel}' does not exist"));
        }
        if !is_contained_in_root(&dir, resource_root) {
            return Err(format!("native_libs_dir '{rel}' escapes the resource root"));
        }

        for abi in &manifest.abis {
            let abi_name = abi.to_string();
            let abi_dir = dir.join(&abi_name);
            if !abi_dir.is_dir() {
                return Err(format!(
                    "declared ABI '{abi_name}' has no directory at '{rel}/{abi_name}'"
                ));
            }
            let mut backed = false;
            let entries = fs::read_dir(&abi_dir)
                .map_err(|err| format!("cannot list '{rel}/{abi_name}': {err}"))?;
            for entry in entries {
                let entry = entry.map_err(|err| format!("cannot list '{rel}/{abi_name}': {err}"))?;
                let path = entry.path();
                let is_so = path.extension().map(|ext| ext == "so").unwrap_or(false);
                if !is_so || !path.is_file() {
                    continue;
                }
                let len = entry
                    .metadata()
                    .map_err(|err| format!("cannot stat '{}': {err}", path.display()))?
                    .len();
                if len > 0 {
                    backed = true;
                    break;
                }
            }
            if !backed {
                return Err(format!(
                    "declared ABI '{abi_name}' has no non-empty .so under '{rel}/{abi_name}'"
                ));
            }
        }
        Ok(())
    }

    fn abi_fixture_manifest(native_libs_dir: Option<&str>, abis: &[&str]) -> RuntimeManifest {
        let abi_list = abis
            .iter()
            .map(|abi| format!("\"{abi}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let native_line = native_libs_dir
            .map(|rel| format!("\nnative_libs_dir = \"{rel}\""))
            .unwrap_or_default();
        let text = format!(
            r#"manifest_version = 1
id = "abi-fixture"
version = "0.1.0"
runtime_api = 1
family = "web"
engines = ["rpgmaker-mv"]
abis = [{abi_list}]
capabilities = []

[android]
min_sdk = 26

[artifacts]
android_module = "runtimes/fixture"{native_line}
"#
        );
        toml::from_str(&text).unwrap()
    }

    /// Every shipped manifest must satisfy [`validate_abi_contract`]. This
    /// replaces a proxy that derived pure-web ABI declarations from the native
    /// artifacts of *other* runtimes, which wrongly narrowed the web backends to
    /// arm64-v8a even though a WebView APK carries no native library at all.
    #[test]
    fn shipped_manifest_abis_are_backed_by_own_artifacts() {
        let manifest_dir = shipped_manifest_dir();
        let resource_root = manifest_dir
            .parent()
            .and_then(|runtimes| runtimes.parent())
            .expect("runtimes/manifests must sit under the resource root")
            .to_path_buf();
        let manifests = shipped_manifests(&manifest_dir);
        assert_eq!(manifests.len(), 6, "all six shipped manifests must be checked");

        for (path, manifest) in &manifests {
            let native = manifest.artifacts.as_ref().and_then(|a| a.native_libs_dir.as_deref());
            if manifest.id == "rgss-android" && native.is_some_and(|p| !resource_root.join(p).exists()) {
                // Public source deliberately omits the optional unverified RGSS
                // payload. Its absence must still be rejected, never accepted.
                assert!(validate_abi_contract(&resource_root, manifest).is_err());
                let registry = RuntimeRegistry::new();
                let runtime = registry.get("rgss-android").unwrap();
                let missing = resource_root.join(native.unwrap());
                assert_eq!(effective_abis(&runtime.abis, Some(&missing)).unwrap_err().code(), Some("BLD-109"));
            } else if let Err(problem) = validate_abi_contract(&resource_root, manifest) {
                panic!("{}: {problem}", path.display());
            }
        }

        // Restored pre-ea31e59 declaration: a pure-web backend is ABI-agnostic.
        for id in ["web-rpgmaker", "tyrano-web"] {
            let (_, manifest) = manifests
                .iter()
                .find(|(_, manifest)| manifest.id == id)
                .unwrap_or_else(|| panic!("{id} manifest must be shipped"));
            assert!(
                manifest.artifacts.as_ref().and_then(|a| a.native_libs_dir.as_deref()).is_none(),
                "{id} must stay a pure-web backend without native_libs_dir"
            );
            assert_eq!(
                manifest.abis,
                vec![Abi::Arm64V8a, Abi::X86_64],
                "{id}: a WebView APK ships no .so, so the declaration must not be narrowed"
            );
        }

        // Residual drift, recorded rather than hidden: the builtin entries for
        // these two runtimes still declare arm64-v8a only, so a future TOML
        // loader would narrow them. Production ABI selection is out of scope
        // here; see Docs/CONFIG_AND_SCHEMAS.md §6.1 before wiring the loader.
        let builtins = RuntimeRegistry::new();
        for id in ["web-rpgmaker", "tyrano-web"] {
            let builtin = builtins.get(id).expect("builtin entry");
            assert_eq!(
                builtin.abis,
                vec![Abi::Arm64V8a],
                "builtin '{id}' must stay arm64-only in this change; if this fails, update the documented residual drift"
            );
        }
    }

    /// Authored negatives for the ABI contract: an empty ABI directory is not
    /// artifact support, and a pure-web declaration is not narrowed by missing
    /// native artifacts elsewhere.
    #[test]
    fn abi_contract_rejects_declaration_without_own_artifacts() {
        let root = scratch("abi-contract");
        let jni = root.join("runtimes/fixture/jni");
        fs::create_dir_all(jni.join("arm64-v8a")).unwrap();
        fs::write(jni.join("arm64-v8a/libfixture.so"), b"native").unwrap();
        // Authored empty directory: the old proxy counted directories only.
        fs::create_dir_all(jni.join("x86_64")).unwrap();

        let empty_abi_dir = abi_fixture_manifest(
            Some("runtimes/fixture/jni"),
            &["arm64-v8a", "x86_64"],
        );
        let err = validate_abi_contract(&root, &empty_abi_dir)
            .expect_err("an empty ABI directory must not count as artifact support");
        assert!(err.contains("no non-empty .so"), "got: {err}");

        let web = abi_fixture_manifest(None, &["arm64-v8a", "x86_64"]);
        validate_abi_contract(&root, &web).expect(
            "a pure-web declaration must not be narrowed by native artifacts it does not ship",
        );

        let missing_dir = abi_fixture_manifest(Some("runtimes/absent/jni"), &["arm64-v8a"]);
        let err = validate_abi_contract(&root, &missing_dir)
            .expect_err("a missing native_libs_dir must fail closed");
        assert!(err.contains("does not exist"), "got: {err}");
    }
}

/// Schema/serde membership contract for the two v1 enums that gained
/// read-compatibility aliases.
///
/// This reads the JSON schema `enum` arrays directly and compares them with what
/// serde accepts. It is a **membership test only**: it does not run a JSON Schema
/// validator, does not validate reports or manifests against the schema, and
/// passing it does not prove a document is schema-valid.
#[cfg(test)]
mod schema_enum_contract_tests {
    use crate::domain::{EngineId, RuntimeFamily};
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    fn schema_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("schemas")
    }

    fn enum_values(file: &str, pointer: &[&str]) -> BTreeSet<String> {
        let text = fs::read_to_string(schema_dir().join(file))
            .unwrap_or_else(|err| panic!("schema {file} must be readable: {err}"));
        let schema: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|err| panic!("schema {file}: {err}"));
        let mut node = &schema;
        for key in pointer {
            node = node
                .get(*key)
                .unwrap_or_else(|| panic!("schema {file} is missing '{key}'"));
        }
        node.as_array()
            .unwrap_or_else(|| panic!("schema {file} enum must be an array"))
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .unwrap_or_else(|| panic!("schema {file} enum values must be strings"))
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn asset_audit_engine_enum_covers_canonical_and_deprecated_aliases() {
        let values = enum_values(
            "asset-audit-report.schema.json",
            &["properties", "engine", "enum"],
        );
        let aliases = [
            ("rpg-maker-xp", EngineId::RpgMakerXp),
            ("rpg-maker-vx", EngineId::RpgMakerVx),
            ("rpg-maker-vx-ace", EngineId::RpgMakerVxAce),
            ("rpg-maker-vxace", EngineId::RpgMakerVxAce),
            ("rpg-maker-mv", EngineId::RpgMakerMv),
            ("rpg-maker-mz", EngineId::RpgMakerMz),
            ("ren-py", EngineId::RenPy),
        ];
        for (alias, canonical) in aliases {
            assert!(
                values.contains(alias),
                "schema engine enum must accept the deprecated alias '{alias}': {values:?}"
            );
            let parsed: EngineId = serde_json::from_str(&format!("\"{alias}\""))
                .unwrap_or_else(|err| panic!("serde must read alias '{alias}': {err}"));
            assert_eq!(parsed, canonical, "alias '{alias}' must map to {canonical:?}");
            let wire = serde_json::to_string(&canonical).expect("canonical serialization");
            assert_eq!(
                wire,
                serde_json::to_string(&parsed).expect("serialization"),
                "reading an alias must serialize back to the canonical spelling"
            );
            let wire_value = wire.trim_matches('"').to_string();
            assert!(
                values.contains(&wire_value),
                "canonical spelling '{wire_value}' must also be in the schema enum"
            );
            assert_ne!(
                wire_value, alias,
                "alias '{alias}' must never be the serialized spelling"
            );
        }

        // Every value the schema permits must be readable by serde.
        for value in &values {
            serde_json::from_str::<EngineId>(&format!("\"{value}\"")).unwrap_or_else(|err| {
                panic!("schema engine enum value '{value}' must be readable by serde: {err}")
            });
        }

        // CLI-only abbreviations stay out of contract.
        for abbreviation in ["xp", "vx", "vxace", "mv", "mz", "vnm", "vnmaker", "tyranobuilder"] {
            assert!(
                !values.contains(abbreviation),
                "schema must not accept CLI abbreviation '{abbreviation}'"
            );
        }

        // An unknown typo is rejected explicitly, by schema and by serde.
        assert!(!values.contains("rpg-maker-mvv"));
        assert!(
            serde_json::from_str::<EngineId>("\"rpg-maker-mvv\"").is_err(),
            "an unknown engine typo must not deserialize"
        );
    }

    #[test]
    fn runtime_manifest_family_enum_covers_canonical_and_deprecated_aliases() {
        let values = enum_values(
            "runtime-manifest.schema.json",
            &["properties", "family", "enum"],
        );
        let aliases = [
            ("web-rpg-maker", RuntimeFamily::WebRpgMaker),
            ("ren-py", RuntimeFamily::RenPy),
            ("tyrano-web", RuntimeFamily::TyranoWeb),
        ];
        for (alias, canonical) in aliases {
            assert!(
                values.contains(alias),
                "schema family enum must accept the deprecated alias '{alias}': {values:?}"
            );
            let parsed: RuntimeFamily = serde_json::from_str(&format!("\"{alias}\""))
                .unwrap_or_else(|err| panic!("serde must read alias '{alias}': {err}"));
            assert_eq!(parsed, canonical, "alias '{alias}' must map to {canonical:?}");
            let wire = serde_json::to_string(&canonical).expect("canonical serialization");
            let wire_value = wire.trim_matches('"').to_string();
            assert!(
                values.contains(&wire_value),
                "canonical spelling '{wire_value}' must also be in the schema enum"
            );
            assert_ne!(
                wire_value, alias,
                "alias '{alias}' must never be the serialized spelling"
            );
        }

        for value in &values {
            serde_json::from_str::<RuntimeFamily>(&format!("\"{value}\"")).unwrap_or_else(|err| {
                panic!("schema family enum value '{value}' must be readable by serde: {err}")
            });
        }

        for abbreviation in ["vnm", "vnmaker", "tyranobuilder", "rgss-web"] {
            assert!(
                !values.contains(abbreviation),
                "schema must not accept CLI abbreviation '{abbreviation}'"
            );
        }

        assert!(!values.contains("web-rpg-makerr"));
        assert!(
            serde_json::from_str::<RuntimeFamily>("\"web-rpg-makerr\"").is_err(),
            "an unknown family typo must not deserialize"
        );
    }
}
